"""
migrate_implicit_some.py — one-shot migration to implicit_some RON extension.

The runtime loader (schema/ron_loader.rs) already enables implicit_some globally,
so no per-file directive is needed. This script strips all Some(...) wrappers from
.ron files under assets/projects/ so designers can use bare values everywhere.

Handles:
  - Balanced parentheses inside Some() (e.g. Some(( field: (...) )))
  - Strings (skipped so "Some()" in string literals is not touched)
  - // line comments (skipped)

Run from repo root:
  python tools/migrate_implicit_some.py
  python tools/migrate_implicit_some.py --dry-run
"""

import sys
from pathlib import Path

def find_matching_close(text: str, start: int) -> int:
    """
    Given text[start] is the character immediately after the opening '(' of Some(,
    return the index of the matching ')'.  Returns len(text) if not found.
    """
    depth = 1
    i = start
    n = len(text)
    while i < n and depth > 0:
        c = text[i]
        if c == "(":
            depth += 1
            i += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
            i += 1
        elif c == '"':
            # skip string literal
            i += 1
            while i < n:
                if text[i] == "\\" and i + 1 < n:
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
        else:
            i += 1
    return i  # not found (malformed) — return end


def strip_some(text: str) -> str:
    """Strip all Some(...) wrappers from RON text, leaving the inner content."""
    out = []
    i = 0
    n = len(text)

    while i < n:
        c = text[i]

        # Skip line comments
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            end = text.find("\n", i)
            if end == -1:
                out.append(text[i:])
                break
            out.append(text[i : end + 1])
            i = end + 1
            continue

        # Skip string literals
        if c == '"':
            out.append(c)
            i += 1
            while i < n:
                if text[i] == "\\" and i + 1 < n:
                    out.append(text[i : i + 2])
                    i += 2
                elif text[i] == '"':
                    out.append(text[i])
                    i += 1
                    break
                else:
                    out.append(text[i])
                    i += 1
            continue

        # Detect Some(
        if text[i : i + 5] == "Some(":
            inner_start = i + 5
            close = find_matching_close(text, inner_start)
            inner = text[inner_start:close]
            out.append(inner)
            i = close + 1  # skip past ')'
            continue

        out.append(c)
        i += 1

    return "".join(out)


def process_file(path: Path, dry_run: bool) -> bool:
    original = path.read_text(encoding="utf-8")
    updated = strip_some(original)
    if updated == original:
        return False
    if not dry_run:
        path.write_text(updated, encoding="utf-8")
    return True


def main():
    dry_run = "--dry-run" in sys.argv
    root = Path("assets/projects")
    if not root.exists():
        print("ERROR: Run from repo root (assets/projects/ not found).")
        sys.exit(1)

    changed, skipped = [], []
    for f in sorted(root.rglob("*.ron")):
        if process_file(f, dry_run):
            changed.append(f)
            print(f"{'[dry] ' if dry_run else ''}Updated: {f}")
        else:
            skipped.append(f)

    print(f"\n{'[dry-run] ' if dry_run else ''}Done: {len(changed)} files updated, {len(skipped)} unchanged.")


if __name__ == "__main__":
    main()
