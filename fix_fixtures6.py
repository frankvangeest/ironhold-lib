import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\assets\projects"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'rb') as fp:
                content = fp.read()
            
            # FsmEventBinding ALWAYS uses "event:" field (not "on:")
            # This applies in BOTH global_on AND state's on: list
            content = content.replace(b'on: "ui.', b'event: "ui.')
            content = content.replace(b'on: "scene.', b'event: "scene.')
            content = content.replace(b'on: "entity.', b'event: "entity.')
            content = content.replace(b'on: "monster.', b'event: "monster.')
            content = content.replace(b'on: "quest.', b'event: "quest.')
            
            # Also fix FsmState: "event: [" -> "on: [" and "event: []" -> "on: []"
            content = content.replace(b'event: [', b'on: [')
            content = content.replace(b'event: []', b'on: []')
            
            with open(path, 'wb') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

print("Done")