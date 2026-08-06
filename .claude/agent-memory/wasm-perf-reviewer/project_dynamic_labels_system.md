---
name: project-dynamic-labels-system
description: update_dynamic_labels_system builds a String per label every frame; render write is guarded but the alloc is not
metadata:
  type: project
---

`update_dynamic_labels_system` in `crates/ironhold_core/src/lib.rs` (~line 337) runs every Update with no gate.

**Scope correction (verified 2026-08-06):** the query is `Query<(&mut Text, &DynamicLabel)>`, and `DynamicLabel` is only inserted when a scene `Label(...)` declares `bind:` (scene_loader.rs ~1461). Static-text scene labels are therefore NOT touched by this system and cost nothing per frame — adding plain hint labels to a scene is free, do not flag it.

**Why:** Drives data-bound HUD labels (incl. new `target_display`/`target_name`/`target_id`). For each `(Text, DynamicLabel)` it computes `new_text` via `fmt.replace("{}", value)` or `value.to_string()`, then writes `*text = Text::new(...)` only if `text.0 != new_text`.

**How to apply:**
- The change-detection guard (`if text.0 != new_text`) is CORRECT and prevents per-frame glyph/atlas/text-layout churn — the expensive part. This is what the user asked to confirm; it holds.
- BUT the `new_text` String is allocated every frame for every dynamic label regardless of change, since the format/to_string runs before the comparison. This is a small per-frame heap alloc proportional to label count, not a stall. Label counts are tiny (single-digit), so not worth fixing unless label count grows large. If it ever matters: compare against a cached `Local<String>` or format into a reused buffer before allocating a Text.
- Not a plausible audio-stutter cause at current label counts.
