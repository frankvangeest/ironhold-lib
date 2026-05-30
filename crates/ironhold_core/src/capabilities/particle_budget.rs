use bevy::prelude::*;
use crate::schema::catalog::QualityLevel;

/// Global particle quality level. Scales `particle_count` at spawn time.
/// Persists across scene transitions — set via `Action::SetParticleQuality`.
#[derive(Resource)]
pub struct ParticleQuality {
    pub level: QualityLevel,
}

impl Default for ParticleQuality {
    fn default() -> Self {
        Self { level: QualityLevel::High }
    }
}

impl ParticleQuality {
    pub fn multiplier(&self) -> f32 {
        match self.level {
            QualityLevel::Minimal => 0.25,
            QualityLevel::Low     => 0.50,
            QualityLevel::Medium  => 0.75,
            QualityLevel::High    => 1.0,
        }
    }
}

/// Per-scene live particle cap. Reset to `scene.particle_budget` (or 2000) on each scene load.
/// Does not persist across scene transitions.
#[derive(Resource)]
pub struct ParticleBudget {
    pub max_count: u32,
}

impl Default for ParticleBudget {
    fn default() -> Self {
        Self { max_count: 2000 }
    }
}
