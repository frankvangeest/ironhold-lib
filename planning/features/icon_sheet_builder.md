# Feature: Icon Sheet Builder

_Status: Draft_
_Planned at: `a6acab8` (2026-06-22)_

## What
A build-time Python tool (`tools/icon_sheet/build.py`) that takes a folder of individual
icon PNG files and stitches them into a **power-of-2 RGBA texture atlas** plus a sidecar
**manifest** that maps each icon's filename to its `(col, row)` cell — and, for copy-paste
convenience, its row-major `icon_index`. The generated atlas is a drop-in for any place the
engine already consumes an `icon_sheet` texture catalog key (`ActionBarDef`,
`InventoryPanelDef`, `ContainerPanelDef`, and `ItemDef.icon_sheet`).

Designers iterate on icons as loose files (`icons/fireball.png`, `icons/health.png`, …),
run the tool, and get a committed atlas they register in `assets.ron` like any other texture.
The grid dimensions the tool emits (`icon_cols` / `icon_rows` / `icon_cell_size`) are printed
so they can be pasted straight into the consuming RON def.

## Why
Every icon-consuming def today (`ActionBarDef`, `InventoryPanelDef`, `ContainerPanelDef`,
`ItemDef`) requires a **pre-made grid atlas** and a hand-counted `icon_index`. The constraints:

1. **WASM/WebGPU wants power-of-2 textures.** Loose designer icons are arbitrary sizes
   (48², 64², 96²). Without a tooling step the designer must hand-pack a power-of-2 sheet in
   an image editor and manually track which cell holds which icon.
2. **`icon_index` is a magic number.** Designers count cells by hand and hard-code an integer.
   Adding an icon mid-sheet renumbers everything after it.

This tool removes both pains while changing **nothing** about the runtime contract: the atlas
is still a plain texture key, the cells are still a uniform grid, and `icon_index` still works
exactly as it does now. It is purely additive authoring infrastructure that fits the existing
`tools/` pattern (`texture_gen`, `build_asset_manifest`) with **zero WASM impact**.

## Approach

### Build-time tool, not runtime stitching — decision

**Build-time Python tool. Decided.** Rationale:

- **Zero WASM cost.** Runtime stitching pulls the `image` crate (decode + resize + blit) into
  `ironhold_core`, inflating binary size (already tracked against the 100 MB GitHub Pages cap,
  90.7 MB as of last check) and adding startup latency on a target where we already fight
  WebGPU pipeline-compile stalls. A committed PNG loaded through the normal asset path is free.
- **Determinism.** A committed atlas is identical on every platform. Runtime stitching would
  have to produce byte-identical output on native and WASM to keep screenshot baselines stable.
- **Fits the established pattern.** `texture_gen` and `build_asset_manifest` are already
  build-time Python tools run from repo root and committed alongside their outputs. Designers
  already run `python tools/...` after asset edits.
- **Iteration cost is acceptable.** "Edit icon → rerun tool → refresh browser" is one command,
  comparable to the existing `build_asset_manifest` step designers already run after asset
  changes. The tool is fast (a few dozen small PNG blits).

The cost is that the atlas must be regenerated when icons change — mitigated by making the tool
idempotent (stable sort, deterministic packing) and adding it to the asset-tooling checklist.

### `icon_name` runtime resolution — deferred to v2

**v1 ships index-only. No schema changes, no runtime changes.** The manifest is a pure
authoring aid: the tool prints (and writes) the filename→index mapping, the designer copies the
index into the RON def's `icon_index`. This keeps v1 a tool-only change with no `ironhold_core`
edits, no schema-stability risk, and nothing for the CLI/query layer to learn.

`icon_name` runtime resolution (`ItemDef.icon_name: Option<String>`,
`ActionSlotDef.icon_name: Option<String>`, a `LoadedIconManifest` resource) is **explicitly
deferred** and captured under Open Questions / a follow-up backlog item. It is a real ergonomic
win but carries schema-stability and load-order weight that should be designed on its own once
the tool exists and we have feel for how designers reference icons. Designing it now would
violate "minimal architectural footprint."

### Power-of-2 sizing algorithm

```
inputs            = all *.png in the source folder, sorted by filename (stable, deterministic)
N                 = len(inputs)
cell_size         = --cell-size if given,
                    else next_pow2( max(width, height) over all inputs )      # square cells
padding           = --padding (default 2)                                     # transparent gutter
inner             = cell_size - 2 * padding                                    # usable icon area
cols              = next_pow2( ceil( sqrt(N) ) )
rows              = ceil( N / cols )                                           # may be < cols
atlas_w           = next_pow2( cols * cell_size )
atlas_h           = next_pow2( rows * cell_size )
```

- `next_pow2(x)` = smallest power of two ≥ x. `cell_size` is always a power of two so any
  power-of-two `cols`/`rows` keeps `atlas_w`/`atlas_h` powers of two.
- Each icon is centered in its cell's `inner` region (downscaled with high-quality resampling if
  larger than `inner`; never upscaled — smaller icons sit centered with surrounding padding).
- Empty trailing cells (when `N < cols * rows`) are left fully transparent RGBA `(0,0,0,0)`.
- Row-major fill: `index = col + row * cols`, top row first — matches the engine's existing
  `icon_index` convention exactly (`scene_v2.rs` `ActionSlotDef.icon_index` doc:
  "Row 0 = top row; index `col + row * icon_cols`").

**Cell padding for GPU bleed.** `--padding` (default 2 px) reserves a transparent gutter inside
every cell. WebGPU bilinear sampling at cell edges can bleed neighbouring texels; a transparent
gutter means any bleed pulls in `alpha=0` rather than the adjacent icon. The engine's UV math is
**unchanged** — it still samples the full `icon_cell_size × icon_cell_size` cell. The padding
lives *inside* the cell, so `icon_cell_size` reported to RON is the full padded cell size and the
existing renderer needs no edge-inset awareness. (Note: the consuming defs do not currently inset
UVs; a transparent gutter is the correct, renderer-agnostic mitigation precisely because it needs
no runtime change.)

### Tool CLI design

```
python tools/icon_sheet/build.py <source_dir> --output-key <key> [options]

Positional:
  <source_dir>            Folder of individual icon PNGs (e.g. assets/projects/foo/icons/abilities)

Required:
  --output-key <key>      Catalog key the atlas will be registered under (e.g. "icons_abilities").
                          Also names the output files: <key>.png + <key>.icons.ron

Options:
  --output-dir <dir>      Where to write the atlas + manifest.
                          Default: <source_dir> (atlas sits next to its source icons).
  --cell-size <px>        Force a fixed power-of-2 cell size. Default: next_pow2(largest input).
                          Errors if not a power of two.
  --padding <px>          Transparent gutter inside each cell. Default: 2.
  --manifest-format ron|json   Manifest format. Default: ron (matches engine convention).
  --update-assets <assets.ron> Optional: insert/replace the texture key in the given assets.ron
                          in place and print the diff. Default: off (prints the line to paste).
  --check                 Re-run packing in memory and fail (exit 1) if the committed atlas or
                          manifest is stale. For CI / pre-commit verification.
  --json                  Machine-readable summary on stdout (mirrors the CLI's --json convention).
```

**Outputs:**
1. `<output-dir>/<key>.png` — the power-of-2 RGBA atlas.
2. `<output-dir>/<key>.icons.ron` — the manifest (filename-stem → cell + index).
3. **stdout**: the grid dimensions to paste into the consuming def, and the `assets.ron`
   texture line to register the atlas.

**Multiple icon sets**: one invocation = one sheet. A project with abilities + items + UI runs
the tool three times with three `--output-key` values and three source folders. This is simpler
than a multi-set config file and matches how the existing tools are invoked per-output.

### `assets.ron` registration

The generated atlas is a normal texture entry — no new catalog type:

```ron
textures: {
    // Generated by tools/icon_sheet/build.py from icons/abilities/ — 4×4 grid, 64px cells, 256×256.
    "icons_abilities": "projects/foo/icons/abilities.png",
},
```

The tool **prints** this line by default and can **insert it in place** with
`--update-assets path/to/assets.ron` (idempotent: replaces the existing key if present). Default
is print-only so the designer stays in control of catalog edits, consistent with how the project
treats `assets.ron` as the hand-authored source of truth. After registration the designer runs
the existing `python tools/asset_checker/check.py` to confirm the path resolves.

## Tasks
- [ ] Create `tools/icon_sheet/build.py` — CLI parsing, scan source dir, validate inputs are PNG.
- [ ] Implement `next_pow2`, cell-size derivation, grid sizing, centered blit with padding (Pillow/NumPy, matching `texture_gen` deps).
- [ ] Deterministic ordering (sort by filename) so re-runs are byte-stable for `--check`.
- [ ] Write power-of-2 RGBA atlas PNG; leave trailing cells transparent.
- [ ] Write manifest (`.icons.ron` default; `--json` variant) mapping stem → `(col, row, index)`.
- [ ] Print paste-ready grid dims + `assets.ron` texture line; implement `--update-assets` in-place edit.
- [ ] Implement `--check` (stale-detection) and `--json` summary output.
- [ ] `tools/icon_sheet/CLAUDE.md` — usage, when to use, cell-size/padding guidance, regenerate-on-change note.
- [ ] `tools/icon_sheet/requirements.txt` (Pillow + numpy, mirroring `texture_gen`).
- [ ] Tests: a fixture folder of small PNGs + a test asserting atlas dimensions are power-of-2, cell count ≥ N, manifest indices match row-major layout, and `--check` passes on fresh output / fails on stale.
- [ ] Docs: add the tool to the root `CLAUDE.md` Tools table and the `assets/CLAUDE.md` tools section. Note in `docs/20_data_formats.md` (icon_sheet sections) that atlases can be generated rather than hand-packed.
- [ ] Add a follow-up backlog item for v2 `icon_name` runtime resolution (deferred).

_No `ironhold_core`, schema, or CLI (`query.rs`) changes in v1 — this is a tools-only feature._

## Open questions
- **v2 `icon_name` resolution shape**: if/when we add runtime name resolution, is the manifest
  loaded as a Bevy `Asset` (one `Handle<IconManifest>` per sheet) keyed by the same catalog key
  as the texture, or as a side-file the asset loader pairs by convention (`<key>.icons.ron`)?
  How does a slot/item declare *which* sheet's manifest to resolve against when both `icon_sheet`
  and `icon_name` are set? (Likely: resolve `icon_name` against the manifest paired with the
  effective `icon_sheet`.) Deferred — design when v1 ships.
- **Should `--cell-size` clamp/warn on very large sheets?** A folder of 96² icons with many
  entries can produce a 2048²+ atlas. Worth a soft warning at, say, 1024² and a hard note that
  WebGPU max texture size is device-dependent (commonly 8192²).
- **Stem collisions**: two source files with the same stem but different case/extension would
  collide in the manifest. v1 should error clearly rather than silently overwrite.

## Acceptance criteria
- **Given** a folder `assets/projects/foo/icons/abilities/` containing 5 icons
  (`fireball.png`, `heal.png`, `frost.png`, `shield.png`, `dash.png`), each 64×64,
  **when** the designer runs:
  ```bash
  python tools/icon_sheet/build.py assets/projects/foo/icons/abilities --output-key icons_abilities
  ```
  **then** the tool writes:
  - `assets/projects/foo/icons/abilities.png` — a **512×512** RGBA atlas (cell_size = `next_pow2(64)` = 64; cols = `next_pow2(ceil(sqrt(5)))` = `next_pow2(3)` = 4; rows = `ceil(5/4)` = 2; `atlas_w = next_pow2(4*64) = 256`… **see note**), with the 5 icons centered in the first 5 cells (row-major) and the remaining cells transparent.

    > Sizing note for the example: with cols=4, rows=2, raw size is 256×128. Both dimensions are
    > rounded up to a power of two **and** the atlas is emitted **square** so cell math is uniform,
    > giving **256×256** for this 5-icon set. The "512×512, 8×8 grid" shape in the original brief
    > corresponds to forcing an 8×8 layout (e.g. matching an existing `icons_items` 8×8 sheet via
    > `--cell-size 64` with a folder that fills 8 columns); the algorithm above auto-fits to the
    > smallest square power-of-two grid by default. Designers wanting a fixed 8×8 pass `--cell-size`
    > and the tool will pack into the next square power-of-two that holds N at that cell size.

  - `assets/projects/foo/icons/abilities.icons.ron` — a manifest:
    ```ron
    (
        cols: 4,
        rows: 2,
        cell_size: 64,
        padding: 2,
        icons: {
            "dash":     (col: 0, row: 0, index: 0),
            "fireball": (col: 1, row: 0, index: 1),
            "frost":    (col: 2, row: 0, index: 2),
            "heal":     (col: 3, row: 0, index: 3),
            "shield":   (col: 0, row: 1, index: 4),
        },
    )
    ```
    (entries ordered by filename stem; `index = col + row * cols`).
  - **stdout** prints the paste-ready grid + registration line:
    ```
    Atlas: abilities.png  (256x256, 4x2 grid, 64px cells, padding 2)
    Paste into your ActionBarDef / InventoryPanelDef:
        icon_sheet: "icons_abilities", icon_cols: 4, icon_rows: 2, icon_cell_size: 64
    Add to assets.ron textures:
        "icons_abilities": "projects/foo/icons/abilities.png",
    ```
- **Given** a designer pastes `icon_index: 1` into an `ActionSlotDef` with
  `icon_sheet: "icons_abilities"`, `icon_cols: 4`, `icon_cell_size: 64`,
  **then** the existing renderer displays the `fireball` icon with no engine changes — the atlas
  is consumed identically to a hand-made sheet.
- **Given** the committed atlas/manifest is up to date, **when** `--check` runs, **then** it
  exits 0; **when** an icon file is added/changed without regenerating, **then** `--check` exits 1
  and names the stale file.
- **Given** any generated atlas, **then** both atlas dimensions are powers of two (WASM/WebGPU
  safe) and every cell carries ≥ `padding` px of transparent gutter on all sides.
