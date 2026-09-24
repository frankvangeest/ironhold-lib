import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\crates\ironhold_cli\tests\fixtures"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'r') as fp:
                content = fp.read()
            # Fix the specific issue: "on" inside FsmEventBinding should be "event"
            # This happens at lines that look like: "on: \"some_event\","
            # which should become: "event: \"some_event\","
            # But NOT at the top-level "global_on:" field
            lines = content.split('\n')
            for i, line in enumerate(lines):
                stripped = line.strip()
                # Look for "on: " at the start of a line (after indentation) inside global_on list
                if stripped.startswith('on: ') and not stripped.startswith('global_on:'):
                    lines[i] = line.replace('on: ', 'event: ', 1)
            content = '\n'.join(lines)
            
            with open(path, 'w') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

# Also fix assets/projects
root2 = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\assets\projects"
for dirpath, dirnames, filenames in os.walk(root2):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'r') as fp:
                content = fp.read()
            lines = content.split('\n')
            for i, line in enumerate(lines):
                stripped = line.strip()
                if stripped.startswith('on: ') and not stripped.startswith('global_on:'):
                    lines[i] = line.replace('on: ', 'event: ', 1)
            content = '\n'.join(lines)
            
            with open(path, 'w') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

print("Done")