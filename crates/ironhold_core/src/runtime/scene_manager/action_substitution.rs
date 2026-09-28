use crate::schema::Action;

/// Substitutes `{self}` in action fields that can contain entity references.
/// Called by `entity_fsm_interpreter_system` before pushing actions onto the queue.
pub(crate) fn rewrite_self(action: Action, spawn_id: &str) -> Action {
    match action {
        Action::PlayAnimationOn { target, clip, start_at_fraction, freeze } => Action::PlayAnimationOn {
            target: target.replace("{self}", spawn_id),
            clip,
            start_at_fraction,
            freeze,
        },
        Action::EmitEvent(event) => Action::EmitEvent(event.replace("{self}", spawn_id)),
        Action::Despawn(id) => Action::Despawn(id.replace("{self}", spawn_id)),
        Action::SetDespawnTimer { entity, delay_secs } => Action::SetDespawnTimer {
            entity: entity.replace("{self}", spawn_id),
            delay_secs,
        },
        Action::Spawn { prefab, id, position, spawn_point, yaw_deg, at_entity } => Action::Spawn {
            prefab,
            id: id.map(|i| i.replace("{self}", spawn_id)),
            position,
            spawn_point: spawn_point.map(|s| s.replace("{self}", spawn_id)),
            yaw_deg,
            at_entity: at_entity.map(|e| e.replace("{self}", spawn_id)),
        },
        Action::ModifyStat { key, delta } => Action::ModifyStat {
            key: key.replace("{self}", spawn_id),
            delta,
        },
        Action::SetStat { key, value } => Action::SetStat {
            key: key.replace("{self}", spawn_id),
            value,
        },
        Action::ShowDamagePopup { entity, amount } => Action::ShowDamagePopup {
            entity: entity.replace("{self}", spawn_id),
            amount,
        },
        Action::ShowFloatingText { entity, text, offset } => Action::ShowFloatingText {
            entity: entity.replace("{self}", spawn_id),
            text: text.replace("{self}", spawn_id),
            offset,
        },
        Action::SetEntityVisible { entity, visible } => Action::SetEntityVisible {
            entity: entity.replace("{self}", spawn_id),
            visible,
        },
        Action::EmitEventAfterDelay { event, delay_secs } => Action::EmitEventAfterDelay {
            event: event.replace("{self}", spawn_id),
            delay_secs,
        },
        Action::SpawnEffect { key, position, entity } => Action::SpawnEffect {
            key,
            position,
            entity: entity.map(|e| e.replace("{self}", spawn_id)),
        },
        Action::ResetToSpawn(id) => Action::ResetToSpawn(id.replace("{self}", spawn_id)),
        Action::AddItem { entity, item_key, count } =>
            Action::AddItem { entity: entity.replace("{self}", spawn_id), item_key, count },
        Action::RemoveItem { entity, item_key, count } =>
            Action::RemoveItem { entity: entity.replace("{self}", spawn_id), item_key, count },
        Action::TransferItem { from, to, item_key, count } =>
            Action::TransferItem {
                from: from.replace("{self}", spawn_id),
                to: to.replace("{self}", spawn_id),
                item_key,
                count,
            },
        Action::OpenShop(id) => Action::OpenShop(id.replace("{self}", spawn_id)),
        Action::OpenContainer(id) => Action::OpenContainer(id.replace("{self}", spawn_id)),
        other => other,
    }
}

/// Substitutes `{target}` in action fields with the current target's spawn ID.
/// Called by all interpreter systems before pushing actions onto the queue.
/// If `target_id` is empty (no current target), `{target}` is left as-is and a
/// debug message is logged — the action executor will likely fail gracefully.
pub(crate) fn rewrite_target(action: Action, target_id: &str) -> Action {
    fn sub(v: &str, t: &str) -> String { v.replace("{target}", t) }
    match action {
        Action::ModifyStat { key, delta } =>
            Action::ModifyStat { key: sub(&key, target_id), delta },
        Action::SetStat { key, value } =>
            Action::SetStat { key: sub(&key, target_id), value },
        Action::SetVariable(key, value) =>
            Action::SetVariable(sub(&key, target_id), sub(&value, target_id)),
        Action::SpawnEffect { key, position, entity } =>
            Action::SpawnEffect { key, position, entity: entity.map(|e| sub(&e, target_id)) },
        Action::ShowDamagePopup { entity, amount } =>
            Action::ShowDamagePopup { entity: sub(&entity, target_id), amount },
        Action::ShowFloatingText { entity, text, offset } =>
            Action::ShowFloatingText { entity: sub(&entity, target_id), text: text.replace("{target}", target_id), offset },
        Action::SetEntityVisible { entity, visible } =>
            Action::SetEntityVisible { entity: sub(&entity, target_id), visible },
        Action::ResetToSpawn(id) => Action::ResetToSpawn(sub(&id, target_id)),
        Action::Despawn(id) => Action::Despawn(sub(&id, target_id)),
        Action::EmitEvent(event) => Action::EmitEvent(sub(&event, target_id)),
        Action::EmitEventAfterDelay { event, delay_secs } =>
            Action::EmitEventAfterDelay { event: sub(&event, target_id), delay_secs },
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
        Action::CloseContainer => Action::CloseContainer,
        other => other,
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