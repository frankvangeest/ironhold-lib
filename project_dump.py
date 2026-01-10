# Create a source code project dump

import os
from pathlib import Path

# Configuration
ROOT = Path(__file__).resolve().parent
OUT_FILE = ROOT / "project_dump.txt"
MAX_FILE_BYTES = 256 * 1024  # 256 KB

# Folders to ignore completely
IGNORE_DIRS = {
    ".git",
    "target",          # Rust build
    "node_modules",    # Node/NPM
    "dist", "build",   # Bundler outputs
    ".turbo", ".next", # Next.js/Turborepo
    ".idea", ".vscode",
    ".parcel-cache", ".cache",
    "coverage",
    "tmp", "temp",
    "pkg",
}

# File extensions to include (lowercase, with dot)
EXTENSIONS = {
    # Rust
    ".rs",
    # JS
    # ".js", ".jsx",
    # TS
    #  ".ts", ".tsx",
    # Web
    ".html", ".htm", ".css",
    # Docs
    ".md",
    # Assets
    ".ron",
}

# Specific filenames to always include even if extension is odd or empty
ALWAYS_INCLUDE_NAMES = {
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "tsconfig.json",
    "jsconfig.json",
}

def walk_filtered(root: Path):
    """Single-pass walker that prunes ignored directories."""
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in IGNORE_DIRS]
        yield Path(dirpath), dirnames, filenames

def print_tree(root: Path, out):
    """Non-recursive, non-duplicating tree printer."""
    out.write("==== DIRECTORY TREE ====\n")
    out.write(f"{root.name}\n")

    # Collect all entries in a flat list
    entries = []
    for dirpath, dirnames, filenames in walk_filtered(root):
        rel_dir = Path(dirpath).relative_to(root)
        # Sort to keep tree stable
        dirnames_sorted = sorted(dirnames)
        filenames_sorted = sorted(filenames)
        entries.append((rel_dir, dirnames_sorted, filenames_sorted))

    # Build a map from parent rel path -> list of children (dirs + files)
    tree_map = {}
    for rel_dir, dirnames, filenames in entries:
        children = [("dir", d) for d in dirnames] + [("file", f) for f in filenames]
        tree_map[rel_dir] = children

    def _print_dir(rel_dir: Path, prefix: str = ""):
        children = tree_map.get(rel_dir, [])
        for i, (kind, name) in enumerate(children):
            is_last = (i == len(children) - 1)
            connector = "└── " if is_last else "├── "
            if rel_dir == Path("."):
                display_path = name
                child_rel = Path(name)
            else:
                display_path = (rel_dir / name).as_posix()
                child_rel = rel_dir / name

            out.write(f"{prefix}{connector}{display_path}\n")

            if kind == "dir":
                new_prefix = prefix + ("    " if is_last else "│   ")
                _print_dir(child_rel, new_prefix)

    _print_dir(Path("."))
    out.write("\n")

def should_include_file(path: Path) -> bool:
    name = path.name
    if name in ALWAYS_INCLUDE_NAMES:
        return True
    ext = path.suffix.lower()
    return ext in EXTENSIONS

def dump_files(root: Path, out):
    out.write("==== FILE CONTENTS ====\n\n")
    count = 0
    total_bytes = 0
    for dirpath, dirnames, filenames in walk_filtered(root):
        for fname in sorted(filenames):
            fpath = Path(dirpath) / fname
            if not should_include_file(fpath):
                continue
            size = fpath.stat().st_size
            if size > MAX_FILE_BYTES:
                continue
            count += 1
            total_bytes += size
            rel = fpath.relative_to(root)
            out.write(f"----- {rel} ({size} bytes) -----\n")
            with fpath.open("r", encoding="utf-8", errors="replace") as f:
                for line in f:
                    out.write(line)
            out.write("\n")
    out.write(f"\n==== SUMMARY: {count} files, {total_bytes} bytes raw ====\n")


def main():
    with OUT_FILE.open("w", encoding="utf-8") as out:
        print_tree(ROOT, out)
        dump_files(ROOT, out)

if __name__ == "__main__":
    main()
