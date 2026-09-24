import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\crates\ironhold_cli\tests\fixtures"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'r') as fp:
                content = fp.read()
            if 'schema_versievent' in content:
                content = content.replace('schema_versievent', 'schema_version')
                with open(path, 'w') as fp:
                    fp.write(content)
                print(f"Fixed: {path}")

print("Done")