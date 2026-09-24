import os

root = r"C:\workspace\frank\projects\Ironhold\ironhold-lib-rules_to_state_machine_consolidation\assets\projects"

for dirpath, dirnames, filenames in os.walk(root):
    for f in filenames:
        if f == "state_machine.ron":
            path = os.path.join(dirpath, f)
            with open(path, 'rb') as fp:
                content = fp.read()
            # Fix the specific issues:
            # 1. "on: " inside FsmEventBinding -> "event: "
            # 2. "event: " inside FsmState -> "on: "
            content = content.replace(b'on: "', b'event: "')
            content = content.replace(b'event: []', b'on: []')
            content = content.replace(b'event: [', b'on: [')
            # Also fix any "event: " that appears inside a state definition (should be "on: ")
            # This is tricky - we need to be more careful. Let me just do a line-by-line approach
            # Actually the issue is:
            # - FsmEventBinding has "event:" field (not "on:")
            # - FsmState has "on:" field (not "event:")
            # The migration script incorrectly replaced both
            
            # Let me just do a more careful fix
            lines = content.decode('utf-8').split('\n')
            new_lines = []
            in_state = False
            state_brace_depth = 0
            
            for line in lines:
                stripped = line.strip()
                # Track if we're inside a state definition
                if stripped.startswith('name: ') and 'name:' in stripped and not in_state:
                    in_state = True
                    state_brace_depth = 0
                
                # Count braces
                for ch in line:
                    if ch == '(':
                        if in_state:
                            state_brace_depth += 1
                    elif ch == ')':
                        if in_state:
                            state_brace_depth -= 1
                            if state_brace_depth <= 0:
                                in_state = False
                
                # Fix the field names based on context
                if in_state:
                    # Inside a state: "event:" -> "on:"
                    if 'event: [' in line or 'event: []' in line:
                        line = line.replace('event: [', 'on: [').replace('event: []', 'on: []')
                else:
                    # Outside state (in global_on or transitions): "on:" -> "event:"
                    if 'on: "' in line:
                        line = line.replace('on: "', 'event: "')
                
                new_lines.append(line)
            
            content = '\n'.join(new_lines).encode('utf-8')
            
            with open(path, 'wb') as fp:
                fp.write(content)
            print(f"Fixed: {path}")

print("Done")