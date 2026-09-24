import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\crates\ironhold_cli\tests\fixtures"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'r') as fp:
                content = fp.read()
            changed = False
            if 'global_event' in content:
                content = content.replace('global_event', 'global_on')
                changed = True
            if 'schema_versievent' in content:
                content = content.replace('schema_versievent', 'schema_version')
                changed = True
            if changed:
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
            changed = False
            if 'global_event' in content:
                content = content.replace('global_event', 'global_on')
                changed = True
            if 'schema_versievent' in content:
                content = content.replace('schema_versievent', 'schema_version')
                changed = True
            if changed:
                with open(path, 'w') as fp:
                    fp.write(content)
                print(f"Fixed: {path}")

print("Done")