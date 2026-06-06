"""Browser test suite for the Ironhold WASM / WebGPU build.

Runs five test categories against a locally served build:
  1. Smoke      — every project loads to InGame with no errors
  2. Action     — clicking a UI button fires the expected Action
  3. Transition — a LoadScene action transitions the scene correctly
  4. Scene      — per-scene screenshots are diffed against stored baselines
  5. Navigation — multi-step menu flows with per-step screenshots

Usage:
    python test_web.py [--skip-build] [--update-baselines] [--project NAME] [--screenshot-dir DIR]

Options:
    --skip-build        Skip wasm-pack build (use existing pkg/)
    --update-baselines  Overwrite stored baseline screenshots
    --update-baseline   Overwrite baseline for a specific target (repeatable)
    --project           Limit tests to one project (repeatable, e.g. --project entity_logic_demo)
    --screenshot-dir    Where to store current screenshots  [default: screenshots]
    --webgpu            SwiftShader WebGPU (headless); exercises the WebGPU code path without a GPU
    --real-gpu          Non-headless Chromium with real D3D12/Vulkan GPU; requires a display

Screenshot layout:
    screenshot_baselines/scenes/      ← committed to git; scene baselines used in gallery
    screenshot_baselines/pause_nav/   ← committed to git; navigation step baselines
    screenshots/                      ← gitignored; current/comparison files written here

Requirements:
    pip install playwright pillow
    playwright install chromium
"""

import argparse
import asyncio
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image, ImageChops
from playwright.async_api import async_playwright, Browser, BrowserContext, Page

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

PORT = 8000
BASE_URL = f"http://localhost:{PORT}"

PROJECTS = ["quick_scene", "3rd_person_game_demo", "terrain_demo", "custom_materials", "primitive_world", "entity_logic_demo", "particles_demo", "effect_mayhem_demo", "foliage_demo", "stats_demo", "blank_project"]

# Root of the asset tree, relative to the working directory where this script runs.
ASSETS_ROOT = Path("assets/projects")

# Committed baseline screenshots.  Two sub-dirs:
#   scenes/     — one PNG per scene (used by the gallery)
#   pause_nav/  — one PNG per navigation step
# Current/comparison screenshots go in the --screenshot-dir (gitignored).
BASELINE_DIR = Path("screenshot_baselines")

# Seconds to wait for <canvas> / InGame state
CANVAS_TIMEOUT = 180
# Seconds to wait for an async state change (e.g. after a button click)
ACTION_TIMEOUT = 20
# Frames to render after InGame before taking a baseline screenshot.
# Allows async work (terrain mesh generation, texture streaming) to complete.
# At 60 fps this is ~2 seconds; frame-based counting is more reliable than
# wall-clock sleep because it tracks actual engine activity.
SCREENSHOT_SETTLE_FRAMES = 120

# Screenshot diff: fraction of pixels allowed to differ before failing
BASELINE_DIFF_THRESHOLD = 0.04   # 4 %
# Per-channel tolerance before a pixel counts as "different"
PIXEL_TOLERANCE = 15

CHROMIUM_ARGS_GL = [
    "--enable-unsafe-webgpu",
    "--enable-features=Vulkan",
    "--no-sandbox",
    "--disable-setuid-sandbox",
]

CHROMIUM_ARGS_WEBGPU = [
    "--enable-unsafe-webgpu",
    "--enable-features=Vulkan,UseSkiaRenderer",
    "--use-vulkan=swiftshader",
    "--disable-vulkan-surface",
    "--no-sandbox",
    "--disable-setuid-sandbox",
]

CHROMIUM_ARGS_REAL_GPU = [
    "--enable-unsafe-webgpu",
    "--no-sandbox",
    "--disable-setuid-sandbox",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

class TestFailure(Exception):
    pass


async def wait_for_debug_state(page: Page, predicate, timeout_s: int = ACTION_TIMEOUT) -> dict:
    """Poll #debug-state until predicate(state_dict) is True or timeout."""
    deadline = asyncio.get_event_loop().time() + timeout_s
    while asyncio.get_event_loop().time() < deadline:
        raw = await page.inner_text("#debug-state")
        if raw:
            try:
                state = json.loads(raw)
                if predicate(state):
                    return state
            except json.JSONDecodeError:
                pass
        await asyncio.sleep(0.3)
    raw = await page.inner_text("#debug-state")
    raise TestFailure(
        f"Timed out after {timeout_s}s waiting for state condition.\n"
        f"  Last state: {raw or '(empty)'}"
    )


async def wait_for_scene_ready(
    page: Page,
    settle_frames: int = SCREENSHOT_SETTLE_FRAMES,
) -> None:
    """Wait for InGame state, then wait for settle_frames more frames to render.

    Using frame count rather than wall-clock time ensures async scene work
    (terrain mesh generation, texture uploads) has actually completed before
    a screenshot is taken.
    """
    state = await wait_for_debug_state(
        page,
        lambda s: s.get("app_state") == "InGame",
        timeout_s=CANVAS_TIMEOUT,
    )
    entry_frame = state.get("frame", 0)
    await wait_for_debug_state(
        page,
        lambda s: s.get("frame", 0) >= entry_frame + settle_frames,
        timeout_s=CANVAS_TIMEOUT,
    )


def discover_scenes(project: str) -> list[str]:
    """Return all scene paths for a project, relative to the project root.

    E.g. for ``3rd_person_game_demo`` this might return::

        ["scenes/main.scene.ron", "scenes/start_menu.scene.ron", ...]

    The paths are suitable for passing directly as the ``?scene=`` URL param
    and as the ``InitialSceneOverride`` resource in the engine.
    """
    scenes_dir = ASSETS_ROOT / project / "scenes"
    if not scenes_dir.is_dir():
        return []
    return sorted(
        str(p.relative_to(ASSETS_ROOT / project)).replace("\\", "/")
        for p in scenes_dir.glob("*.scene.ron")
    )


async def open_project(
    context: BrowserContext,
    project: str | None = None,
    scene: str | None = None,
) -> tuple[Page, list[str]]:
    """Open a new page for the given project (and optional scene), return (page, error_list).

    When *scene* is supplied it is passed as ``?scene=<path>`` which activates
    the engine's ``InitialSceneOverride``, bypassing the project's normal
    ``initial_scene``.
    """
    errors: list[str] = []

    def on_console(msg):
        text = msg.text
        level = msg.type
        if level == "error":
            errors.append(f"[console error] {text}")
        elif any(p in text for p in ["panicked at", "VALIDATION ERROR", "wgpu error", "No WebGPU"]):
            errors.append(f"[fatal log] {text}")

    page = await context.new_page()
    page.on("console", on_console)
    page.on("pageerror", lambda e: errors.append(f"[page error] {e}"))

    url = f"{BASE_URL}/play.html?project={project}&testing=1" if project else f"{BASE_URL}/play.html?testing=1"
    if scene:
        url += f"&scene={scene}"
    await page.goto(url, wait_until="networkidle")

    try:
        await page.wait_for_selector("canvas", timeout=CANVAS_TIMEOUT * 1000)
    except Exception:
        raise TestFailure(
            f"Timed out waiting for <canvas> on project '{project or 'default'}'"
            + (f", scene '{scene}'" if scene else "")
            + " — WebGPU adapter likely failed."
        )

    return page, errors


def compare_screenshots(baseline_path: str, current_path: str) -> float:
    """Return fraction of pixels that differ by more than PIXEL_TOLERANCE."""
    img_a = Image.open(baseline_path).convert("RGB")
    img_b = Image.open(current_path).convert("RGB")
    if img_a.size != img_b.size:
        return 1.0
    diff = ImageChops.difference(img_a, img_b)
    total = img_a.width * img_a.height
    differing = sum(1 for p in diff.getdata() if max(p) > PIXEL_TOLERANCE)
    return differing / total


# ---------------------------------------------------------------------------
# Test 1 — Smoke: every project reaches InGame
# ---------------------------------------------------------------------------

async def test_smoke(context: BrowserContext, project: str) -> None:
    page, errors = await open_project(context, project)
    try:
        await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame",
            timeout_s=CANVAS_TIMEOUT,
        )
        if errors:
            raise TestFailure(f"Browser errors detected:\n" + "\n".join(f"  • {e}" for e in errors))
    finally:
        await page.close()


# ---------------------------------------------------------------------------
# Test 2 — Action: clicking Dance triggers PlayAnimation
# ---------------------------------------------------------------------------

async def test_button_fires_action(context: BrowserContext) -> None:
    """
    quick_scene has a 'Dance' button (trigger: 'dance').
    Rule: ui.button_pressed:dance → PlayAnimation("dance")
    """
    page, errors = await open_project(context, "quick_scene")
    try:
        await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")

        # Bevy UI is canvas-rendered, not DOM — click by canvas coordinates.
        # Dance button: position=(20,60) size=(150,40) → center=(95,80)
        await page.mouse.click(95, 80)

        state = await wait_for_debug_state(
            page,
            lambda s: "PlayAnimation" in s.get("last_action", ""),
        )
        if errors:
            raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
        return state["last_action"]
    finally:
        await page.close()


# ---------------------------------------------------------------------------
# Test 3 — Transition: Start Game loads a new scene
# ---------------------------------------------------------------------------

async def test_scene_transition(context: BrowserContext) -> None:
    """
    3rd_person_game_demo starts at start_menu.scene.ron.
    Clicking 'Start Game' → LoadScene("scenes/main.scene.ron") → new scene loads.
    """
    page, errors = await open_project(context, "3rd_person_game_demo")
    try:
        # Wait for start_menu to fully load
        start_state = await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
        initial_scene = start_state.get("scene", "")

        # Trigger the scene load.
        # Start Game button: position=(100,100) size=(300,65) → center=(250,132)
        await page.mouse.click(250, 132)

        # Wait for a different scene to become ready in InGame
        final_state = await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame" and s.get("scene", "") != initial_scene,
            timeout_s=CANVAS_TIMEOUT,
        )

        # Verify the scene actually changed to main (not just any different scene).
        # We don't assert last_action == LoadScene here because on-load rules (e.g.
        # PlayMusicLoop, Preload) fire immediately after scene.ready and overwrite it.
        if "main" not in final_state.get("scene", ""):
            raise TestFailure(
                f"Expected scene to contain 'main', got: {final_state.get('scene')}"
            )
        if errors:
            raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
        return initial_scene, final_state["scene"]
    finally:
        await page.close()


# ---------------------------------------------------------------------------
# Test 4 — Scene Baselines: one screenshot per scene per project
# ---------------------------------------------------------------------------

def scene_baseline_key(project: str, scene: str) -> str:
    """Return the filesystem-safe key used for the baseline filename.

    E.g. ``("3rd_person_game_demo", "scenes/main.scene.ron")`` →
    ``"3rd_person_game_demo_main"``.
    """
    stem = Path(scene).stem.replace(".scene", "")
    return f"{project}_{stem}"


async def test_screenshot_scene_baseline(
    context: BrowserContext,
    project: str,
    scene: str,
    screenshot_dir: Path,
    update: bool,
) -> None:
    """Load *scene* directly inside *project* and diff against the stored baseline."""
    page, errors = await open_project(context, project, scene=scene)
    try:
        await wait_for_scene_ready(page)

        key = scene_baseline_key(project, scene)
        baselines_dir = BASELINE_DIR / "scenes"
        current_path = screenshot_dir / f"{key}_current.png"
        baseline_path = baselines_dir / f"{key}.png"

        await page.screenshot(path=str(current_path))

        if update or not baseline_path.exists():
            baselines_dir.mkdir(parents=True, exist_ok=True)
            shutil.copy(current_path, baseline_path)
            print(f"    Baseline {'updated' if baseline_path.exists() else 'created'}: {baseline_path.name}")
            return

        diff = compare_screenshots(str(baseline_path), str(current_path))
        if diff > BASELINE_DIFF_THRESHOLD:
            raise TestFailure(
                f"Screenshot diff {diff:.1%} exceeds {BASELINE_DIFF_THRESHOLD:.0%} threshold. "
                f"Inspect: {current_path}"
            )
        print(f"    Diff: {diff:.2%} (threshold {BASELINE_DIFF_THRESHOLD:.0%}) — OK")

        if errors:
            raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
    finally:
        await page.close()


# ---------------------------------------------------------------------------
# Test 5 — Navigation: pause menu flow with per-step screenshots
# ---------------------------------------------------------------------------

async def test_pause_menu_navigation(
    context: BrowserContext,
    screenshot_dir: Path,
    update: bool,
) -> None:
    """
    Full pause menu navigation flow for 3rd_person_game_demo:
      1. Start menu (screenshot)
      2. Click Start Game → main scene (screenshot)
      3. Press Esc → pause menu opens (screenshot)
      4. Press Esc → pause menu closes (screenshot)
      5. Press Esc → pause menu opens again (screenshot)
      6. Click Resume → main scene (screenshot)

    Button coordinates (absolute-positioned, 1280×720 viewport):
      Start Game: position=(100,100) size=(300,65) → click (250, 132)

    Resume button (ui_panel centered layout, panel width=320 padding=30 gap=16):
      Panel height = 30 + (50+16+65+16+65) + 30 = 272 → top = (720-272)/2 = 224
      Resume center y = 224 + 30 + 50 + 16 + 65/2 = 352
      Resume center x = 1280/2 = 640 → click (640, 352)
    """
    page, errors = await open_project(context, "3rd_person_game_demo")
    steps_dir = screenshot_dir / "pause_nav"
    steps_dir.mkdir(parents=True, exist_ok=True)

    async def snap(name: str) -> None:
        """Capture a step screenshot and diff against its baseline (or create one)."""
        await asyncio.sleep(0.6)   # let one rendered frame settle
        current = steps_dir / f"{name}_current.png"
        baseline = BASELINE_DIR / "pause_nav" / f"{name}.png"
        await page.screenshot(path=str(current))
        if update or not baseline.exists():
            baseline.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(current, baseline)
            label = "updated" if baseline.exists() else "created"
            print(f"      baseline {label}: {name}")
        else:
            diff = compare_screenshots(str(baseline), str(current))
            verdict = "OK" if diff <= BASELINE_DIFF_THRESHOLD else "DIFF"
            print(f"      {name}: {diff:.2%} — {verdict}")
            if diff > BASELINE_DIFF_THRESHOLD:
                raise TestFailure(
                    f"Screenshot '{name}' diff {diff:.1%} exceeds threshold. See: {current}"
                )

    async def after_input(frame_before: int, expected_action: str = "") -> dict:
        """Wait for Bevy to process an input: frame advances past frame_before and,
        if expected_action is given, last_action must contain that string."""
        def pred(s: dict) -> bool:
            if s.get("frame", 0) <= frame_before:
                return False
            if expected_action and expected_action not in s.get("last_action", ""):
                return False
            return True
        return await wait_for_debug_state(page, pred)

    try:
        # ── Step 1: start menu ──────────────────────────────────────────────
        await wait_for_scene_ready(page)
        await snap("01_start_menu")

        # ── Step 2: Start Game → main scene ────────────────────────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.mouse.click(250, 132)   # Start Game button
        await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame" and "main" in s.get("scene", ""),
            timeout_s=CANVAS_TIMEOUT,
        )
        await snap("02_main_scene")

        # ── Step 3: Esc → pause overlay opens ──────────────────────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.keyboard.press("Escape")
        await wait_for_debug_state(
            page,
            lambda s: s.get("frame", 0) > frame and s.get("logic_state") == "paused",
        )
        await snap("03_pause_menu_open")

        # ── Step 4: Esc → pause overlay closes ─────────────────────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.keyboard.press("Escape")
        await wait_for_debug_state(
            page,
            lambda s: s.get("frame", 0) > frame and s.get("logic_state") == "playing",
        )
        await snap("04_main_scene_after_esc_close")

        # ── Step 5: Esc → pause overlay opens again ─────────────────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.keyboard.press("Escape")
        await wait_for_debug_state(
            page,
            lambda s: s.get("frame", 0) > frame and s.get("logic_state") == "paused",
        )
        await snap("05_pause_menu_reopen")

        # ── Step 6: Resume button → overlay dismissed ───────────────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.mouse.click(640, 352)   # Resume button (centered panel layout)
        await wait_for_debug_state(
            page,
            lambda s: s.get("frame", 0) > frame and s.get("logic_state") == "playing",
        )
        await snap("06_main_scene_after_resume")

        if errors:
            raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
    finally:
        await page.close()


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

async def run_all(
    screenshot_dir: Path,
    update_baselines: bool,
    update_baseline_projects: set[str],
    filter_projects: set[str],
    chromium_args: list[str] | None = None,
    headless: bool = True,
) -> list[tuple[str, bool, str]]:
    results: list[tuple[str, bool, str]] = []
    # When --project is given, restrict to those projects; otherwise run all.
    active_projects = [p for p in PROJECTS if not filter_projects or p in filter_projects]

    async with async_playwright() as pw:
        browser: Browser = await pw.chromium.launch(headless=headless, args=chromium_args or CHROMIUM_ARGS_GL)
        context: BrowserContext = await browser.new_context(viewport={"width": 1280, "height": 720})

        # --- Smoke tests ---
        for project in active_projects:
            label = f"smoke:{project}"
            print(f"  [{label}]")
            try:
                await test_smoke(context, project)
                results.append((label, True, ""))
                print(f"    PASS")
            except TestFailure as e:
                results.append((label, False, str(e)))
                print(f"    FAIL: {e}")

        # --- Button → Action (quick_scene only) ---
        if not filter_projects or "quick_scene" in filter_projects:
            label = "action:dance_button"
            print(f"  [{label}]")
            try:
                last_action = await test_button_fires_action(context)
                results.append((label, True, ""))
                print(f"    PASS  (last_action={last_action})")
            except TestFailure as e:
                results.append((label, False, str(e)))
                print(f"    FAIL: {e}")

        # --- Scene Transition (3rd_person_game_demo only) ---
        if not filter_projects or "3rd_person_game_demo" in filter_projects:
            label = "transition:start_game"
            print(f"  [{label}]")
            try:
                before, after = await test_scene_transition(context)
                results.append((label, True, ""))
                print(f"    PASS  ({before!r} -> {after!r})")
            except TestFailure as e:
                results.append((label, False, str(e)))
                print(f"    FAIL: {e}")

        # --- Scene baselines (one screenshot per .scene.ron per project) ---
        for project in active_projects:
            for scene in discover_scenes(project):
                key = scene_baseline_key(project, scene)
                label = f"scene_baseline:{key}"
                print(f"  [{label}]")
                update_this = update_baselines or key in update_baseline_projects
                try:
                    await test_screenshot_scene_baseline(
                        context, project, scene, screenshot_dir, update_this
                    )
                    results.append((label, True, ""))
                    print(f"    PASS")
                except TestFailure as e:
                    results.append((label, False, str(e)))
                    print(f"    FAIL: {e}")

        # --- Navigation: pause menu flow (3rd_person_game_demo only) ---
        if not filter_projects or "3rd_person_game_demo" in filter_projects:
            label = "navigation:pause_menu_flow"
            print(f"  [{label}]")
            update_nav = update_baselines or "pause_nav" in update_baseline_projects
            try:
                await test_pause_menu_navigation(context, screenshot_dir, update_nav)
                results.append((label, True, ""))
                print(f"    PASS")
            except TestFailure as e:
                results.append((label, False, str(e)))
                print(f"    FAIL: {e}")

        await browser.close()

    return results


def build_wasm() -> bool:
    print("[build] Running wasm-pack …")
    result = subprocess.run(
        ["wasm-pack", "build", "crates/ironhold_web", "--target", "web", "--out-dir", "../../pkg"],
    )
    if result.returncode != 0:
        print("[build] FAILED", file=sys.stderr)
        return False
    print("[build] OK")
    return True


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--update-baselines", action="store_true",
                        help="Overwrite all stored baselines")
    parser.add_argument("--update-baseline", metavar="TARGET", action="append", default=[],
                        dest="update_baseline_targets",
                        help="Overwrite baseline for a specific target (repeatable). "
                             "Accepted values: a project name (e.g. quick_scene), "
                             "'pause_nav', or a scene key "
                             "(e.g. quick_scene_main, 3rd_person_game_demo_start_menu).")
    parser.add_argument("--project", metavar="NAME", action="append", default=[],
                        dest="projects",
                        help="Limit tests to specific project(s) (repeatable). "
                             "Omit to run all projects.")
    parser.add_argument("--screenshot-dir", default="screenshots")
    gpu_group = parser.add_mutually_exclusive_group()
    gpu_group.add_argument("--webgpu", action="store_true",
                           help="Use SwiftShader WebGPU (headless); exercises the WebGPU code path "
                                "without requiring a display or physical GPU")
    gpu_group.add_argument("--real-gpu", action="store_true",
                           help="Use real GPU via non-headless Chromium; requires a display")
    args = parser.parse_args()

    screenshot_dir = Path(args.screenshot_dir)
    screenshot_dir.mkdir(exist_ok=True)
    update_baseline_projects: set[str] = set(args.update_baseline_targets)
    filter_projects: set[str] = set(args.projects)

    if not args.skip_build:
        if not build_wasm():
            sys.exit(1)

    if not Path("pkg/ironhold_web.js").exists():
        print("ERROR: pkg/ironhold_web.js not found — run wasm-pack first.", file=sys.stderr)
        sys.exit(1)

    print("[server] Starting dev server …")
    server = subprocess.Popen(
        [sys.executable, "serve.py"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(1)

    if args.real_gpu:
        chromium_args = CHROMIUM_ARGS_REAL_GPU
        headless = False
    elif args.webgpu:
        chromium_args = CHROMIUM_ARGS_WEBGPU
        headless = True
    else:
        chromium_args = CHROMIUM_ARGS_GL
        headless = True

    try:
        results = asyncio.run(run_all(screenshot_dir, args.update_baselines, update_baseline_projects, filter_projects, chromium_args, headless))
    finally:
        server.terminate()
        server.wait()

    print()
    passed = [r for r in results if r[1]]
    failed = [r for r in results if not r[1]]
    print(f"Results: {len(passed)}/{len(results)} passed")
    if failed:
        print("Failures:")
        for label, _, msg in failed:
            print(f"  • {label}: {msg}")
        sys.exit(1)


if __name__ == "__main__":
    main()
