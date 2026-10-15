---
name: inputmap-key-fallback-asymmetry
description: InputMap key fields are Option<String> (input_strafe_bindings); unknown name = unbound for every field EXCEPT target_next (falls back to Tab); validate's InputMap checks skip players with no inputs: block
metadata:
  type: project
---

As of feature/input_strafe_bindings (reviewed 2026-10-16): every InputMap keyboard field is `Option<String>`, `None` = unbound, strafe_left/right default None and are additive into Move.x in `input_translator_system`.

- Unrecognised key name: `InputMap::key()` -> None (unbound) for forward..interact, but `tab_targeting_system` maps an authored-but-unknown `target_next` (incl. `""`) to `KeyCode::Tab`. validate's `invalid_key` / `empty_key_binding` messages say "silently ignored" for all fields, which is false for target_next.
- validate's `duplicate_input_key` / `movement_key_slot_collision` do `let Some(inputs) = ...inputs.as_ref() else { continue }`, so a player prefab with NO `inputs:` block (runtime = full WASD/Space/ShiftLeft/F/Tab defaults) is never compared — false negative. Contrast line ~3349's gamepad check which uses `.clone().unwrap_or_default()`.
- duplicate_input_key is per-InputMap only; cross-player collisions (local_coop players all default `interact: KeyF`) are not checked.

**Why:** these asymmetries are where "unbound vs default vs fallback" bugs hide.
**How to apply:** when touching InputMap or its validate checks, decide per field what unknown/empty/omitted-block resolves to and check the validator models the same thing. See [[project_schema_validate_errors_are_nonfatal]].
