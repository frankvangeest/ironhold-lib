# Memory Index

- [Docs audience split](project_docs_audience_split.md) — Phase A shipped; code-span cross-refs dead on GitHub; Getting-started assumes repo clone; no prebuilt CLI

- [opencode probe plan-review](project_opencode_probe_plan_review.md) — dev-only doc must enter docs/README via dev rows only; paid model in default fallback list; nvs recipe
- [opencode GLM driver plan-review](project_opencode_glm_driver_plan_review.md) — global<project merge breaks m365 option (a); build fails "free unless -deep"; "-alt provider" undefined

- [ocean demo plan-review](project_ocean_demo_plan_review.md) — compass vs rotation_euler_deg clash; SetParam keys must mirror RON paths; boat input vs InputMap; readout precision

- [lock-on plan-review](project_lock_on_plan_review.md) — no per-player target-clear input (gamepad can't unlock); player-in-frame unguaranteed; camera_modes prefabs default Tab

- [step_height plan-review](project_step_height_plan_review.md) — tread rule unmeasured (~2x radius); p1_split_ring shared room9/10; seed opt-ins in blank_project

- [Substitution token surfaces](project_substitution_token_surfaces.md) — no single token table in docs; no validate check for unresolvable/typo tokens; tokens never capture in on: patterns
- [Despawn fade design](project_despawn_fade_design.md) — fade_secs is ADDITIVE to delay_secs; Shrink fallback console-only; target-ring must clear at fade start

- [Action bar click activation](project_action_bar_click_activation.md) — slots clickable, owner_player not viewport; 4 key-only wording hotspots to grep on slot-input changes

- [Fixed-tick pipeline review](project_fixed_tick_pipeline_review.md) — 64 Hz timers; pause freezes menu-armed delays; list of "per frame" doc surfaces to flip
- [Same-frame event order](project_same_frame_event_order.md) — real order: UI > last-frame carry-over > FixedUpdate > Emit chain > Scene; D3 fixes only the Emit slice
- [FSM designer traps](project_fsm_designer_traps.md) — `on` x3; initial_state entry skip; transitions carry no actions; struct-variant actions miswritten; stale old-syntax grep recipe

- [entity.* event doc surfaces](project_entity_event_doc_surfaces.md) — 6 places a new entity.* event/InteractableDef field must land; STATUS.md event list + docs/60 check list lag most
- [ShowFloatingText tracks its entity](project_floating_text_tracks_entity.md) — pairing it with Despawn of the same entity in one do_actions list = zero visible frames, no warning
- [Pause is cosmetic](project_pause_is_cosmetic.md) — pause menu is an overlay only; nothing in the sim stops; 4 doc surfaces claim otherwise; no UI focus/keyboard-nav concept exists anywhere

- [My top-5 UX wishlist status](project_ux_wishlist_status.md) — stakeholder list ownership; 2026-10-01 refresh: pause, load-hang, demos, {target}->"", motion: docs
- [Hot-join input/prefab coupling](project_hot_join_input_prefab_coupling.md) — per-slot join prefabs for keyboard seats; gamepad binds ONLY at join time; canonical room8; several older gaps now closed
- [Player-count-change assumptions](project_player_count_change_assumptions.md) — seat index vs viewport slot conflated at join; "2+ players" gating assumes count is fixed per scene; both break on leave
- [CameraConfig party/split nesting](project_camera_config_party_split_nesting.md) — party:/split: now SIBLING fields of components.camera (nested location is legacy fallback only); flycam is TAG-driven not field-driven; doc surface + local_coop_demo migration constraints
- [Flycam spectator priority](project_flycam_spectator_priority.md) — flycam wins camera; shared-WASD; 4 doc spots to sync; shipped model/children+shape/primitive+dual-tag diagnostics, all documented
- [camera_mode reachability matrix](project_camera_mode_reachability_matrix.md) — HISTORICAL/fixed; keep for the "check WHICH spawn path the example exercises" review lesson
- [Camera transition & "default" sentinel](project_camera_transition_and_default_sentinel.md) — transition: is read from the TARGET mode, so "default" round-trips snap back; fov has 3 different defaults (45/60/90)
- [RON enum double-paren trap](project_ron_enum_double_paren.md) — enum variants wrapping a named struct need Orbit((field: value)); single-paren examples fail to parse; only cli validate catches it
- [Docs lag the action schema](project_docs_lag_actions.md) — 5 doc surfaces to check for new Action variants AND new optional fields; docs/20 table usually fine, the other four lag; general methodology, not a stale instance list
- [validate coverage gaps](project_validate_coverage_gaps.md) — CLI validate checks key-lookup fields asymmetrically (initial_items, ItemDef.currency_stat uncovered; ToggleOverlay now covered); merchant checks silently skip with no items_path
- [pkg/ web build must be rebuilt](project_pkg_rebuild_required.md) — staged schema/action changes do not reach designers until wasm-pack build + commit of pkg/
- [{self} substitution pattern](project_self_substitution_pattern.md) — Entity-targeted actions accept {self} in .behavior.ron; canonical example is primitive_world/behaviors/attack_dummy.behavior.ron
- [EffectDef `layers` field](project_effectdef_layers.md) — multi-layer emitter list; canonical multi-layer example is particles_demo `campfire_fire`; canonical single-layer is primitive_world `campfire_fire`
- [Auto-written GameVariables undocumented](project_auto_written_gamevariables_undocumented.md) — capability-populated bind keys (targeting's target_display/target_name/target_id) live only in core CLAUDE.md, not docs/
- [Audio GameVariables](project_audio_no_gamevariable.md) — only audio_volume_percent is engine-written (preset, not effective); mute still needs SetVariable bridge
- [NPC collider canonical example](project_npc_collider_canonical_example.md) — collider_height/radius worked example lives in 3rd_person_game_demo snake/spider prefabs, not docs' own orc_guard/rat examples
- [decals: map has two consumers](project_decals_map_two_consumers.md) — assets.ron decals: feeds BOTH Action::ProjectDecal and scene target_indicator; doc sections don't cross-link; texture: field resolves against decals not textures
- [AnimationPolicy doc gaps](project_animation_policy_gaps.md) — 4 old gaps CLOSED (incl. un-freezing); still no policy field table, clips-alias looping undocumented
- [dynamic_animation_control demo](project_dynamic_animation_control_demo.md) — canonical seek/freeze demo; flycam default speed 100 is wrong for dioramas; moving scenes must go in test_web NON_DETERMINISTIC_SCENES
- [World label legibility](project_world_label_legibility.md) — captions are fixed screen-px & never wrap; px/m depends only on viewport HEIGHT; camera-back makes overlap WORSE; short-token+legend pattern; field table gap now closed (docs/20 ~380-403)
- [Em-dash font glyph gap](project_em_dash_font_glyph_gap.md) — engine font has no `—` glyph, renders as tofu box in ANY project's in-game text; always flag it, recommend ASCII hyphen `-`
- [Target indicator color tiers](project_target_indicator_color_tiers.md) — 3-tier ring color (indicator_color > category > scene color); silent fallthrough now documented (docs/20 ~506); shipped categories are hostile/neutral/friendly; indicator_color still has no shipped example
- [CameraShake re-trigger ambiguity](project_camera_shake_retrigger_ambiguity.md) — RESOLVED: confirmed REPLACES (insert overwrites, no merge/cap) against shipped action_executor.rs
- [Dialogue system doc gaps](project_dialogue_system_doc_gaps.md) — hint_text undocumented on InteractableDef; dialogue.started payload is spawn id not prefab; condition absent-key semantics unexplained; canonical example 3rd_person_game_demo
- [Inventory & item system](project_inventory_item_system.md) — v1 shops display-only; entity:"player" magic string vs spawn id; currency_stat "gold" now a real stat, merchant uses unquoted kind; canonical 3rd_person_game_demo
- [Nameplate system](project_nameplate_system.md) — {self}.stat needs a matching stat_template; player uses global player_health so its forced nameplate shows NO bars; mana bar shows on nothing
- [Local co-op system](project_local_coop_system.md) — Stage 1 single-machine co-op; first-scene-entity-wins party camera; player_index/gamepad_index/max_view_box; canonical local_coop_demo; distinct from unshipped LAN networking
- [Split-screen doc gap](project_split_screen_doc_gap.md) — split-screen internal fixes update only CLAUDE.md, not designer-facing docs/20_data_formats.md; v2 Ascii-vs-Pixel asymmetry needs a note
- [Per-player targeting gating](project_per_player_targeting_gating.md) — party-mode target_hud dead-end now documented (docs/20 ~591-593); remaining live footgun is target_next default "Tab" breaking in WASM
- [depth_scale field scope](project_depth_scale_field_scope.md) — per-label depth_scale exists ONLY on EntityLabelDef/WorldLabelDef; stat widgets + nameplates only inherit; holds the clamp(ref/dist, min_scale, 1.0) formula and its never-grows limitation
- [ActionBar single-player assumptions](project_action_bar_single_player_assumptions.md) — gamepad_key shipped; missing-gamepad_index is now a hard validate error, still-open face-button collision footgun remains; scene-wide slot_key collisions; doc-location trap for co-op bar features
- [owner_player ↔ player_index wiring](project_player_index_owner_player.md) — owner_player matches prefab player_index; terminology now cross-linked in docs (~1010, ~2514-2515); canonical example local_coop_demo room3/room11
- [world_stat_bar style landscape](project_world_stat_bar_style_landscape.md) — FOUR styles; bars+popups+floating text duplicate in split-screen, ONLY nameplates single; 5 docs/20 surfaces to keep in sync
- [Particle warmup recipe](project_particle_warmup_recipe.md) — 3 pipeline kinds vs designer fields; canonical particles_demo scene.ready; no campfire_body key; Ambient-priority/entry_actions traps
- [Warn vs silent fallback principle](project_warn_vs_silent_fallback_principle.md) — engine warns when authored intent is contradictory (cross-bar dup, dup player_index, missing animation_policy, gamepad_key without gamepad_index now a hard error too), silent when fallback is a legit common choice; prefer load/validate-time over per-frame
- [Primitive-player fields](project_primitive_player_fields.md) — which PrefabDef fields apply to tags:["player"] prefabs; room7 = bare-capsule baseline, room10 = mixed GLB+composed-body example
- [Gamepad input system](project_gamepad_input_system.md) — additive to keyboard; gamepad_index is a bind-once SEED (not a live slot); duplicate_gamepad_index check shipped AND documented; unclaimed-pad footgun
- [Schema bool-toggle house style](project_schema_bool_toggle_house_style.md) — binary opt-in fields are bools (default false), not two-variant enums; ~19 bool precedents vs enums-for-multistate-only; recommend bool for new on/off fields
- [Split-switch prefab duplication](project_split_switch_prefab_duplication.md) — split switches live on the first player's prefab & scene entities can't override components, so each variant demo clones the whole prefab pair (and drifts)
- [local_coop_demo room conventions](project_local_coop_demo_room_conventions.md) — 10-room portal chain, no ?scene= deep-link, exits must be listed, 22px font wraps-into-neighbour past a verified 82-char ceiling
- [screen_offset stacking pattern](project_screen_offset_stacking.md) — shared world `offset` + pixel `screen_offset`; defaults 2.4/2.5/2.8 mismatch; 72px/m is a migration artifact, not a rule
- [RON comments cite dev-only paths](project_ron_comments_cite_dev_paths.md) — asset RON comments point at planning/*.md and Rust doc comments designers can't open; rewrite as docs/20 references
- [Jump re-arm coupling](project_jump_rearm_coupling.md) — jump/ground-cast invariant; playtest targets: 3rd_person is FLAT, quick_scene=slopes, primitive_world=only jump-sound canary
- [Quoted-string vs enum house style](project_quoted_string_vs_enum_house_style.md) — orbit_button:"Right" (string) vs velocity_curve:EaseOut (unquoted enum) both ship; new easing must match velocity_curve; CLI must validate string keys
- [Corpse loot v2 pattern](project_corpse_loot_v2_pattern.md) — docs/30 section now REWRITTEN to v2 (not stale); gap is the missing 7-artifact "add a 4th monster" checklist
- [container.* events & loot gotchas](project_container_events_undocumented.md) — events now in docs/30; trigger_zone needs explicit entity.exited handler; initial_items never refill after loot
- [ui: Label/Button font & clip](project_ui_label_button_font_and_clip.md) — font_size house style is f32+default fn (13 precedents); 22/26px, ~11px/char; clip+wrap+center = half-cut lines; camera_modes hints overflow today
- [RON parse-failure diagnostics](project_ron_parse_failure_diagnostics.md) — ron 0.11 error text is GOOD (names the Action variant); logic/catalog/.behavior/.dialogue loaders now error! w/ path; .project.ron hangs loading; blast radius = whole file
- [UI trigger wiring](project_ui_trigger_wiring.md) — 4 surfaces emit ui.button_pressed:{trigger}; exact-match only; `unreachable_trigger` check's blind spots (gamepad, state gating, nested behaviors, parse cascade)
- [max_frame_delta_secs](project_max_fixed_delta_secs.md) — renamed from max_fixed_; validate now enforces ~0.03..60s; doc surfaces list
- [Default mouse-binding test projects](project_mouse_default_binding_test_projects.md) — 3rd_person/local_coop override orbit_button; primitive_world/stats_demo use defaults + have ActionBars
- [UI nesting breaks 11 flat scene.ui scans](project_ui_nesting_flat_scan_sites.md) — StatRadar pre-pass + 4 warns + 6 CLI checks skip nested children; Container/ContainerPanel name clash; auto-size+SpaceBetween is a no-op; Group shipped 2026-10-06 (open doc nits listed)
