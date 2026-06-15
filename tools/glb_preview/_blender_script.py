"""
Blender-internal render script. Do not run directly.
Invoked by preview.py via: blender --background --python _blender_script.py -- <args>
"""

import sys
import math
import bpy
import mathutils


# ---------------------------------------------------------------------------
# Arg parsing (everything after the -- separator)
# ---------------------------------------------------------------------------

def _parse():
    try:
        argv = sys.argv[sys.argv.index("--") + 1:]
    except ValueError:
        argv = []
    args = {}
    i = 0
    while i < len(argv):
        if argv[i].startswith("--") and i + 1 < len(argv):
            args[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            i += 1
    return args


# ---------------------------------------------------------------------------
# Scene setup
# ---------------------------------------------------------------------------

def _clear_scene():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()
    for col in [bpy.data.meshes, bpy.data.lights, bpy.data.cameras,
                bpy.data.materials, bpy.data.images]:
        for block in list(col):
            col.remove(block)


def _import_glb(path: str) -> list:
    before = set(bpy.context.scene.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.context.scene.objects if o not in before]


def _assign_fallback_materials(objects):
    """Assign a default diffuse material to any mesh with no materials."""
    fallback = None
    for obj in objects:
        if obj.type != "MESH":
            continue
        if obj.data.materials and any(m is not None for m in obj.data.materials):
            continue
        if fallback is None:
            fallback = bpy.data.materials.new("_Fallback")
            fallback.use_nodes = True
            bsdf = fallback.node_tree.nodes.get("Principled BSDF")
            if bsdf:
                bsdf.inputs["Base Color"].default_value = (0.75, 0.75, 0.75, 1.0)
                bsdf.inputs["Roughness"].default_value = 0.6
        if not obj.data.materials:
            obj.data.materials.append(fallback)
        else:
            for i, m in enumerate(obj.data.materials):
                if m is None:
                    obj.data.materials[i] = fallback


def _scene_bounds(objects):
    """Return (center Vector, radius float) of all imported objects."""
    inf = float("inf")
    lo = mathutils.Vector(( inf,  inf,  inf))
    hi = mathutils.Vector((-inf, -inf, -inf))
    # Armatures and empties have zero or misleading extents — use meshes only.
    mesh_objects = [o for o in objects if o.type == "MESH"] or objects
    for obj in mesh_objects:
        for corner in obj.bound_box:
            w = obj.matrix_world @ mathutils.Vector(corner)
            lo.x, lo.y, lo.z = min(lo.x, w.x), min(lo.y, w.y), min(lo.z, w.z)
            hi.x, hi.y, hi.z = max(hi.x, w.x), max(hi.y, w.y), max(hi.z, w.z)
    center = (lo + hi) / 2
    radius = (hi - lo).length / 2
    return center, max(radius, 0.01)


def _add_camera(center: mathutils.Vector, radius: float,
                az_deg: float = 45.0, el_deg: float = 30.0):
    """Orbit camera: az_deg = horizontal rotation, el_deg = elevation above horizon."""
    dist = radius * 3.0
    az = math.radians(az_deg)
    el = math.radians(el_deg)

    loc = mathutils.Vector((
        center.x + dist * math.cos(el) * math.sin(az),
        center.y - dist * math.cos(el) * math.cos(az),
        center.z + dist * math.sin(el),
    ))

    cam_data = bpy.data.cameras.new("PreviewCam")
    cam_data.lens = 50
    # Scale clip planes to scene — prevents tiny models being clipped by Blender's 0.1m default.
    cam_data.clip_start = max(dist * 0.001, 1e-6)
    cam_data.clip_end   = dist * 10.0
    cam_obj = bpy.data.objects.new("PreviewCam", cam_data)
    bpy.context.scene.collection.objects.link(cam_obj)
    cam_obj.location = loc

    direction = center - loc
    cam_obj.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = cam_obj


def _add_lighting(center: mathutils.Vector, radius: float, strength: float = 1.0):
    """Three-point lighting: warm key, cool fill, white rim."""
    lights = [
        # (name,  type,    color,               energy_factor, offset)
        ("Key",  "AREA", (1.00, 0.85, 0.65),  4.0, (-1.2,  -0.8,  1.5)),
        ("Fill", "AREA", (0.80, 0.85, 0.95),  1.5, ( 1.5,  -0.5,  0.8)),
        ("Rim",  "AREA", (1.00, 0.95, 0.88),  2.0, ( 0.0,   1.2,  1.2)),
    ]
    for name, kind, color, factor, offset in lights:
        ld = bpy.data.lights.new(name, kind)
        ld.color = color
        ld.energy = radius * radius * 200 * factor * strength
        ld.size = radius * 2
        lo = bpy.data.objects.new(name, ld)
        bpy.context.scene.collection.objects.link(lo)
        lo.location = center + mathutils.Vector(offset) * radius * 2


# ---------------------------------------------------------------------------
# Render
# ---------------------------------------------------------------------------

def _render(output: str, size: int):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = size
    scene.render.resolution_y = size
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.compression = 15
    scene.render.filepath = output

    # EEVEE quality settings (attribute names vary by version — skip unknown ones)
    for attr, val in [("taa_render_samples", 64), ("taa_samples", 64)]:
        try:
            setattr(scene.eevee, attr, val)
            break
        except AttributeError:
            pass

    # Neutral world background (visible through transparent gaps, ignored with RGBA)
    world = bpy.data.worlds.new("PreviewWorld")
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.05, 0.05, 0.05, 1.0)
    bg.inputs["Strength"].default_value = 0.0
    scene.world = world

    bpy.ops.render.render(write_still=True)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    args = _parse()
    glb_path = args.get("input", "")
    out_path = args.get("output", "")
    size = int(args.get("size", "512"))

    if not glb_path or not out_path:
        print("ERROR: --input and --output are required", file=sys.stderr)
        sys.exit(1)

    _clear_scene()
    objects = _import_glb(glb_path)

    if not objects:
        print(f"ERROR: no objects imported from {glb_path}", file=sys.stderr)
        sys.exit(1)

    _assign_fallback_materials(objects)
    strength = float(args.get("light-strength", "1.0"))
    az_deg   = float(args.get("camera-az", "45.0"))
    el_deg   = float(args.get("camera-el", "30.0"))
    center, radius = _scene_bounds(objects)
    _add_camera(center, radius, az_deg, el_deg)
    _add_lighting(center, radius, strength)
    _render(out_path, size)
    print(f"Rendered: {out_path}")


main()
