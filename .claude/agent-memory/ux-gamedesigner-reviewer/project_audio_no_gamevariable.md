---
name: audio-no-gamevariable
description: Audio auto-writes ONLY audio_volume_percent (chosen preset, ignores mute/max_volume); mute still needs a SetVariable bridge on audio.muted/unmuted for a Label
metadata:
  type: project
---

**Shipped (feature/audio_volume_readout, reviewed 2026-10-06):** engine auto-writes `audio_volume_percent` (chosen preset 0-100 as integer string, "100" from frame 1, ignores mute and `max_volume`, survives LoadScene, resets on project load). Reserved key: a designer `SetVariable` on it sticks only until the next AudioState change (SetVolume/ToggleMute/SyncAudioState) - it's compared and rewritten. Canonical example: 3rd_person_game_demo options.scene.ron `audio_heading` (`format: "Volume: {}%"`). Documented in docs/20 Label auto-written table (~900), SetVolume/SyncAudioState action rows (~3915), AudioConfig section (~4690), docs/30 (~123), STATUS.md (~98).

**Mute is still NOT auto-written.** Data-only bridge in `state_machine.ron`:
```ron
( event: "audio.muted",   do_actions: [ SetVariable("audio_state", "Muted") ] ),
( event: "audio.unmuted", do_actions: [ SetVariable("audio_state", "Sound On") ] ),
```
plus `SyncAudioState` in entry_actions to initialise. The `audio_` prefix mixes one engine-written key with demo-authored `audio_state`/`audio_muted` - docs now say which is which. `bind` takes ONE key, so "Volume: 50% - Muted" in one label is impossible.

**How to apply:** Any audio/mute UX review should check the event->SetVariable bridge for mute. If UI says "Toggle Mute" with no bound Label, flag missing state feedback. Recommend the bridge rather than a new engine variable for mute (deliberate design choice).
