#!/usr/bin/env python3
"""
GLB Splitter — split a monolithic GLB into a mesh-only file and named animation-group files.

Usage:
    # List animations and their auto-detected prefixes:
    python tools/glb_splitter/split.py character.glb --list

    # Auto-group by name prefix (locomotion_*, combat_*, ...):
    python tools/glb_splitter/split.py character.glb --by-prefix

    # Explicit groups (comma-separated clip names per group):
    python tools/glb_splitter/split.py character.glb \\
        --group locomotion walk,run,idle,jump \\
        --group combat attack,dodge,die

    # Write groups only, keep the original GLB as-is for the mesh:
    python tools/glb_splitter/split.py character.glb --by-prefix --no-mesh

Output files (written next to the source by default, or in --out-dir):
    character_mesh.glb          mesh + skeleton, no animations
    character_locomotion.glb    animation-only (skeleton retained for binding)
    character_combat.glb        ...
"""

import argparse
import copy
import sys
from pathlib import Path

try:
    from pygltflib import GLTF2
except ImportError:
    print("pygltflib not installed.  Run: pip install pygltflib", file=sys.stderr)
    sys.exit(1)


# ── name helpers ───────────────────────────────────────────────────────────────

def strip_blender_prefix(name: str) -> str:
    """Remove the 'ObjectName|' prefix that Blender's glTF exporter adds."""
    return name.split('|', 1)[-1] if '|' in name else name


def auto_prefix(name: str) -> str:
    """Derive a group key from an animation name (lower-cased first token)."""
    clean = strip_blender_prefix(name or '')
    for sep in ('_', '.', ' '):
        if sep in clean:
            return clean.split(sep, 1)[0].lower()
    return clean.lower() or 'default'


# ── accessor / buffer helpers ──────────────────────────────────────────────────

def _attr_accessor_set(attrs) -> set:
    """Collect non-None accessor indices from an Attributes object or plain dict."""
    if isinstance(attrs, dict):
        return {v for v in attrs.values() if isinstance(v, int)}
    return {getattr(attrs, k) for k in vars(attrs) if isinstance(getattr(attrs, k), int)}


def skin_accessor_set(gltf: GLTF2) -> set:
    return {s.inverseBindMatrices for s in gltf.skins
            if s.inverseBindMatrices is not None}


def anim_accessor_set(gltf: GLTF2, anim_indices) -> set:
    s = set()
    for i in anim_indices:
        for samp in gltf.animations[i].samplers:
            s.add(samp.input)
            s.add(samp.output)
    return s


def compact_buffer(gltf: GLTF2, keep_acc: set) -> dict:
    """
    Rebuild the binary buffer keeping only the bufferViews used by keep_acc.
    Mutates gltf in-place.  Returns {old_accessor_idx: new_accessor_idx}.
    Assumes a single embedded buffer (standard for GLB).
    """
    blob = gltf.binary_blob() or b''

    # Determine which bufferViews to retain
    keep_bv = {gltf.accessors[i].bufferView for i in keep_acc
               if gltf.accessors[i].bufferView is not None}

    # Build bufferView old->new index map; shallow-copy each kept view
    bv_map: dict = {}
    new_bvs = []
    for old_i, bv in enumerate(gltf.bufferViews):
        if old_i in keep_bv:
            bv_map[old_i] = len(new_bvs)
            new_bvs.append(copy.copy(bv))

    # Rebuild binary blob: copy each kept bufferView's byte range, 4-byte aligned
    new_blob = bytearray()
    for new_bv in new_bvs:
        pad = (4 - len(new_blob) % 4) % 4
        new_blob.extend(b'\x00' * pad)
        src_offset = new_bv.byteOffset        # original offset — read before overwriting
        new_bv.byteOffset = len(new_blob)     # new offset in compacted buffer
        new_blob.extend(blob[src_offset: src_offset + new_bv.byteLength])

    # Build accessor old->new index map
    acc_map: dict = {}
    new_accs = []
    for old_i, acc in enumerate(gltf.accessors):
        if old_i in keep_acc:
            acc_map[old_i] = len(new_accs)
            new_acc = copy.copy(acc)
            if new_acc.bufferView is not None:
                new_acc.bufferView = bv_map[new_acc.bufferView]
            new_accs.append(new_acc)

    gltf.bufferViews = new_bvs
    gltf.accessors = new_accs
    if gltf.buffers:
        gltf.buffers[0].byteLength = len(new_blob)
    gltf.set_binary_blob(bytes(new_blob))
    return acc_map


def _remap_anim_accessors(gltf: GLTF2, acc_map: dict) -> None:
    for anim in gltf.animations:
        for samp in anim.samplers:
            samp.input = acc_map[samp.input]
            samp.output = acc_map[samp.output]


def _remap_skin_accessors(gltf: GLTF2, acc_map: dict) -> None:
    for skin in gltf.skins:
        if skin.inverseBindMatrices is not None:
            skin.inverseBindMatrices = acc_map[skin.inverseBindMatrices]


# ── split operations ───────────────────────────────────────────────────────────

def make_mesh_only(gltf: GLTF2) -> GLTF2:
    """
    Return a deep copy with all animations stripped.
    The binary buffer is unchanged (may retain unused animation bytes — acceptable for v1).
    """
    g = copy.deepcopy(gltf)
    g.animations = []
    return g


def make_anim_group(gltf: GLTF2, anim_indices: list, group_name: str) -> GLTF2:
    """
    Return a deep copy containing only the specified animations plus the skeleton.
    Mesh geometry, materials, and textures are stripped.
    The binary buffer is compacted to contain only skin + animation data.
    """
    g = copy.deepcopy(gltf)

    # Keep only target animations (preserve relative order)
    g.animations = [g.animations[i] for i in sorted(anim_indices)]

    # Strip mesh references from nodes; keep node hierarchy + names for animation targeting
    for node in g.nodes:
        node.mesh = None
        node.weights = None

    # Remove mesh / material / texture assets
    g.meshes = []
    g.materials = []
    g.textures = []
    g.images = []
    g.samplers = []   # texture samplers (not animation samplers)

    # Compact buffer: retain only skin inverse-bind-matrix + animation key/value accessors
    keep_acc = (skin_accessor_set(g)
                | anim_accessor_set(g, list(range(len(g.animations)))))

    if keep_acc:
        acc_map = compact_buffer(g, keep_acc)
        _remap_skin_accessors(g, acc_map)
        _remap_anim_accessors(g, acc_map)
    else:
        # No accessor data at all (unusual but valid)
        g.accessors = []
        g.bufferViews = []
        g.buffers = []
        g.set_binary_blob(b'')

    return g


# ── CLI ────────────────────────────────────────────────────────────────────────

def _print_animation_list(gltf: GLTF2) -> None:
    if not gltf.animations:
        print("No animations in this file.")
        return
    print(f"{'#':<4}  {'Auto-prefix':<20}  Name")
    print('-' * 64)
    for i, anim in enumerate(gltf.animations):
        name = anim.name or f'(unnamed_{i})'
        print(f"{i:<4}  {auto_prefix(name):<20}  {name}")


def _sanitize_filename(name: str) -> str:
    """Convert an animation name to a safe filename stem (lowercase, spaces to underscores)."""
    clean = strip_blender_prefix(name)
    clean = clean.lower()
    for ch in (' ', '-', '/', '\\', ':', '*', '?', '"', '<', '>', '|'):
        clean = clean.replace(ch, '_')
    # Collapse multiple underscores
    while '__' in clean:
        clean = clean.replace('__', '_')
    return clean.strip('_') or 'anim'


def main() -> None:
    ap = argparse.ArgumentParser(
        description="Split a GLB into a mesh-only file and animation-group files.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    ap.add_argument("input", help="Source .glb file")
    ap.add_argument("--list", action="store_true",
                    help="Print animation names and auto-prefixes, then exit")
    ap.add_argument("--one-per-clip", action="store_true",
                    help="Write one GLB per animation clip (one file per clip, named by clip)")
    ap.add_argument("--by-prefix", action="store_true",
                    help="Auto-group animations by name prefix (part before first _ or .)")
    ap.add_argument("--group", nargs=2, metavar=("NAME", "CLIPS"), action="append",
                    default=[],
                    help="NAME: group label; CLIPS: comma-separated clip names (repeatable)")
    ap.add_argument("--out-dir", metavar="DIR",
                    help="Output directory (default: same directory as input file)")
    ap.add_argument("--mesh-suffix", default="_mesh",
                    help="Suffix appended to the stem for the mesh-only file (default: _mesh)")
    ap.add_argument("--no-mesh", action="store_true",
                    help="Skip generating the mesh-only file")
    args = ap.parse_args()

    src = Path(args.input).resolve()
    if not src.exists():
        ap.error(f"File not found: {src}")

    out_dir = Path(args.out_dir).resolve() if args.out_dir else src.parent
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"Loading {src.name} ...")
    gltf = GLTF2().load(str(src))

    if args.list:
        _print_animation_list(gltf)
        return

    # ── build groups ──────────────────────────────────────────────────────────
    groups: dict = {}

    if args.one_per_clip:
        for i, anim in enumerate(gltf.animations):
            name = anim.name or f'anim{i}'
            key = _sanitize_filename(name)
            # Avoid collisions if two clips sanitize to the same name
            if key in groups:
                key = f"{key}_{i}"
            groups[key] = [i]

    if args.by_prefix:
        for i, anim in enumerate(gltf.animations):
            prefix = auto_prefix(anim.name or f'anim{i}')
            groups.setdefault(prefix, []).append(i)

    if args.group:
        # Build name->index lookup (raw name and Blender-prefix-stripped name both map)
        name_to_idx: dict = {}
        for i, anim in enumerate(gltf.animations):
            raw = anim.name or ''
            name_to_idx[raw] = i
            name_to_idx[strip_blender_prefix(raw)] = i

        for group_name, clips_csv in args.group:
            clips = [c.strip() for c in clips_csv.split(',')]
            indices = []
            for clip in clips:
                if clip in name_to_idx:
                    indices.append(name_to_idx[clip])
                else:
                    print(f"  Warning: clip '{clip}' not found in {src.name}", file=sys.stderr)
            if indices:
                groups[group_name] = indices
            else:
                print(f"  Warning: group '{group_name}' has no valid clips -- skipped",
                      file=sys.stderr)

    if not groups and not args.no_mesh:
        ap.error("No animation groups specified and --no-mesh not set.\n"
                 "Use --one-per-clip, --by-prefix, --group NAME CLIPS, or --list.")

    stem = src.stem

    # ── write mesh-only file ──────────────────────────────────────────────────
    if not args.no_mesh:
        out = out_dir / f"{stem}{args.mesh_suffix}.glb"
        print(f"Writing mesh-only ({len(gltf.meshes)} mesh(es)) -> {out.name}")
        make_mesh_only(gltf).save(str(out))

    # ── write animation group / per-clip files ────────────────────────────────
    total = len(groups)
    for idx, (group_name, anim_indices) in enumerate(groups.items(), 1):
        out = out_dir / f"{group_name}.glb"
        clip_names = [gltf.animations[i].name or f'anim{i}' for i in anim_indices]
        if len(anim_indices) == 1:
            print(f"[{idx:>3}/{total}] {out.name}  ({clip_names[0]})")
        else:
            print(f"[{idx:>3}/{total}] {out.name}  ({len(anim_indices)} clips)")
            for cn in clip_names:
                print(f"         {cn}")
        make_anim_group(gltf, anim_indices, group_name).save(str(out))

    print(f"\nDone. {total} animation file(s) + {'0' if args.no_mesh else '1'} mesh file written to {out_dir}")


if __name__ == '__main__':
    main()


if __name__ == '__main__':
    main()
