#!/usr/bin/env python3
"""
One-off migration script: convert rules.ron → state_machine.ron

This script is committed in the migration commit and deleted in the removal commit.
It stays in history for review but never ships.

Run from repo root.
"""

import os
import re
import sys
from pathlib import Path

# Fixtures that need hand work - the script prints and skips these
HAND_HANDLED = {
    "valid_ui_trigger",
    "unset_rules_path_with_convention_file",
    "state_machine_only_ignores_dead_rules_ron",
    "rules_path_case_mismatch",
    "rules_path_custom_filename_is_discovered",
    "bad_rules_parse_no_cascade",
    "inline_rules_are_discovered",
    "blank_project",
}

# Also hand-handled: any fixture where the target state_machine.ron already exists
# (checked per-directory in the main loop)

def find_rules_files(root: Path):
    """Find all logic/*rules*.ron files in assets/projects/ and crates/ironhold_cli/tests/fixtures/"""
    rules_files = []
    for base in [root / "assets/projects", root / "crates/ironhold_cli/tests/fixtures"]:
        if not base.exists():
            continue
        for project_dir in base.iterdir():
            if not project_dir.is_dir():
                continue
            logic_dir = project_dir / "logic"
            if not logic_dir.exists():
                continue
            for f in logic_dir.glob("*rules*.ron"):
                rules_files.append(f)
    return rules_files

def read_file(path: Path) -> str:
    with open(path, 'r', encoding='utf-8', newline='') as f:
        return f.read()

def write_file(path: Path, content: str):
    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(content)

def detect_line_ending(content: str) -> str:
    if '\r\n' in content:
        return '\r\n'
    return '\n'

def replace_depth_aware(text: str, target_key: str, replacement: str) -> str:
    """
    Replace `target_key:` with `replacement:` only at depth 1 (top-level tuple fields),
    skipping string literals. This handles the `on:` -> `event:` rename correctly.
    """
    result = []
    i = 0
    depth = 0  # paren/bracket depth
    in_string = False
    string_char = None
    escape_next = False
    
    while i < len(text):
        ch = text[i]
        
        if escape_next:
            result.append(ch)
            escape_next = False
            i += 1
            continue
            
        if in_string:
            result.append(ch)
            if ch == '\\':
                escape_next = True
            elif ch == string_char:
                in_string = False
                string_char = None
            i += 1
            continue
            
        if ch in ('"', "'"):
            in_string = True
            string_char = ch
            result.append(ch)
            i += 1
            continue
            
        if ch in ('(', '['):
            depth += 1
            result.append(ch)
            i += 1
            continue
            
        if ch in (')', ']'):
            depth -= 1
            result.append(ch)
            i += 1
            continue
            
        # Check for target_key: at depth 1 (inside a rule tuple)
        if depth == 1 and text[i:].startswith(target_key + ':'):
            result.append(replacement + ':')
            i += len(target_key) + 1
            continue
            
        result.append(ch)
        i += 1
        
    return ''.join(result)

def migrate_rules_to_fsm(rules_content: str) -> str:
    """Transform rules.ron content to state_machine.ron format"""
    content = rules_content
    
    # 1. Replace schema_version: 2 (or 1) with schema_version: 1
    content = re.sub(r'schema_version:\s*[12]', 'schema_version: 1', content)
    
    # 2. Replace top-level `rules:` with `global_on:`
    content = replace_depth_aware(content, 'rules', 'global_on')
    
    # 3. Replace each rule's `on:` with `event:` (only at rule-tuple depth)
    content = replace_depth_aware(content, 'on', 'event')
    
    return content

def update_project_ron(project_path: Path):
    """Flip rules_path: \"logic/rules.ron\" -> state_machine_path: \"logic/state_machine.ron\""""
    content = read_file(project_path)
    original = content
    
    # Match exactly: rules_path: "logic/rules.ron" or rules_path: Some("logic/rules.ron")
    # Also handle the case where it's not present
    patterns = [
        (r'rules_path:\s*"logic/rules\.ron"', 'state_machine_path: "logic/state_machine.ron"'),
        (r'rules_path:\s*Some\("logic/rules\.ron"\)', 'state_machine_path: Some("logic/state_machine.ron")'),
    ]
    
    for pattern, replacement in patterns:
        content = re.sub(pattern, replacement, content)
    
    if content != original:
        write_file(project_path, content)
        return True
    return False

def main():
    repo_root = Path(__file__).parent  # repo root (script is at repo root)
    print(f"Repo root: {repo_root}")
    
    rules_files = find_rules_files(repo_root)
    print(f"Found {len(rules_files)} rules files to process")
    
    processed = 0
    skipped = 0
    errors = 0
    
    for rules_file in rules_files:
        project_dir = rules_file.parent.parent
        project_name = project_dir.name
        
        print(f"\nProcessing: {project_name} ({rules_file})")
        
        # Check if hand-handled
        if project_name in HAND_HANDLED:
            print(f"  SKIP (hand-handled): {project_name}")
            skipped += 1
            continue
            
        # Check if target already exists
        target_file = project_dir / "logic" / "state_machine.ron"
        if target_file.exists():
            print(f"  SKIP (target exists): {target_file}")
            skipped += 1
            continue
            
        # Check project.ron for rules_path we can cleanly match
        project_ron = project_dir / f"{project_name}.project.ron"
        has_project_ron = project_ron.exists()
        
        if has_project_ron:
            project_content = read_file(project_ron)
            if 'rules_path: "logic/rules.ron"' not in project_content and \
               'rules_path: Some("logic/rules.ron")' not in project_content:
                print(f"  ERROR: Cannot cleanly match rules_path in {project_ron}")
                print(f"  Content: {project_content[:200]}...")
                errors += 1
                continue
        else:
            # No .project.ron - this is a CLI fixture using convention fallback
            # We'll create state_machine.ron and the CLI will use convention path
            print(f"  No .project.ron - using convention fallback")
            
        # Read and transform
        rules_content = read_file(rules_file)
        
        # Check for when: or EnterState - abort if found
        if 'when:' in rules_content:
            print(f"  ERROR: Found 'when:' in {rules_file} - manual review needed")
            errors += 1
            continue
            
        # Check for EnterState in do_actions (would be EnterState("..."))
        if 'EnterState(' in rules_content:
            print(f"  ERROR: Found 'EnterState(' in {rules_file} - manual review needed")
            errors += 1
            continue
        
        # Transform
        fsm_content = migrate_rules_to_fsm(rules_content)
        
        # Write state_machine.ron (preserve line endings)
        line_ending = detect_line_ending(rules_content)
        fsm_content = fsm_content.replace('\n', line_ending)
        write_file(target_file, fsm_content)
        
        # Update project.ron if it exists
        if has_project_ron:
            update_project_ron(project_ron)
        
        # Leave rules.ron on disk for proof test
        print(f"  OK: Created {target_file}")
        processed += 1
        
    print(f"\n=== Summary ===")
    print(f"Processed: {processed}")
    print(f"Skipped: {skipped}")
    print(f"Errors: {errors}")
    
    if errors > 0:
        print("ERRORS FOUND - migration aborted")
        sys.exit(1)
        
    print("\nNext steps:")
    print("1. Hand-merge valid_ui_trigger (rules first in global_on)")
    print("2. Hand-edit blank_project into minimal form + explanatory comment")
    print("3. Fix any comment text that says 'rules.ron' (grep afterward)")
    print("4. Run the equivalence proof test")

if __name__ == "__main__":
    main()