# Memory Index

- [Docs lag the action schema](project_docs_lag_actions.md) — docs/20_data_formats.md, docs/30_runtime_events_and_logic.md, docs/STATUS.md consistently miss new Action variants when added
- [pkg/ web build must be rebuilt](project_pkg_rebuild_required.md) — staged schema/action changes do not reach designers until wasm-pack build + commit of pkg/
- [Color tuples vary RGB vs RGBA](project_color_tuple_inconsistency.md) — DamagePopupStyle uses 3-tuple RGB while StatLabelDef/WorldStatBarDef use 4-tuple RGBA in the same prefab block
- [{self} substitution pattern](project_self_substitution_pattern.md) — Entity-targeted actions accept {self} in .behavior.ron; canonical example is primitive_world/behaviors/attack_dummy.behavior.ron
- [EffectDef `layers` field](project_effectdef_layers.md) — multi-layer emitter list; canonical multi-layer example is particles_demo `campfire_fire`; canonical single-layer is primitive_world `campfire_fire`
- [Auto-written GameVariables undocumented](project_auto_written_gamevariables_undocumented.md) — capability-populated bind keys (targeting's target_display/target_name/target_id) live only in core CLAUDE.md, not docs/
- [Audio writes no GameVariable](project_audio_no_gamevariable.md) — ToggleMute/SetVolume only emit events; mute state must be bridged to a variable via SetVariable on audio.muted/unmuted for a Label to show it
- [NPC collider canonical example](project_npc_collider_canonical_example.md) — collider_height/radius worked example lives in 3rd_person_game_demo snake/spider prefabs, not docs' own orc_guard/rat examples
- [decals: map has two consumers](project_decals_map_two_consumers.md) — assets.ron decals: feeds BOTH Action::ProjectDecal and scene target_indicator; doc sections don't cross-link; texture: field resolves against decals not textures
- [AnimationPolicy doc gaps](project_animation_policy_gaps.md) — animation_sources undocumented as a field; PlayAnimationOn missing from actions table; clip-vs-id distinction unexplained
- [Target indicator color tiers](project_target_indicator_color_tiers.md) — 3-tier ring color (indicator_color > category > scene color); silent fallthrough undocumented; indicator_color & "ally" have no shipped example
- [CameraShake re-trigger ambiguity](project_camera_shake_retrigger_ambiguity.md) — re-trigger semantics documented 3 contradictory ways (restart vs merge+cap vs replace); verify doc against shipped executor
