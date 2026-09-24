import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\assets\projects"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'rb') as fp:
                content = fp.read()
            
            # FsmTransition uses "on:" for the event name (NOT "event:")
            # Only fix the transitions section
            lines = content.decode('utf-8').split('\n')
            new_lines = []
            in_transitions = False
            for line in lines:
                stripped = line.strip()
                if stripped == 'transitions: [':
                    in_transitions = True
                elif in_transitions and stripped.startswith(']'):
                    in_transitions = False
                
                if in_transitions:
                    # In transitions, "event:" should be "on:"
                    if 'event: "' in line:
                        line = line.replace('event: "', 'on: "')
                
                new_lines.append(line)
            
            content = '\n'.join(new_lines).encode('utf-8')
            
            with open(path, 'wb') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

print("Done")