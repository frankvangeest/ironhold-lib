use bevy::prelude::*;
use crate::runtime::scene_manager::WorldLabel;

/// Lifetime tracker for a floating damage/heal number.
/// Spawned by `Action::ShowDamagePopup`; the entity also carries `WorldLabel + Text2d`.
/// `damage_popup_system` rises the label's world offset, fades its alpha, and despawns
/// the entity when `elapsed >= duration`.
#[derive(Component)]
pub struct DamagePopup {
    pub elapsed: f32,
    pub duration: f32,
    /// Metres per second the label rises. Sourced from `ProjectConfig.damage_popup_style`.
    pub rise_speed: f32,
}

/// Advances all `DamagePopup` entities: rises their `WorldLabel` offset, fades alpha,
/// and despawns them when their lifetime expires.
/// Registered `.before(world_label_screen_pos_system)` so the updated offset is
/// projected to screen space in the same frame it was changed.
pub fn damage_popup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DamagePopup, &mut WorldLabel, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut popup, mut label, mut color) in query.iter_mut() {
        popup.elapsed += dt;
        let t = (popup.elapsed / popup.duration).min(1.0);
        label.offset.y += dt * popup.rise_speed;
        let new_alpha = (1.0 - t).max(0.0);
        if (color.0.alpha() - new_alpha).abs() > 0.01 {
            color.0 = color.0.with_alpha(new_alpha);
        }
        if popup.elapsed >= popup.duration {
            commands.entity(entity).despawn();
        }
    }
}
