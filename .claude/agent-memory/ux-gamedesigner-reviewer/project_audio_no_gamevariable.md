---
name: audio-no-gamevariable
description: ToggleMute/SetVolume emit events but write NO GameVariable — mute state cannot be data-bound to a Label directly; designer must bridge via SetVariable on audio.muted/unmuted
metadata:
  type: project
---

`Action::ToggleMute` and `Action::SetVolume` only mutate internal `audio_state` (in action_executor.rs ~line 225-248) and emit `GameEvent::Trigger`: `audio.muted`, `audio.unmuted`, `audio.volume_changed`. They do NOT write any `GameVariables` key.

**Why this matters for designers:** there is no `audio_muted` / `muted` / `volume` variable a `Label((bind: ...))` can read. Unlike the targeting capability (which auto-writes `target_display` etc., see [[auto-written-gamevariables-undocumented]]), audio exposes state ONLY as transient events.

**The data-only workaround (no engine change needed):** in `state_machine.ron` / `rules.ron`, react to the audio events and mirror state into a variable:
```ron
( event: "audio.muted",   do_actions: [ SetVariable("audio_state", "Muted") ] ),
( event: "audio.unmuted", do_actions: [ SetVariable("audio_state", "Sound On") ] ),
```
then `Label((bind: "audio_state", format: "{}"))`. Canonical proof-of-pattern: `docs/20_data_formats.md:595` (action_bar status uses the same SetVariable-on-event trick) and `3rd_person_game_demo` main.scene.ron target_label (bind+format).

**How to apply:** Any review of an audio/mute UX feature should check whether the designer wired the event→SetVariable bridge. If the UI says "Toggle Mute" with no bound Label, flag missing visual state feedback as a friction/blocker. Recommend the bridge rather than asking for a new engine GameVariable.
