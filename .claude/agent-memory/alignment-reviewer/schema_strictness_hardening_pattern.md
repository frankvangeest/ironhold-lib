---
name: schema-strictness-hardening-pattern
description: Reviewing deny_unknown_fields / stricter-parse changes — the five Action-bearing RON surfaces and how each one's parse failure actually surfaces to a designer
metadata:
  type: project
---

Whenever a change makes a schema type parse *more strictly* (`deny_unknown_fields`, a newly
required field, a removed field), the alignment question is not "is it reachable from RON" — it
trivially is. It is: **when the designer's RON now fails, do they find out?** Established during
the `feature/action-deny-unknown-fields` review (2026-09-04).

**Why:** stricter parsing converts a *scoped silent* failure (one field dropped) into a *total*
failure (the whole file's asset is `None`). The blast radius always grows. Whether that is a net
win for the designer depends entirely on the diagnostic at the failure site, which in this repo is
inconsistent per file type.

**How to apply — the five `Action`-bearing authoring surfaces and their real failure behaviour:**

| Surface | Runtime failure path | Runtime message | `ironhold_cli validate` |
|---|---|---|---|
| `logic/rules.ron` | `project_loader.rs` `LoadState::Failed(_)` arm | `warn!("rules failed to load — proceeding without it")` — **no path, error discarded**; game runs with *zero* rules | covered (`try_parse` → `FileResult.errors`) |
| `logic/state_machine.ron` | same arm, a few lines below | same weak shape | covered |
| `scenes/*.scene.ron` (`ActionSlotDef.do_actions`) | `spawn_scene_v2` `params.scenes.get(...)` never `Some` | none — stuck in `AppState::LoadingScene` forever | covered (`parse_file::<GameSceneV2>`) |
| `behaviors/*.behavior.ron` | `message_interpreter.rs` `let Some(fsm) = state_machines.get(..) else { continue }` | none — entity is simply inert | covered (`parse_file::<StateMachineAsset>`) |
| `dialogues/*.dialogue.ron` (`DialogueChoiceDef.do_actions`) | `dialogue.rs` `dialogue_assets.get(&handle) { None => return }` | none — panel never opens | **NOT covered — `do_validate` never globs `dialogues/`** |

Two structural takeaways to reuse:

1. **The catalog arms in `project_loader.rs` are the good pattern to copy** — `Failed(e)` +
   `asset_server.get_path(h)` + `error!("... {} — {} — proceeding with ...", path, e)`. The
   rules / state_machine / model_fixes arms are the stale `Failed(_)` + bare `warn!` shape and
   should be brought up to it whenever a change makes those files more likely to fail.
2. **Dialogue files are the standing blind spot** — the only Action-bearing surface with neither
   CLI coverage nor a runtime message. Any future strictness change to `Action`, `DialogueNodeDef`,
   or `DialogueChoiceDef` should add `glob_dir(project_dir, "dialogues", ".dialogue.ron")` to
   `do_validate` first. Extends [[validate-cross-file-blind-spots]] blind spot #4 (which covers
   `collect_actions` skipping dialogues) — the file is not even *parsed*, so it is a parse gap, not
   just a cross-check gap.

**Verdict calibration:** a pure `deny_unknown_fields` add is ALIGNED on the reachability axis by
construction. Downgrade to NEEDS WORK only when the newly-failing file type has a weak diagnostic
that the same change could cheaply fix. Do not treat "existing shipped RON might have a stray
field" as a review blocker — that is what the test suite + `cli validate` sweep is for; flag it as
a verification item instead.
