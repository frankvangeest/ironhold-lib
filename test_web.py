"""Browser test suite for the Ironhold WASM / WebGPU build.

Runs five test categories against a locally served build:
  1. Smoke      — every project loads to InGame with no errors
  2. Action     — clicking a UI button fires the expected Action
  3. Transition — a LoadScene action transitions the scene correctly
  4. Scene      — per-scene screenshots are diffed against stored baselines
  5. Navigation — multi-step menu flows with per-step screenshots

Usage:
    python test_web.py [--skip-build] [--update-baselines] [--project NAME] [--screenshot-dir DIR]
    python test_web.py --responsive --real-gpu --skip-build   # viewport/device matrix instead

Options:
    --responsive        Run the responsive matrix (resize + touch + per-viewport baselines in
                        screenshot_baselines/responsive/) INSTEAD of the main suite
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
import math
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

# Default differs from serve.py's 8000 on purpose: test_web.py starts its own server and does
# not notice when the bind fails, so a leftover manual `python serve.py` on 8000 would silently
# serve the suite instead (confusing timeouts). Override with --port.
PORT = 8001
BASE_URL = f"http://localhost:{PORT}"

PROJECTS = ["quick_scene", "3rd_person_game_demo", "terrain_demo", "custom_materials", "primitive_world", "entity_logic_demo", "particles_demo", "effect_mayhem_demo", "foliage_demo", "stats_demo", "blank_project", "ui_demo", "local_coop_demo", "camera_modes", "dynamic_animation_control", "first_person_starter"]

# Root of the asset tree, relative to the working directory where this script runs.
ASSETS_ROOT = Path("assets/projects")

# Scenes excluded from the screenshot-baseline diff (they never converge to a fixed pixel
# state — e.g. a scene that intentionally keeps an animation playing rather than freezing it —
# so any committed baseline would flake against BASELINE_DIFF_THRESHOLD regardless of run count).
# Keyed the same way as PROJECTS: "project/relative/scene/path.scene.ron".
NON_DETERMINISTIC_SCENES = {
    "dynamic_animation_control/scenes/continue.scene.ron",
}

# Committed baseline screenshots.  Two sub-dirs:
#   scenes/     — one PNG per scene (used by the gallery)
#   pause_nav/  — one PNG per navigation step
# Current/comparison screenshots go in the --screenshot-dir (gitignored).
BASELINE_DIR = Path("screenshot_baselines")

# --responsive mode: a small viewport/device matrix, kept out of the main suite (it would multiply
# a ~20-minute run). Baselines live in their own folder so the gallery (which reads scenes/) never
# picks them up. Entries: (label, viewport width, viewport height, mobile).
# `mobile` turns on Chromium's mobile emulation and touch. Captured and compared with --real-gpu only,
# like every other baseline. See planning/features/responsive_ui.md ("Playwright screenshot impact").
RESPONSIVE_BASELINE_DIR = BASELINE_DIR / "responsive"
#
# There is deliberately NO device_scale_factor > 1 entry here: Playwright's `device_scale_factor`
# emulation makes ResizeObserver's devicePixelContentBoxSize report the CSS size (verified: an 800x600
# element reports [800,600] at DSF 1, 2 and 3), which is what winit's web backend reads for the
# physical canvas size, so the engine then sees a window of CSS/dpr. That is an emulation artifact (a
# real display reports CSS*dpr). Real HiDPI is covered by `test_real_hidpi`, which launches Chromium
# with `--force-device-scale-factor` instead.
RESPONSIVE_MATRIX = [
    ("1920x1080",      1920, 1080, False),   # PC
    ("1024x768",       1024,  768, False),   # tablet, landscape
    ("768x1024",        768, 1024, False),   # tablet, portrait
    ("844x390",         844,  390, True),    # phone, landscape
    ("390x844",         390,  844, True),    # phone, portrait
]
REAL_HIDPI_FACTORS = (2, 3)
# (project, scene) pairs the matrix runs. `ui_demo` is the Group-layout showcase; the 3rd_person
# main scene is the real HUD (its baselines change when that HUD is retrofitted onto `Group`).
RESPONSIVE_TARGETS = [
    ("ui_demo", "scenes/main.scene.ron"),
    ("3rd_person_game_demo", "scenes/main.scene.ron"),
]

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
    relative_paths = sorted(
        str(p.relative_to(ASSETS_ROOT / project)).replace("\\", "/")
        for p in scenes_dir.glob("*.scene.ron")
    )
    return [
        rel for rel in relative_paths
        if f"{project}/{rel}" not in NON_DETERMINISTIC_SCENES
    ]


async def open_project(
    context: BrowserContext,
    project: str | None = None,
    scene: str | None = None,
    extra_query: str = "",
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
    url += extra_query
    await page.goto(url, wait_until="networkidle")

    # Ready = the engine's first frame has run (play.html sets `data-ready` on <body> when the
    # WASM runtime first writes #debug-state). A <canvas> existing is not that signal: it can be
    # present before the wasm has loaded.
    try:
        await page.wait_for_selector("body[data-ready]", state="attached", timeout=CANVAS_TIMEOUT * 1000)
    except Exception:
        raise TestFailure(
            f"Timed out waiting for the engine to become ready (body[data-ready]) on project '{project or 'default'}'"
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
    Clicking 'Start Game' → LoadScene("scenes/character_select.scene.ron").
    Clicking a character on that screen → LoadScene("scenes/main.scene.ron").
    """
    page, errors = await open_project(context, "3rd_person_game_demo")
    try:
        # Wait for start_menu to fully load
        start_state = await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
        initial_scene = start_state.get("scene", "")

        # Trigger the scene load.
        # Start Game button: position=(100,100) size=(300,65) → center=(250,132)
        await page.mouse.click(250, 132)

        # Wait for character_select to become ready in InGame
        select_state = await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame" and s.get("scene", "") != initial_scene,
            timeout_s=CANVAS_TIMEOUT,
        )
        if "character_select" not in select_state.get("scene", ""):
            raise TestFailure(
                f"Expected scene to contain 'character_select', got: {select_state.get('scene')}"
            )

        # Select Male Warrior button: position=(360,520) size=(230,56) → center=(475,548)
        await page.mouse.click(475, 548)

        # Wait for a different scene to become ready in InGame
        final_state = await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame" and s.get("scene", "") != select_state.get("scene", ""),
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

        # Checked before either the create/update path or the diff path returns/raises — a scene
        # getting its baseline created for the first time still deserves its console-error check;
        # skipping it here previously meant a new project's very first baseline run (exactly what
        # happens when a project is added) verified nothing about console output.
        if errors:
            raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))

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

    Resume button (ui_panel centered layout, panel width=320 padding=30 gap=16).
    Panel content: title(50) + resume(65) + audio_state label(36) + mute button(65) + quit(65),
    each separated by a 16 gap — the audio_state label and mute button were added by the audio
    mute toggle feature after this math was first computed; recompute here if pause.scene.ron's
    ui list changes again.
      Panel height = 30 + (50+16+65+16+36+16+65+16+65) + 30 = 405 → top = (720-405)/2 = 157.5
      Resume center y = 157.5 + 30 + 50 + 16 + 65/2 = 286
      Resume center x = 1280/2 = 640 → click (640, 286)
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

        # ── Step 2: Start Game → character select → main scene ─────────────
        state = await wait_for_debug_state(page, lambda s: True)
        frame = state["frame"]
        await page.mouse.click(250, 132)   # Start Game button
        await wait_for_debug_state(
            page,
            lambda s: s.get("app_state") == "InGame" and "character_select" in s.get("scene", ""),
            timeout_s=CANVAS_TIMEOUT,
        )
        await page.mouse.click(475, 548)   # Male Warrior button
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
        await page.mouse.click(640, 286)   # Resume button (centered panel layout)
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
    executable_path: str | None = None,
) -> list[tuple[str, bool, str]]:
    results: list[tuple[str, bool, str]] = []
    # When --project is given, restrict to those projects; otherwise run all.
    active_projects = [p for p in PROJECTS if not filter_projects or p in filter_projects]

    async with async_playwright() as pw:
        browser: Browser = await pw.chromium.launch(
            headless=headless,
            args=chromium_args or CHROMIUM_ARGS_GL,
            executable_path=executable_path,
        )
        context: BrowserContext = await browser.new_context(viewport={"width": 1280, "height": 720})

        # --- Smoke tests ---
        for project in active_projects:
            label = f"smoke:{project}"
            print(f"  [{label}]")
            try:
                await test_smoke(context, project)
                results.append((label, True, ""))
                print(f"    PASS")
            except Exception as e:
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
            except Exception as e:
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
            except Exception as e:
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
                except Exception as e:
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
            except Exception as e:
                results.append((label, False, str(e)))
                print(f"    FAIL: {e}")

        await browser.close()

    return results


# ---------------------------------------------------------------------------
# --responsive mode
# ---------------------------------------------------------------------------

CANVAS_RECT_JS = """() => {
  const all = document.querySelectorAll('canvas');
  const c = document.getElementById('ironhold-canvas');
  const r = c.getBoundingClientRect();
  return {count: all.length, x: r.x, y: r.y, w: r.width, h: r.height,
          scrollH: document.documentElement.scrollHeight, scrollW: document.documentElement.scrollWidth};
}"""


async def assert_canvas_fills_viewport(page: Page, w: int, h: int) -> None:
    """The single canvas is exactly the viewport (the bar is hidden under ?testing=1), no scrolling."""
    r = await page.evaluate(CANVAS_RECT_JS)
    if r["count"] != 1:
        raise TestFailure(f"expected exactly 1 <canvas>, found {r['count']} (stale pkg/? see docs/dev/browser_tests.md)")
    if abs(r["w"] - w) > 1 or abs(r["h"] - h) > 1 or abs(r["x"]) > 1 or abs(r["y"]) > 1:
        raise TestFailure(f"canvas is {r['w']:.0f}x{r['h']:.0f} at ({r['x']:.0f},{r['y']:.0f}), expected {w}x{h} at (0,0)")
    if r["scrollH"] > h + 1 or r["scrollW"] > w + 1:
        raise TestFailure(f"page scrolls: {r['scrollW']}x{r['scrollH']} for a {w}x{h} viewport")


# Projects in the responsive matrix that set a `ui_scale` block, and its reference. Mirrors the engine's
# formula (planning/features/responsive_ui.md Phase 2) independently, so a drift on either side fails.
# It hardcodes the DEFAULT fit/snap/min/max: if a matrix project ever sets those fields the test fails
# loudly (rather than drifting silently), and this mirror must grow the same fields.
UI_SCALE_REFERENCES = {"ui_demo": (1280.0, 720.0)}


def expected_ui_scale(project: str, w: float, h: float) -> float:
    ref = UI_SCALE_REFERENCES.get(project)
    if ref is None:
        return 1.0
    raw = min(w / ref[0], h / ref[1])                       # fit: Fit
    snapped = math.floor(raw / 0.25 + 1e-4) * 0.25          # snap DOWN to 0.25
    return min(max(snapped, 0.5), 2.0)                      # min_scale / max_scale


def assert_debug_window(state: dict, w: int, h: int, ui_scale: float = 1.0) -> None:
    """`window_size` (logical) and `ui_scale` from #debug-state match the viewport / 1.0."""
    if "window_size" not in state or "ui_scale" not in state:
        raise TestFailure("debug state has no window_size/ui_scale: stale pkg/ (rebuild)")
    ww, wh = state["window_size"]
    if abs(ww - w) > 1 or abs(wh - h) > 1:
        raise TestFailure(f"engine window_size is {ww}x{wh}, expected {w}x{h}")
    if abs(state["ui_scale"] - ui_scale) > 1e-4:
        raise TestFailure(f"ui_scale is {state['ui_scale']}, expected {ui_scale} for a {w}x{h} window")


async def test_responsive_entry(
    browser: Browser,
    project: str,
    scene: str,
    label: str,
    width: int,
    height: int,
    mobile: bool,
    screenshot_dir: Path,
    update: bool,
) -> None:
    context = await browser.new_context(
        viewport={"width": width, "height": height},
        is_mobile=mobile,
        has_touch=mobile,
    )
    try:
        page, errors = await open_project(context, project, scene=scene)
        try:
            await wait_for_scene_ready(page)
            state = await wait_for_debug_state(
                page, lambda s: "window_size" not in s or abs(s["window_size"][0] - width) <= 1   # a stale pkg (no field) falls through to assert_debug_window's clear message
            )
            assert_debug_window(state, width, height, expected_ui_scale(project, width, height))
            await assert_canvas_fills_viewport(page, width, height)

            key = f"{scene_baseline_key(project, scene)}_{label}"
            current_path = screenshot_dir / f"responsive_{key}_current.png"
            baseline_path = RESPONSIVE_BASELINE_DIR / f"{key}.png"
            await page.screenshot(path=str(current_path))
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))

            if update or not baseline_path.exists():
                RESPONSIVE_BASELINE_DIR.mkdir(parents=True, exist_ok=True)
                created = not baseline_path.exists()
                shutil.copy(current_path, baseline_path)
                print(f"    Baseline {'created' if created else 'updated'}: {baseline_path.name}")
                return
            diff = compare_screenshots(str(baseline_path), str(current_path))
            if diff > BASELINE_DIFF_THRESHOLD:
                raise TestFailure(
                    f"Screenshot diff {diff:.1%} exceeds {BASELINE_DIFF_THRESHOLD:.0%} threshold. Inspect: {current_path}"
                )
            print(f"    Diff: {diff:.2%} (threshold {BASELINE_DIFF_THRESHOLD:.0%}) — OK")
        finally:
            await page.close()
    finally:
        await context.close()


async def test_resize_follows_viewport(browser: Browser) -> None:
    """One page, several viewport sizes: the engine's window and the canvas both follow each resize."""
    context = await browser.new_context(viewport={"width": 1280, "height": 720})
    try:
        page, errors = await open_project(context, "ui_demo")
        try:
            await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            for w, h in ((800, 600), (1920, 1080), (390, 844), (844, 390), (1280, 720)):
                await page.set_viewport_size({"width": w, "height": h})
                state = await wait_for_debug_state(
                    page,
                    lambda s, w=w, h=h: "window_size" not in s
                    or (abs(s["window_size"][0] - w) <= 1 and abs(s["window_size"][1] - h) <= 1),
                )
                assert_debug_window(state, w, h, expected_ui_scale("ui_demo", w, h))
                await assert_canvas_fills_viewport(page, w, h)
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
        finally:
            await page.close()
    finally:
        await context.close()


async def test_touch_tap_fires_action(browser: Browser) -> None:
    """A touch tap (not a mouse click) on quick_scene's Dance button must fire its action.

    Settles whether taps reach Bevy UI `Interaction`: nothing in core reads `Touches`, so this relies
    on bevy_ui's own touch handling. A failure here is an input finding for the mobile backlog items,
    not a layout regression.
    """
    context = await browser.new_context(viewport={"width": 1280, "height": 720}, has_touch=True)
    try:
        page, errors = await open_project(context, "quick_scene")
        try:
            await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            await page.touchscreen.tap(95, 80)   # Dance button centre, as test_button_fires_action
            state = await wait_for_debug_state(page, lambda s: "PlayAnimation" in s.get("last_action", ""))
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
            return state["last_action"]
        finally:
            await page.close()
    finally:
        await context.close()


async def test_real_hidpi(
    pw,
    factor: int,
    chromium_args: list[str] | None,
    headless: bool,
    executable_path: str | None,
) -> str:
    """On a real (not emulated) device scale factor the engine must see the CSS-sized window and the
    canvas backing store must be CSS * dpr (sharp), not CSS-sized.

    Chromium is launched with `--force-device-scale-factor=N` and `no_viewport=True` (no CDP
    emulation: Playwright's `device_scale_factor` makes ResizeObserver report CSS-sized
    devicePixelContentBoxSize, which makes winit see a CSS/dpr window; that is an emulation artifact).
    The window size is whatever the browser opens with, so everything is measured in-page.
    """
    browser = await pw.chromium.launch(
        headless=headless,
        args=(chromium_args or CHROMIUM_ARGS_GL) + [f"--force-device-scale-factor={factor}"],
        executable_path=executable_path,
    )
    try:
        context = await browser.new_context(no_viewport=True)
        # `px_budget=off`: this test checks the *uncapped* path (backing store == CSS x dpr). The render-budget cap
        # (play.html, k > 1 above 3.69 MP) is covered by `test_render_budget`; at DPR 3 the default window would hit it.
        page, errors = await open_project(context, "ui_demo", extra_query="&px_budget=off")
        try:
            await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            await asyncio.sleep(2)   # let a post-load resize settle
            m = await page.evaluate("""() => {
              const c = document.getElementById('ironhold-canvas'); const r = c.getBoundingClientRect();
              return {dpr: devicePixelRatio, innerW: innerWidth, innerH: innerHeight,
                      cssW: r.width, cssH: r.height, backW: c.width, backH: c.height};
            }""")
            state = await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            if "window_size" not in state:
                raise TestFailure("debug state has no window_size: stale pkg/ (rebuild)")
            ww, wh = state["window_size"]
            if abs(m["dpr"] - factor) > 0.01:
                raise TestFailure(f"browser reports devicePixelRatio {m['dpr']}, expected {factor}")
            if abs(ww - m["cssW"]) > 1 or abs(wh - m["cssH"]) > 1:
                raise TestFailure(f"engine window_size {ww}x{wh} != canvas CSS size {m['cssW']:.0f}x{m['cssH']:.0f}")
            want = expected_ui_scale("ui_demo", ww, wh)
            if abs(state["ui_scale"] - want) > 1e-4:
                raise TestFailure(f"ui_scale {state['ui_scale']} != {want} for a {ww:.0f}x{wh:.0f} window at DPR {factor}")
            if abs(m["backW"] - m["cssW"] * factor) > factor or abs(m["backH"] - m["cssH"] * factor) > factor:
                raise TestFailure(
                    f"canvas backing store {m['backW']}x{m['backH']} is not CSS x {factor} "
                    f"({m['cssW']:.0f}x{m['cssH']:.0f}): UI would be blurry or mis-scaled"
                )
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  • {e}" for e in errors))
            return f"css {m['cssW']:.0f}x{m['cssH']:.0f}, backing {m['backW']}x{m['backH']}, window_size {ww:.0f}x{wh:.0f}"
        finally:
            await page.close()
            await context.close()
    finally:
        await browser.close()


RENDER_BUDGET_PX = 3686400   # play.html BUDGET_PX (2560x1440)


async def test_render_budget(browser: Browser) -> str:
    """Above the pixel budget play.html CSS-scales the canvas's parent so the backing store (CSS x dpr, measured
    before the transform) stays within budget while the picture still fills the viewport; at or under the budget
    nothing is scaled (k == 1). Uses viewport-size emulation only (DPR 1): `device_scale_factor` emulation makes
    ResizeObserver report CSS-sized content boxes, so it cannot be used to check DPR behaviour (see
    docs/dev/browser_tests.md). Real DPR coverage is part of the manual playtest."""
    context = await browser.new_context(viewport={"width": 3840, "height": 2160})
    try:
        page, errors = await open_project(context, "ui_demo")
        try:
            await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            await asyncio.sleep(2)   # let the post-load resize settle
            m = await page.evaluate("""() => {
              const c = document.getElementById('ironhold-canvas');
              return {k: window.ironholdRenderScale, backW: c.width, backH: c.height, dpr: devicePixelRatio};
            }""")
            if m["k"] is None or m["k"] <= 1.0:
                raise TestFailure(f"3840x2160 is over the budget but ironholdRenderScale is {m['k']}")
            back = m["backW"] * m["backH"]
            # Device-pixel snapping: allow a small tolerance over the budget.
            if back > RENDER_BUDGET_PX * 1.02:
                raise TestFailure(f"canvas backing store {m['backW']}x{m['backH']} = {back} px exceeds the budget {RENDER_BUDGET_PX}")
            await assert_canvas_fills_viewport(page, 3840, 2160)   # transformed rect still fills the viewport
            state = await wait_for_debug_state(page, lambda s: "window_size" in s)
            ww, wh = state["window_size"]
            if abs(ww * m["k"] - 3840) > 2 or abs(wh * m["k"] - 2160) > 2:
                raise TestFailure(f"engine window_size {ww}x{wh} x k={m['k']} != 3840x2160")
            # Back under the budget: no scaling.
            await page.set_viewport_size({"width": 1920, "height": 1080})
            await page.wait_for_function("window.ironholdRenderScale === 1")
            await assert_canvas_fills_viewport(page, 1920, 1080)
            # The engine and the backing store follow back to the uncapped size, not just the CSS rect.
            state = await wait_for_debug_state(
                page, lambda s: "window_size" in s and abs(s["window_size"][0] - 1920) <= 1 and abs(s["window_size"][1] - 1080) <= 1)
            back = await page.evaluate("() => { const c = document.getElementById('ironhold-canvas'); return [c.width, c.height]; }")
            if abs(back[0] - 1920) > 1 or abs(back[1] - 1080) > 1:
                raise TestFailure(f"backing store {back[0]}x{back[1]} did not return to 1920x1080 once back under the budget")
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  \u2022 {e}" for e in errors))
            return f"k={m['k']:.2f}, backing {m['backW']}x{m['backH']}, window_size {ww:.0f}x{wh:.0f}"
        finally:
            await page.close()
    finally:
        await context.close()


async def test_render_budget_pointer(browser: Browser) -> str:
    """Pointer mapping under the render-budget CSS scale: at k > 1 a click at the *visual* position of a UI button must
    still hit it (winit reads element-local offsetX/offsetY, which is untransformed, so the engine's logical coordinates
    are the visual ones divided by k). quick_scene's Dance button is top-left anchored at logical (95, 80)."""
    context = await browser.new_context(viewport={"width": 3840, "height": 2160})
    try:
        page, errors = await open_project(context, "quick_scene")
        try:
            await wait_for_debug_state(page, lambda s: s.get("app_state") == "InGame")
            await asyncio.sleep(2)   # let the post-load resize settle
            k = await page.evaluate("window.ironholdRenderScale")
            if k is None or k <= 1.0:
                raise TestFailure(f"3840x2160 is over the budget but ironholdRenderScale is {k}")
            await page.mouse.click(95 * k, 80 * k)
            state = await wait_for_debug_state(page, lambda s: "PlayAnimation" in s.get("last_action", ""))
            if errors:
                raise TestFailure("Browser errors:\n" + "\n".join(f"  \u2022 {e}" for e in errors))
            return f"k={k:.2f}, click at ({95 * k:.0f},{80 * k:.0f}) -> {state['last_action']}"
        finally:
            await page.close()
    finally:
        await context.close()


async def run_responsive(
    screenshot_dir: Path,
    update_baselines: bool,
    update_targets: set[str],
    filter_projects: set[str],
    chromium_args: list[str] | None = None,
    headless: bool = True,
    executable_path: str | None = None,
) -> list[tuple[str, bool, str]]:
    results: list[tuple[str, bool, str]] = []

    async def record(label: str, coro) -> None:
        print(f"  [{label}]")
        try:
            extra = await coro
            results.append((label, True, ""))
            print(f"    PASS" + (f"  ({extra})" if extra else ""))
        except Exception as e:
            results.append((label, False, str(e)))
            print(f"    FAIL: {e}")

    async with async_playwright() as pw:
        browser: Browser = await pw.chromium.launch(
            headless=headless,
            args=chromium_args or CHROMIUM_ARGS_GL,
            executable_path=executable_path,
        )
        await record("responsive:resize_follows_viewport", test_resize_follows_viewport(browser))
        await record("responsive:touch_tap_fires_action", test_touch_tap_fires_action(browser))
        await record("responsive:render_budget", test_render_budget(browser))
        await record("responsive:render_budget_pointer", test_render_budget_pointer(browser))
        for project, scene in RESPONSIVE_TARGETS:
            if filter_projects and project not in filter_projects:
                continue
            for label, w, h, mobile in RESPONSIVE_MATRIX:
                key = f"{scene_baseline_key(project, scene)}_{label}"
                update_this = update_baselines or "responsive" in update_targets or project in update_targets or key in update_targets
                await record(
                    f"responsive:{key}",
                    test_responsive_entry(browser, project, scene, label, w, h, mobile,
                                          screenshot_dir, update_this),
                )
        await browser.close()

        # Real HiDPI: separate browsers, one per forced device scale factor (see RESPONSIVE_MATRIX note).
        for factor in REAL_HIDPI_FACTORS:
            await record(
                f"responsive:real_hidpi_x{factor}",
                test_real_hidpi(pw, factor, chromium_args, headless, executable_path),
            )
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
    global PORT, BASE_URL
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--port", type=int, default=PORT,
                        help="Port for the suite's own dev server (default: %(default)s)")
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
    parser.add_argument("--responsive", action="store_true",
                        help="Run the responsive viewport/device matrix (plus the resize and touch "
                             "tests) INSTEAD of the main suite. Baselines: screenshot_baselines/"
                             "responsive/. --update-baseline accepts 'responsive' (all) or an entry "
                             "key such as ui_demo_main_390x844; --project limits the projects.")
    parser.add_argument("--screenshot-dir", default="screenshots")
    gpu_group = parser.add_mutually_exclusive_group()
    gpu_group.add_argument("--webgpu", action="store_true",
                           help="Use SwiftShader WebGPU (headless); exercises the WebGPU code path "
                                "without requiring a display or physical GPU")
    gpu_group.add_argument("--real-gpu", action="store_true",
                           help="Use real GPU via non-headless Chromium; requires a display")
    parser.add_argument("--browser-path", metavar="PATH", default=None,
                        help="Launch a specific installed browser binary (e.g. a portable Brave "
                             "install) instead of Playwright's bundled Chromium. Implies "
                             "--real-gpu (non-headless, real GPU args) unless --webgpu is also "
                             "given explicitly.")
    args = parser.parse_args()
    PORT = args.port
    BASE_URL = f"http://localhost:{PORT}"

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
        [sys.executable, "serve.py", str(PORT)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(1)

    if args.webgpu:
        chromium_args = CHROMIUM_ARGS_WEBGPU
        headless = True
    elif args.real_gpu or args.browser_path:
        chromium_args = CHROMIUM_ARGS_REAL_GPU
        headless = False
    else:
        chromium_args = CHROMIUM_ARGS_GL
        headless = True

    runner = run_responsive if args.responsive else run_all
    try:
        results = asyncio.run(runner(
            screenshot_dir, args.update_baselines, update_baseline_projects, filter_projects,
            chromium_args, headless, executable_path=args.browser_path,
        ))
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
