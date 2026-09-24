import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\assets\projects"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'rb') as fp:
                content = fp.read()
            
            # Fix: In FsmTransition, "on:" is correct (the event name), not "event:"
            # My previous script incorrectly changed "on:" to "event:" in transitions
            content = content.replace(b'event: "ui.', b'on: "ui.')
            content = content.replace(b'event: "scene.', b'on: "scene.')
            content = content.replace(b'event: "entity.', b'on: "entity.')
            content = content.replace(b'event: "monster.', b'on: "monster.')
            content = content.replace(b'event: "quest.', b'on: "quest.')
            
            with open(path, 'wb') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

print("Done")