#!/usr/bin/env python3
"""
GLB/GLTF inspector — prints node names, materials, animations, and textures.

Usage:
    python inspect_glb.py <file.glb>
    python inspect_glb.py <file.glb> --json

Useful for authoring RON scene files: reveals the exact node names, material
names, and animation clip names that are embedded in a GLB asset.
"""

import sys
import json
import struct
import argparse
from pathlib import Path

try:
    import pygltflib
except ImportError:
    print("pygltflib not found. Install with:  pip install pygltflib", file=sys.stderr)
    sys.exit(1)

# PIL is optional — only needed for embedded image dimensions
try:
    from PIL import Image
    import io as _io
    _PIL = True
except ImportError:
    _PIL = False

_COMPONENT_SIZE = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
_TYPE_COUNT = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}


def _read_float_scalar(gltf, accessor_index):
    """Return all float values from a SCALAR FLOAT accessor."""
    if accessor_index is None or accessor_index >= len(gltf.accessors):
        return []
    acc = gltf.accessors[accessor_index]
    if acc.componentType != 5126 or acc.type != "SCALAR" or acc.bufferView is None:
        return []
    blob = gltf.binary_blob()
    if not blob:
        return []
    bv = gltf.bufferViews[acc.bufferView]
    base = (bv.byteOffset or 0) + (acc.byteOffset or 0)
    stride = bv.byteStride or 4
    return [struct.unpack_from("<f", blob, base + i * stride)[0] for i in range(acc.count)]


def _anim_duration(gltf, anim):
    """Return the duration of an animation clip in seconds."""
    max_t = 0.0
    seen = set()
    for ch in anim.channels:
        inp = anim.samplers[ch.sampler].input
        if inp in seen:
            continue
        seen.add(inp)
        times = _read_float_scalar(gltf, inp)
        if times:
            max_t = max(max_t, max(times))
    return max_t


def _image_dims(gltf, image):
    """Return (width, height) for an embedded image, or None."""
    if not _PIL or image.bufferView is None:
        return None
    blob = gltf.binary_blob()
    if not blob:
        return None
    bv = gltf.bufferViews[image.bufferView]
    data = blob[(bv.byteOffset or 0):(bv.byteOffset or 0) + bv.byteLength]
    try:
        return Image.open(_io.BytesIO(data)).size
    except Exception:
        return None


def inspect(path: Path) -> dict:
    gltf = pygltflib.GLTF2().load(str(path))

    # Collect joint indices so we can tag bones correctly
    joint_set = set()
    for skin in (gltf.skins or []):
        joint_set.update(skin.joints or [])

    # Nodes
    nodes = []
    for i, node in enumerate(gltf.nodes or []):
        if i in joint_set:
            kind = "bone"
        elif node.mesh is not None:
            kind = "mesh"
        else:
            kind = "node"
        nodes.append({"name": node.name or f"node_{i}", "kind": kind, "mesh_index": node.mesh})

    # Meshes
    meshes = []
    for i, mesh in enumerate(gltf.meshes or []):
        mat_indices = list({p.material for p in (mesh.primitives or []) if p.material is not None})
        meshes.append({
            "name": mesh.name or f"mesh_{i}",
            "primitive_count": len(mesh.primitives or []),
            "material_indices": sorted(mat_indices),
        })

    # Materials
    materials = []
    for i, mat in enumerate(gltf.materials or []):
        entry = {"name": mat.name or f"material_{i}"}
        if mat.pbrMetallicRoughness:
            pbr = mat.pbrMetallicRoughness
            entry["metallic_factor"] = pbr.metallicFactor
            entry["roughness_factor"] = pbr.roughnessFactor
            if pbr.baseColorTexture:
                entry["base_color_texture"] = pbr.baseColorTexture.index
            if pbr.metallicRoughnessTexture:
                entry["metallic_roughness_texture"] = pbr.metallicRoughnessTexture.index
        if mat.normalTexture:
            entry["normal_texture"] = mat.normalTexture.index
        if mat.occlusionTexture:
            entry["occlusion_texture"] = mat.occlusionTexture.index
        if mat.emissiveFactor and any(f != 0.0 for f in mat.emissiveFactor):
            entry["emissive_factor"] = list(mat.emissiveFactor)
        if mat.doubleSided:
            entry["double_sided"] = True
        if mat.alphaMode and mat.alphaMode != "OPAQUE":
            entry["alpha_mode"] = mat.alphaMode
        materials.append(entry)

    # Animations
    animations = []
    for anim in (gltf.animations or []):
        animations.append({
            "name": anim.name or "unnamed",
            "duration_s": round(_anim_duration(gltf, anim), 4),
            "channel_count": len(anim.channels or []),
        })

    # Images
    images = []
    for i, img in enumerate(gltf.images or []):
        entry = {
            "index": i,
            "name": img.name or img.uri or f"image_{i}",
            "mime_type": img.mimeType or "unknown",
            "source": "embedded" if img.bufferView is not None else f"uri:{img.uri or '?'}",
        }
        dims = _image_dims(gltf, img)
        if dims:
            entry["width"], entry["height"] = dims
        images.append(entry)

    return {
        "file": str(path),
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "animations": animations,
        "images": images,
    }


def _print_report(data: dict):
    print(f"\n=== GLB Inspector: {data['file']} ===\n")

    nodes = data["nodes"]
    print(f"Nodes ({len(nodes)}):")
    for n in nodes:
        print(f"  [{n['kind']:<5}]  {n['name']}")

    meshes = data["meshes"]
    if meshes:
        print(f"\nMeshes ({len(meshes)}):")
        for m in meshes:
            mats = ", ".join(f"mat[{i}]" for i in m["material_indices"]) or "none"
            print(f"  {m['name']:<42}  {m['primitive_count']} prim(s)  materials: {mats}")

    mats = data["materials"]
    if mats:
        print(f"\nMaterials ({len(mats)}):")
        for m in mats:
            tags = []
            for key, label in [
                ("base_color_texture", "baseColor"),
                ("normal_texture", "normal"),
                ("metallic_roughness_texture", "metalRough"),
                ("occlusion_texture", "occlusion"),
            ]:
                if key in m:
                    tags.append(f"{label}=tex[{m[key]}]")
            if m.get("double_sided"):
                tags.append("double_sided")
            if "alpha_mode" in m:
                tags.append(m["alpha_mode"])
            print(f"  {m['name']:<42}  {'  '.join(tags)}")

    anims = data["animations"]
    if anims:
        print(f"\nAnimations ({len(anims)}):")
        for a in anims:
            print(f"  {a['name']:<42}  {a['duration_s']:.3f}s  ({a['channel_count']} channels)")
    else:
        print("\nAnimations: none")

    imgs = data["images"]
    if imgs:
        print(f"\nImages ({len(imgs)}):")
        for img in imgs:
            dims = f"  {img['width']}x{img['height']}" if "width" in img else ""
            print(f"  [{img['index']}] {img['name']:<40}  {img['mime_type']}{dims}  ({img['source']})")

    print()


def main():
    parser = argparse.ArgumentParser(description="Inspect a GLB or GLTF file")
    parser.add_argument("file", help="Path to .glb or .gltf file")
    parser.add_argument("--json", action="store_true", help="Emit JSON instead of human-readable output")
    args = parser.parse_args()

    path = Path(args.file)
    if not path.exists():
        print(f"File not found: {path}", file=sys.stderr)
        sys.exit(1)

    data = inspect(path)

    if args.json:
        print(json.dumps(data, indent=2))
    else:
        _print_report(data)


if __name__ == "__main__":
    main()
