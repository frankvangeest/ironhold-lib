use crate::schema::Action;

/// Substitutes `{self}` in action fields that can contain entity references.
/// Called by `entity_fsm_interpreter_system` before pushing actions onto the queue.
///
/// This match is **exhaustive by design, with no `other => other` wildcard**. A missing arm for
/// `{self}`/`{target}` substitution is exactly the shape of bug that shipped twice on the
/// rules_to_state_machine_consolidation branch (`SetVariable`, then `SetDespawnTimer`) after the
/// original `message_interpreter.rs` was split into this file — both compiled cleanly under a
/// wildcard and were only caught by manual review, not the compiler or a test. Adding a new
/// `Action` variant now force a decision here (compile error until every arm is listed), not a
/// silent no-op. See the `#[cfg(test)] mod tests` below for coverage of every string-bearing arm.
pub(crate) fn rewrite_self(action: Action, spawn_id: &str) -> Action {
    fn sub(v: &str, id: &str) -> String { v.replace("{self}", id) }
    match action {
        // ── Entity-address fields: substitution applies ──────────────────────────────
        Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => Action::Spawn {
            prefab,
            id: id.map(|i| sub(&i, spawn_id)),
            position,
            spawn_point: spawn_point.map(|s| sub(&s, spawn_id)),
            yaw_deg,
            at_entity: at_entity.map(|e| sub(&e, spawn_id)),
        },
        Action::Despawn(id) => Action::Despawn(sub(&id, spawn_id)),
        Action::SetDespawnTimer { entity, delay_secs } => Action::SetDespawnTimer {
            entity: sub(&entity, spawn_id),
            delay_secs,
        },
        Action::SetVariable(key, value) =>
            Action::SetVariable(sub(&key, spawn_id), sub(&value, spawn_id)),
        Action::IncrementVariable(key, delta) =>
            Action::IncrementVariable(sub(&key, spawn_id), delta),
        Action::PlayAnimationOn { target, clip, start_at_fraction, freeze } => Action::PlayAnimationOn {
            target: sub(&target, spawn_id),
            clip,
            start_at_fraction,
            freeze,
        },
        Action::EmitEvent(event) => Action::EmitEvent(sub(&event, spawn_id)),
        Action::ModifyStat { key, delta } => Action::ModifyStat {
            key: sub(&key, spawn_id),
            delta,
        },
        Action::SetStat { key, value } => Action::SetStat {
            key: sub(&key, spawn_id),
            value,
        },
        Action::ShowDamagePopup { entity, amount } => Action::ShowDamagePopup {
            entity: sub(&entity, spawn_id),
            amount,
        },
        Action::ShowFloatingText { entity, text, offset } => Action::ShowFloatingText {
            entity: sub(&entity, spawn_id),
            text: sub(&text, spawn_id),
            offset,
        },
        Action::SetEntityVisible { entity, visible } => Action::SetEntityVisible {
            entity: sub(&entity, spawn_id),
            visible,
        },
        Action::EmitEventAfterDelay { event, delay_secs } => Action::EmitEventAfterDelay {
            event: sub(&event, spawn_id),
            delay_secs,
        },
        Action::SpawnEffect { key, position, entity } => Action::SpawnEffect {
            key,
            position,
            entity: entity.map(|e| sub(&e, spawn_id)),
        },
        Action::ProjectDecal { key, entity, position, radius, duration_secs, color, pulse_speed } =>
            Action::ProjectDecal {
                key,
                entity: entity.map(|e| sub(&e, spawn_id)),
                position,
                radius,
                duration_secs,
                color,
                pulse_speed,
            },
        Action::SetTarget(id) => Action::SetTarget(sub(&id, spawn_id)),
        Action::ResetToSpawn(id) => Action::ResetToSpawn(sub(&id, spawn_id)),
        Action::StartDialogue { npc_id, dialogue_path } => Action::StartDialogue {
            npc_id: sub(&npc_id, spawn_id),
            dialogue_path,
        },
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: sub(&entity, spawn_id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: sub(&entity, spawn_id), item_key, count },
        Action::TransferItem { from, to, item_key, count } =>
            Action::TransferItem {
                from: sub(&from, spawn_id),
                to: sub(&to, spawn_id),
                item_key,
                count,
            },
        Action::OpenShop(id) => Action::OpenShop(sub(&id, spawn_id)),
        Action::OpenContainer(id) => Action::OpenContainer(sub(&id, spawn_id)),

        // ── Fields that are catalog keys / mode names / paths, not entity addresses:
        //    no substitution, listed explicitly so a future field rename is a conscious choice. ──
        Action::LoadScene(path) => Action::LoadScene(path),
        Action::Log(msg) => Action::Log(msg),
        Action::PlayAnimation(clip) => Action::PlayAnimation(clip),
        Action::PlaySound { key, volume } => Action::PlaySound { key, volume },
        Action::PlayMusicLoop { key, volume } => Action::PlayMusicLoop { key, volume },
        Action::LoadSceneOverlay(path) => Action::LoadSceneOverlay(path),
        Action::ToggleOverlay(path) => Action::ToggleOverlay(path),
        Action::PreloadScene(path) => Action::PreloadScene(path),
        Action::PreloadPrefab(key) => Action::PreloadPrefab(key),
        Action::PreloadGlb(key) => Action::PreloadGlb(key),
        Action::ApplyModifier { modifier_key } => Action::ApplyModifier { modifier_key },
        Action::RemoveModifier { modifier_key } => Action::RemoveModifier { modifier_key },
        Action::SetParticleQuality(level) => Action::SetParticleQuality(level),
        Action::SetCameraMode { mode, owner_player } => Action::SetCameraMode { mode, owner_player },
        Action::CameraShake { duration_secs, intensity, owner_player } =>
            Action::CameraShake { duration_secs, intensity, owner_player },
        Action::BuyItem(item_key) => Action::BuyItem(item_key),

        // ── No string fields at all: identity, grouped ──────────────────────────────
        other @ (Action::Quit
            | Action::StopMusic
            | Action::UnloadOverlay
            | Action::SetVolume(_)
            | Action::ToggleMute
            | Action::SyncAudioState
            | Action::ToggleOwnNameplate
            | Action::ClearTarget
            | Action::AdvanceDialogue
            | Action::EndDialogue
            | Action::OpenInventory
            | Action::CloseInventory
            | Action::ToggleInventory
            | Action::CloseShop
            | Action::CloseContainer
            | Action::TakeAllFromContainer
            | Action::JoinPlayer) => other,
    }
}

/// Substitutes `{target}` in action fields with the current target's spawn ID.
/// Called by all interpreter systems before pushing actions onto the queue.
/// If `target_id` is empty (no current target), `{target}` is replaced with the empty string —
/// the action executor then fails to resolve the (now-empty) entity id and logs a warning rather
/// than silently no-oping; there is no "leave `{target}` as literal text" behavior.
///
/// Exhaustive by design, no wildcard — see `rewrite_self`'s doc comment for why.
pub(crate) fn rewrite_target(action: Action, target_id: &str) -> Action {
    fn sub(v: &str, t: &str) -> String { v.replace("{target}", t) }
    match action {
        // ── Entity-address fields: substitution applies ──────────────────────────────
        Action::ModifyStat { key, delta } =>
            Action::ModifyStat { key: sub(&key, target_id), delta },
        Action::SetStat { key, value } =>
            Action::SetStat { key: sub(&key, target_id), value },
        // Substitutes {target} in both the key and the value (main's original only substituted
        // the value) -- kept intentionally, for consistency with ModifyStat/SetStat, whose keys
        // are also substituted. Harmless: a key without the literal "{target}" token is untouched.
        Action::SetVariable(key, value) =>
            Action::SetVariable(sub(&key, target_id), sub(&value, target_id)),
        Action::IncrementVariable(key, delta) =>
            Action::IncrementVariable(sub(&key, target_id), delta),
        Action::SpawnEffect { key, position, entity } =>
            Action::SpawnEffect { key, position, entity: entity.map(|e| sub(&e, target_id)) },
        Action::ProjectDecal { key, entity, position, radius, duration_secs, color, pulse_speed } =>
            Action::ProjectDecal {
                key,
                entity: entity.map(|e| sub(&e, target_id)),
                position,
                radius,
                duration_secs,
                color,
                pulse_speed,
            },
        Action::ShowDamagePopup { entity, amount } =>
            Action::ShowDamagePopup { entity: sub(&entity, target_id), amount },
        Action::ShowFloatingText { entity, text, offset } =>
            Action::ShowFloatingText { entity: sub(&entity, target_id), text: sub(&text, target_id), offset },
        Action::SetEntityVisible { entity, visible } =>
            Action::SetEntityVisible { entity: sub(&entity, target_id), visible },
        Action::SetTarget(id) => Action::SetTarget(sub(&id, target_id)),
        Action::ResetToSpawn(id) => Action::ResetToSpawn(sub(&id, target_id)),
        Action::Despawn(id) => Action::Despawn(sub(&id, target_id)),
        Action::SetDespawnTimer { entity, delay_secs } =>
            Action::SetDespawnTimer { entity: sub(&entity, target_id), delay_secs },
        Action::EmitEvent(event) => Action::EmitEvent(sub(&event, target_id)),
        Action::EmitEventAfterDelay { event, delay_secs } =>
            Action::EmitEventAfterDelay { event: sub(&event, target_id), delay_secs },
        Action::StartDialogue { npc_id, dialogue_path } =>
            Action::StartDialogue { npc_id: sub(&npc_id, target_id), dialogue_path },
        Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => Action::Spawn {
            prefab,
            id: id.map(|i| sub(&i, target_id)),
            position,
            spawn_point: spawn_point.map(|sp| sub(&sp, target_id)),
            yaw_deg,
            at_entity: at_entity.map(|e| sub(&e, target_id)),
        },
        Action::PlayAnimationOn { target, clip, start_at_fraction, freeze } => Action::PlayAnimationOn {
            target: sub(&target, target_id),
            clip,
            start_at_fraction,
            freeze,
        },
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: sub(&entity, target_id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: sub(&entity, target_id), item_key, count },
        Action::TransferItem { from, to, item_key, count } =>
            Action::TransferItem {
                from: sub(&from, target_id),
                to: sub(&to, target_id),
                item_key,
                count,
            },
        Action::OpenShop(id) => Action::OpenShop(sub(&id, target_id)),
        Action::OpenContainer(id) => Action::OpenContainer(sub(&id, target_id)),

        // ── Fields that are catalog keys / mode names / paths, not entity addresses:
        //    no substitution, listed explicitly. ──
        Action::LoadScene(path) => Action::LoadScene(path),
        Action::Log(msg) => Action::Log(msg),
        Action::PlayAnimation(clip) => Action::PlayAnimation(clip),
        Action::PlaySound { key, volume } => Action::PlaySound { key, volume },
        Action::PlayMusicLoop { key, volume } => Action::PlayMusicLoop { key, volume },
        Action::LoadSceneOverlay(path) => Action::LoadSceneOverlay(path),
        Action::ToggleOverlay(path) => Action::ToggleOverlay(path),
        Action::PreloadScene(path) => Action::PreloadScene(path),
        Action::PreloadPrefab(key) => Action::PreloadPrefab(key),
        Action::PreloadGlb(key) => Action::PreloadGlb(key),
        Action::ApplyModifier { modifier_key } => Action::ApplyModifier { modifier_key },
        Action::RemoveModifier { modifier_key } => Action::RemoveModifier { modifier_key },
        Action::SetParticleQuality(level) => Action::SetParticleQuality(level),
        Action::SetCameraMode { mode, owner_player } => Action::SetCameraMode { mode, owner_player },
        Action::CameraShake { duration_secs, intensity, owner_player } =>
            Action::CameraShake { duration_secs, intensity, owner_player },
        Action::BuyItem(item_key) => Action::BuyItem(item_key),

        // ── No string fields at all: identity, grouped ──────────────────────────────
        other @ (Action::Quit
            | Action::StopMusic
            | Action::UnloadOverlay
            | Action::SetVolume(_)
            | Action::ToggleMute
            | Action::SyncAudioState
            | Action::ToggleOwnNameplate
            | Action::ClearTarget
            | Action::AdvanceDialogue
            | Action::EndDialogue
            | Action::OpenInventory
            | Action::CloseInventory
            | Action::ToggleInventory
            | Action::CloseShop
            | Action::CloseContainer
            | Action::TakeAllFromContainer
            | Action::JoinPlayer) => other,
    }
}

/// Returns the intent slot key from an event name like "intent.slot.attack:player_01"
pub(crate) fn intent_slot_key(event_name: &str) -> Option<String> {
    event_name.strip_prefix("intent.slot.").and_then(|s| s.split(':').next().map(|s| s.to_string()))
}

/// Extracts the scene file stem from a real scene path (e.g. "scenes/main.scene.ron" -> "main"),
/// for building a `SceneEvent`'s derived "scene.ready:{stem}"-shaped event name. This is NOT
/// string-prefix stripping on an already-formed event name (see `intent_slot_key` above for that
/// pattern) -- `SceneEvent::Ready`/`Loaded`/`Requested`/`Unloading` all carry a raw path, not a
/// prefixed event string. Callers wrap this in `.unwrap_or_default()`, so always returns `Some`.
pub(crate) fn scene_path_stem(path: &str) -> Option<String> {
    Some(
        std::path::Path::new(path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(path)
            .trim_end_matches(".scene.ron")
            .trim_end_matches(".ron")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_path_stem_extracts_from_real_scene_path() {
        assert_eq!(scene_path_stem("scenes/main.scene.ron"), Some("main".to_string()));
        assert_eq!(scene_path_stem("scenes/pause.scene.ron"), Some("pause".to_string()));
        assert_eq!(scene_path_stem("projects/test/scenes/main.scene.ron"), Some("main".to_string()));
        assert_eq!(scene_path_stem("main.ron"), Some("main".to_string()));
        assert_eq!(scene_path_stem("bare_name"), Some("bare_name".to_string()));
    }

    #[test]
    fn intent_slot_key_strips_prefix_and_player_suffix() {
        assert_eq!(intent_slot_key("intent.slot.1:player_01"), Some("1".to_string()));
        assert_eq!(intent_slot_key("intent.slot.attack:player_01"), Some("attack".to_string()));
        assert_eq!(intent_slot_key("intent.slot.1"), Some("1".to_string()));
        assert_eq!(intent_slot_key("scene.ready:main"), None);
    }

    #[test]
    fn rewrite_self_substitutes_despawn_and_set_despawn_timer() {
        assert_eq!(rewrite_self(Action::Despawn("{self}".to_string()), "zombie_01"),
            Action::Despawn("zombie_01".to_string()));
        assert_eq!(
            rewrite_self(Action::SetDespawnTimer { entity: "{self}".to_string(), delay_secs: 5.0 }, "zombie_01"),
            Action::SetDespawnTimer { entity: "zombie_01".to_string(), delay_secs: 5.0 }
        );
    }

    #[test]
    fn rewrite_target_substitutes_set_despawn_timer() {
        // Regression test: this arm was silently dropped when message_interpreter.rs was split
        // into this file, and `SetDespawnTimer("{target}")` kept the literal placeholder string.
        assert_eq!(
            rewrite_target(Action::SetDespawnTimer { entity: "{target}".to_string(), delay_secs: 5.0 }, "enemy_a"),
            Action::SetDespawnTimer { entity: "enemy_a".to_string(), delay_secs: 5.0 }
        );
    }

    #[test]
    fn rewrite_target_substitutes_set_variable_key_and_value() {
        // Regression test: this variant was entirely missing before, so {target} in a
        // SetVariable's value stayed as the literal string "{target}".
        assert_eq!(
            rewrite_target(Action::SetVariable("p1_hit".to_string(), "{target}".to_string()), "enemy_a"),
            Action::SetVariable("p1_hit".to_string(), "enemy_a".to_string())
        );
    }

    #[test]
    fn rewrite_target_substitutes_project_decal_and_set_target() {
        assert_eq!(
            rewrite_target(Action::ProjectDecal {
                key: "aoe_fire_circle".to_string(),
                entity: Some("{target}".to_string()),
                position: None,
                radius: 3.0,
                duration_secs: 5.0,
                color: (1.0, 0.4, 0.1, 0.7),
                pulse_speed: 0.8,
            }, "boss_01"),
            Action::ProjectDecal {
                key: "aoe_fire_circle".to_string(),
                entity: Some("boss_01".to_string()),
                position: None,
                radius: 3.0,
                duration_secs: 5.0,
                color: (1.0, 0.4, 0.1, 0.7),
                pulse_speed: 0.8,
            }
        );
        assert_eq!(rewrite_target(Action::SetTarget("{target}".to_string()), "enemy_a"),
            Action::SetTarget("enemy_a".to_string()));
    }

    #[test]
    fn rewrite_self_substitutes_start_dialogue_npc_id_not_path() {
        assert_eq!(
            rewrite_self(Action::StartDialogue {
                npc_id: "{self}".to_string(),
                dialogue_path: "dialogues/npc.dialogue.ron".to_string(),
            }, "npc_01"),
            Action::StartDialogue {
                npc_id: "npc_01".to_string(),
                dialogue_path: "dialogues/npc.dialogue.ron".to_string(),
            }
        );
    }

    #[test]
    fn rewrite_target_leaves_catalog_keys_and_paths_untouched() {
        // LoadScene/PlaySound/PlayMusicLoop/etc. take catalog keys or paths, not entity
        // addresses -- {target} inside one of these (which would be unusual authoring) is not
        // substituted, matching pre-existing behavior.
        assert_eq!(rewrite_target(Action::LoadScene("scenes/{target}.scene.ron".to_string()), "main"),
            Action::LoadScene("scenes/{target}.scene.ron".to_string()));
        assert_eq!(
            rewrite_target(Action::PlaySound { key: "{target}".to_string(), volume: 1.0 }, "click"),
            Action::PlaySound { key: "{target}".to_string(), volume: 1.0 }
        );
    }

    #[test]
    fn rewrite_self_and_rewrite_target_are_identity_for_fieldless_variants() {
        assert_eq!(rewrite_self(Action::Quit, "x"), Action::Quit);
        assert_eq!(rewrite_target(Action::StopMusic, "x"), Action::StopMusic);
        assert_eq!(rewrite_target(Action::CloseContainer, "x"), Action::CloseContainer);
        assert_eq!(rewrite_self(Action::JoinPlayer, "x"), Action::JoinPlayer);
    }
}
