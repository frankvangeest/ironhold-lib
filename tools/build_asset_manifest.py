import os
import json
import re
from pathlib import Path

# Configuration
ROOT = Path(__file__).resolve().parent.parent
ASSETS_DIR = ROOT / "assets"
OUT_FILE = ROOT / "assets_manifest.json"

# Regex for matching previews like -preview-01
PREVIEW_PATTERN = re.compile(r"(.*?)(?:-preview(?:-(\d+))?)?\.(png|avif|jpg|jpeg|webp)$", re.IGNORECASE)

def get_asset_id(path: Path):
    return str(path.relative_to(ASSETS_DIR).as_posix())

def parse_texture_descriptions(md_path: Path):
    """Simple parser for texture-descriptions.md"""
    descriptions = {}
    if not md_path.exists():
        return descriptions
    
    current_title = None
    current_text = []
    
    with open(md_path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if line.startswith("## "):
                if current_title:
                    descriptions[current_title.lower()] = " ".join(current_text).strip()
                current_title = line[3:].strip()
                current_text = []
            elif current_title and line:
                if not line.startswith(">") and not line.startswith("---"):
                    current_text.append(line)
        
        if current_title:
            descriptions[current_title.lower()] = " ".join(current_text).strip()
            
    return descriptions

def build_manifest():
    manifest = {
        "files": [],
        "folders": {}
    }
    
    # Load global texture descriptions
    global_descriptions = parse_texture_descriptions(ASSETS_DIR / "shared" / "textures" / "texture-descriptions.md")

    for dirpath, dirnames, filenames in os.walk(ASSETS_DIR):
        rel_dir = Path(dirpath).relative_to(ASSETS_DIR)
        
        # Skip .ron files (they are ignored per requirement)
        # Note: We still want to process the directory even if it has .ron files
        
        current_assets = {} # name -> asset_data

        for fname in filenames:
            if fname.endswith(".ron"):
                continue
            
            fpath = Path(dirpath) / fname
            stem = fpath.stem
            ext = fpath.suffix.lower()
            
            # Check if it's a preview image
            match = PREVIEW_PATTERN.match(fname)
            is_preview = False
            base_name = stem
            
            if ext in ['.png', '.avif', '.jpg', '.jpeg', '.webp']:
                # Logic: if name ends with -preview or matches a sibling model/asset
                if "-preview" in stem:
                    is_preview = True
                    base_name = stem.split("-preview")[0]
                else:
                    # Check if a model with the same stem exists in this folder
                    # This is a bit tricky during a single pass, so we'll do a second refinement pass or store candidates
                    pass

            if ext == ".md":
                # Description candidate
                pass
            
            # For now, let's just collect all files and then group them
        
    # Re-writing with a more robust grouping logic
    all_files = []
    for dirpath, dirnames, filenames in os.walk(ASSETS_DIR):
        for fname in filenames:
            if not fname.endswith(".ron"):
                all_files.append(Path(dirpath) / fname)

    asset_groups = {} # (dir, base_name) -> {files: [], previews: [], description: ""}

    # 1. Identify primary assets (.glb, .wav, .mp3, etc.)
    PRIMARY_EXTS = ['.glb', '.wav', '.mp3', '.wgsl']
    
    for fpath in all_files:
        ext = fpath.suffix.lower()
        rel_dir = fpath.parent.relative_to(ASSETS_DIR)
        
        if ext in PRIMARY_EXTS:
            key = (rel_dir, fpath.stem)
            if key not in asset_groups:
                asset_groups[key] = {"primary": fpath, "previews": [], "description_file": None, "type": ext[1:]}

    # 2. Identify Texture Folders (per requirement: Stylized_Bark_003_SD is one asset)
    # If a folder name ends with _SD or seems like a texture set, we might group it.
    # Actually, the user said "Stylized_Bark_003_SD is one texture asset".
    for dirpath, dirnames, filenames in os.walk(ASSETS_DIR / "shared" / "textures"):
        p = Path(dirpath)
        if p.name.endswith("_SD") or p.name == "backgrounds" or p.name == "noise":
            # Treat this folder as an asset if it's not the root textures folder
            if p != ASSETS_DIR / "shared" / "textures":
                rel_dir = p.parent.relative_to(ASSETS_DIR)
                key = (rel_dir, p.name)
                if key not in asset_groups:
                    asset_groups[key] = {"primary_dir": p, "previews": [], "description_file": None, "type": "texture_set"}

    # 3. Assign previews and descriptions
    for fpath in all_files:
        ext = fpath.suffix.lower()
        rel_dir = fpath.parent.relative_to(ASSETS_DIR)
        stem = fpath.stem
        
        # Check if it's a preview or description for an existing group
        if ext in ['.png', '.avif', '.jpg', '.jpeg', '.webp']:
            is_preview = False
            target_name = stem
            if "-preview" in stem:
                target_name = stem.split("-preview")[0]
                is_preview = True
            
            key = (rel_dir, target_name)
            if key in asset_groups:
                asset_groups[key]["previews"].append(str(fpath.relative_to(ROOT).as_posix()))
            elif not is_preview:
                # If it's just an image not explicitly a preview, maybe it's a standalone texture or a candidate
                pass

        if ext == ".md":
            key = (rel_dir, stem)
            if key in asset_groups:
                asset_groups[key]["description_file"] = fpath

    # 4. Finalize manifest
    final_assets = []
    for (rel_dir, name), data in asset_groups.items():
        asset_rel_path = ""
        if "primary" in data:
            asset_rel_path = str(data["primary"].relative_to(ASSETS_DIR).as_posix())
        elif "primary_dir" in data:
            asset_rel_path = str(data["primary_dir"].relative_to(ASSETS_DIR).as_posix())

        # Description logic
        description = ""
        if data["description_file"]:
            with open(data["description_file"], 'r', encoding='utf-8') as f:
                description = f.read()
        else:
            # Fallback to global
            clean_name = name
            if name.endswith("_SD"):
                clean_name = name[:-3]
            
            fallback_key = clean_name.replace("_", " ").lower()
            description = global_descriptions.get(fallback_key, "")

        # If no explicit previews but it's a texture set, look for a basecolor/albedo
        if not data["previews"] and data["type"] == "texture_set" and "primary_dir" in data:
            for tfname in os.listdir(data["primary_dir"]):
                tlow = tfname.lower()
                if "basecolor" in tlow or "albedo" in tlow or "diffuse" in tlow:
                    data["previews"].append(str((data["primary_dir"] / tfname).relative_to(ROOT).as_posix()))
                    break

        final_assets.append({
            "name": name,
            "path": asset_rel_path,
            "dir": str(rel_dir.as_posix()),
            "type": data["type"],
            "previews": sorted(data["previews"]),
            "description": description
        })

    with open(OUT_FILE, 'w', encoding='utf-8') as f:
        json.dump(final_assets, f, indent=2)

    print(f"Manifest written to {OUT_FILE}")

if __name__ == "__main__":
    build_manifest()
