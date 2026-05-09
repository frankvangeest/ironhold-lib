use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use indexmap::IndexMap;

#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct StatCatalog {
    pub schema_version: u32,
    pub stats: HashMap<String, StatDef>,
}

impl StatCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "Unsupported StatCatalog schema_version {} (expected 1)",
                self.schema_version
            ));
        }
        for (key, def) in &self.stats {
            if def.min > def.max {
                return Err(format!("Stat {:?}: min ({}) > max ({})", key, def.min, def.max));
            }
            if def.base < def.min || def.base > def.max {
                return Err(format!(
                    "Stat {:?}: base ({}) must be within [min ({}), max ({})]",
                    key, def.base, def.min, def.max
                ));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct StatDef {
    pub base: f32,
    #[serde(default)]
    pub min: f32,
    pub max: f32,
    /// Units per second added to the stat when regen is active. 0 = no regen.
    #[serde(default)]
    pub regen_rate: f32,
    /// Seconds after a decrease before regen resumes. 0 = immediate.
    #[serde(default)]
    pub regen_delay: f32,
    #[serde(default)]
    pub thresholds: Vec<StatThreshold>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StatThreshold {
    pub when: ThresholdCondition,
    /// Event name emitted as `GameEvent::Trigger` when the condition transitions false → true.
    pub emit: String,
}

#[derive(Deserialize, Debug, Clone)]
pub enum ThresholdCondition {
    BelowOrEqual(f32),
    AboveOrEqual(f32),
    /// Fraction of max (0.0–1.0). E.g. `BelowPercent(0.25)` fires when current < 25 % of max.
    BelowPercent(f32),
    /// Fraction of max (0.0–1.0). E.g. `AtOrAbovePercent(1.0)` fires when current = max.
    AtOrAbovePercent(f32),
}

impl ThresholdCondition {
    pub fn is_met(&self, current: f32, max: f32) -> bool {
        match self {
            ThresholdCondition::BelowOrEqual(val) => current <= *val,
            ThresholdCondition::AboveOrEqual(val) => current >= *val,
            ThresholdCondition::BelowPercent(pct) => {
                if max == 0.0 { false } else { current / max < *pct }
            }
            ThresholdCondition::AtOrAbovePercent(pct) => {
                if max == 0.0 { false } else { current / max >= *pct }
            }
        }
    }
}

#[derive(Clone)]
pub struct LiveStat {
    pub def: StatDef,
    pub current: f32,
    /// Seconds remaining before regen resumes. Counts down to 0.
    pub regen_cooldown: f32,
    /// Edge-detection state per threshold: `true` when the condition was met last check.
    /// Events fire only on false→true transitions to avoid re-firing every frame.
    pub prev_threshold_states: Vec<bool>,
}

impl LiveStat {
    pub fn new(def: StatDef) -> Self {
        let current = def.base;
        let max = def.max;
        let prev = def.thresholds.iter()
            .map(|t| t.when.is_met(current, max))
            .collect();
        Self {
            current,
            regen_cooldown: 0.0,
            prev_threshold_states: prev,
            def,
        }
    }

    /// Add `delta` to `current`, clamp to `[min, max]`. Negative delta resets the regen cooldown.
    pub fn apply_delta(&mut self, delta: f32) -> f32 {
        let clamped = (self.current + delta).clamp(self.def.min, self.def.max);
        if delta < 0.0 {
            self.regen_cooldown = self.def.regen_delay;
        }
        self.current = clamped;
        clamped
    }

    /// Set `current` to `value`, clamp to `[min, max]`. Decreasing the value resets the regen cooldown.
    pub fn set_value(&mut self, value: f32) -> f32 {
        let clamped = value.clamp(self.def.min, self.def.max);
        if clamped < self.current {
            self.regen_cooldown = self.def.regen_delay;
        }
        self.current = clamped;
        clamped
    }
}

/// Live stat state for the current project. Populated at project load time from `stats.ron`.
/// Stats persist across scene transitions (the resource is not cleared on scene load).
#[derive(Resource, Default)]
pub struct LoadedStats(pub HashMap<String, LiveStat>);

/// Stat shape declared on a prefab. Every spawned instance gets an independent `LiveStat`
/// in its `StatMap` component. `{self}` in `emit` strings is replaced with the entity's
/// spawn ID at spawn time.
#[derive(Deserialize, Debug, Clone)]
pub struct StatTemplateDef {
    /// Stat name within this entity — the key inside `StatMap` (e.g. `"health"`).
    pub key: String,
    pub base: f32,
    #[serde(default)]
    pub min: f32,
    pub max: f32,
    #[serde(default)]
    pub regen_rate: f32,
    #[serde(default)]
    pub regen_delay: f32,
    #[serde(default)]
    pub thresholds: Vec<StatThreshold>,
}

/// Per-entity stat store, inserted as a `Component` at spawn time.
/// Uses `IndexMap` for deterministic insertion-order iteration (replay correctness).
/// TODO: derive Reflect + register when bevy_ggrs rollback integration lands.
#[derive(Component, Default, Clone)]
pub struct StatMap(pub IndexMap<String, LiveStat>);

#[cfg(test)]
mod tests {
    use super::*;

    fn health_def() -> StatDef {
        StatDef {
            base: 100.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: vec![
                StatThreshold {
                    when: ThresholdCondition::BelowOrEqual(0.0),
                    emit: "stat.health.depleted".to_string(),
                },
                StatThreshold {
                    when: ThresholdCondition::BelowPercent(0.25),
                    emit: "stat.health.low".to_string(),
                },
                StatThreshold {
                    when: ThresholdCondition::AtOrAbovePercent(1.0),
                    emit: "stat.health.full".to_string(),
                },
            ],
        }
    }

    #[test]
    fn live_stat_initialises_to_base() {
        let stat = LiveStat::new(health_def());
        assert_eq!(stat.current, 100.0);
        assert_eq!(stat.regen_cooldown, 0.0);
        // At base=max, AtOrAbovePercent(1.0) is met; prev states should reflect this.
        assert!(!stat.prev_threshold_states[0]); // BelowOrEqual(0) — not met at 100
        assert!(!stat.prev_threshold_states[1]); // BelowPercent(0.25) — not met at 100
        assert!(stat.prev_threshold_states[2]);  // AtOrAbovePercent(1.0) — met at 100
    }

    #[test]
    fn apply_delta_clamps_to_min() {
        let mut stat = LiveStat::new(health_def());
        let result = stat.apply_delta(-200.0);
        assert_eq!(result, 0.0);
        assert_eq!(stat.current, 0.0);
    }

    #[test]
    fn apply_delta_clamps_to_max() {
        let mut stat = LiveStat::new(health_def());
        stat.current = 80.0;
        let result = stat.apply_delta(50.0);
        assert_eq!(result, 100.0);
        assert_eq!(stat.current, 100.0);
    }

    #[test]
    fn apply_negative_delta_resets_regen_cooldown() {
        let mut def = health_def();
        def.regen_delay = 3.0;
        let mut stat = LiveStat::new(def);
        stat.apply_delta(-10.0);
        assert_eq!(stat.regen_cooldown, 3.0);
    }

    #[test]
    fn apply_positive_delta_does_not_reset_regen_cooldown() {
        let mut def = health_def();
        def.regen_delay = 3.0;
        let mut stat = LiveStat::new(def);
        stat.regen_cooldown = 1.5; // already counting down
        stat.apply_delta(5.0);
        assert_eq!(stat.regen_cooldown, 1.5); // unchanged
    }

    #[test]
    fn set_value_clamps_and_resets_cooldown_on_decrease() {
        let mut def = health_def();
        def.regen_delay = 2.0;
        let mut stat = LiveStat::new(def);
        let result = stat.set_value(-50.0);
        assert_eq!(result, 0.0);
        assert_eq!(stat.regen_cooldown, 2.0);
    }

    #[test]
    fn threshold_condition_below_or_equal() {
        let c = ThresholdCondition::BelowOrEqual(0.0);
        assert!(c.is_met(0.0, 100.0));
        assert!(!c.is_met(0.001, 100.0));
        assert!(c.is_met(-1.0, 100.0));
    }

    #[test]
    fn threshold_condition_below_percent() {
        let c = ThresholdCondition::BelowPercent(0.25);
        assert!(c.is_met(24.0, 100.0)); // 24% < 25%
        assert!(!c.is_met(25.0, 100.0)); // 25% is not < 25%
        assert!(!c.is_met(50.0, 100.0));
    }

    #[test]
    fn threshold_condition_at_or_above_percent() {
        let c = ThresholdCondition::AtOrAbovePercent(1.0);
        assert!(c.is_met(100.0, 100.0));
        assert!(!c.is_met(99.9, 100.0));
    }

    #[test]
    fn stat_catalog_validate_rejects_bad_bounds() {
        let mut catalog = StatCatalog {
            schema_version: 1,
            stats: HashMap::new(),
        };
        catalog.stats.insert("hp".to_string(), StatDef {
            base: 50.0,
            min: 100.0, // min > max — invalid
            max: 50.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: vec![],
        });
        assert!(catalog.validate().is_err());
    }

    #[test]
    fn stat_catalog_validate_rejects_base_out_of_range() {
        let mut catalog = StatCatalog {
            schema_version: 1,
            stats: HashMap::new(),
        };
        catalog.stats.insert("hp".to_string(), StatDef {
            base: 200.0, // base > max — invalid
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: vec![],
        });
        assert!(catalog.validate().is_err());
    }
}
