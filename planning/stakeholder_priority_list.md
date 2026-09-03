# Stakeholder Priority List

**Snapshot as of `db1ede0` (2026-09-03).** This is a one-time snapshot, not a living/re-generated
document — priorities will drift as the codebase changes, so treat this as a point-in-time read,
not an ongoing source of truth. Re-run manually later if a fresh read is wanted.

## What this is

Five of the project's specialized review-agent personas were each asked, independently and in
parallel, for their genuine top-5 wishlist — the things *they* would most want prioritized from
their own lens — ranked most→least important. Each was grounded in:

- their own `.claude/agent-memory/{agent}/` directory (recently purged of stale claims, so these
  reflect real current-state findings, not outdated ones)
- `planning/backlog.md` and `planning/claude_suggestions.md`
- their own judgment for anything real but not yet logged anywhere ("self-described")

Debug-detective was intentionally excluded from this round — its memory is mostly a record of
already-fixed bugs, not a forward-looking wishlist, so its "priorities" would have been thinner
than the other five lenses.

Every item below points to its source (a `backlog.md` item, a `claude_suggestions.md` entry, or
"self-described — not yet logged") so it can be chased down and, where warranted, promoted or
acted on independently of this snapshot.

---

## System-Architect — stability, maintainability, future-proofing

*Lens: what would most hurt this codebase's trajectory if ignored for another 6 months.*

### 1. Rapier cross-platform float divergence blocks the entire multiplayer roadmap, with no design work yet started
**Source:** `.claude/agent-memory/system-architect/determinism_networking.md`; `planning/backlog.md` ▸ Beta 0.5 (Deterministic Tick + Replay), Beta 0.6 (LAN Co-op), Beta 0.8/0.9 (Internet/Dedicated Server)

Four full milestones of planned work are gated on determinism, and the actual hard blocker —
Rapier3D's non-deterministic floating-point behavior across platforms — has no mitigation plan,
only a memory note recommending a SimClock chokepoint + run-mode enum that hasn't been scoped as a
feature. Every month of feature work built on top of the current physics/movement code adds more
surface area a future determinism retrofit will have to re-audit. This isn't a bug — it's a
foundation nobody has verified is buildable, sitting under a quarter of the roadmap.

### 2. `Action` enum has no `#[serde(deny_unknown_fields)]`, so typo'd RON action fields silently vanish
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; originally flagged by debug-detective during `dynamic_animation_control.md`'s review (2026-08-26)

The project's core architectural bet ("the schema is the designer's API surface") only holds if
the schema layer actually rejects malformed input. Without `deny_unknown_fields`, a misspelled
field parses cleanly and is silently dropped — by the engine, `ironhold_cli validate`, and
`ron_lint` alike. `PrefabComponents` already got this treatment as precedent; `Action` — the
highest-traffic authoring surface in the pipeline — still hasn't, and the surface only grows with
every new variant.

### 3. Scene-singleton config (`orbit_camera`, `flycam`, `player`) is architecturally misplaced on `PrefabDef`
**Source:** `planning/backlog.md` ▸ Icebox ▸ Engine/Runtime

Camera/input config is inherently scene-level (one active camera rig per scene), yet it lives on
`PrefabDef`, which is meant to be reusable and instantiable — a scene/prefab boundary violation
baked in early and never corrected. The cost compounds concretely: demonstrating one boolean flip
already required cloning two ~60-line prefabs into duplicates. Every new per-instance camera/input
feature pays this same prefab-forking tax, and it only grows the longer the layer stays wrong.

### 4. Test-suite trust is degrading: recurring unexplained flakiness plus no infrastructure to assert "a warning was/wasn't logged"
**Source:** `planning/backlog.md` ▸ Bugs (local-coop action-bar test flakiness) and ▸ Queued ▸ Engine/Runtime ("Test infrastructure for asserting 'no warning was logged'"); `planning/claude_suggestions.md` ▸ Testing

The entire code-change workflow treats "full test suite green" as the merge gate, but that signal
is already unreliable (4 documented occurrences of the same unreproducible intermittent failure)
and structurally incomplete — two real regression fixes couldn't get true regression tests because
Bevy's internal `log`-crate warnings aren't bridged into `tracing` in the test harness. Left alone,
either flakiness gets normalized until a real regression hides behind "that test is just flaky," or
a whole class of Bevy-internal-warning bugs keeps shipping without coverage.

### 5. `spawn_scene_v2` is pinned at Bevy's 16-param `SystemParam` ceiling with no systemic fix, only a workaround convention
**Source:** `.claude/agent-memory/system-architect/fragile_modules.md`

This is a hard Bevy-imposed compile-time wall, not a style preference — the system already sits
exactly at the boundary. The only mitigation on record is a workaround discipline ("bundle the next
resource into an existing `SystemParam` struct"), not a structural guarantee. Because scene-load is
the single most central system in the engine, this is the most likely place a future feature
silently breaks the build in a way that looks like "just add one resource" until it doesn't.

---

## Alignment-Reviewer — RON designer-reachability, no hardcoded behavior

*Lens: what a designer currently can't do through RON alone, or where behavior silently diverges by authoring path.*

### 1. RON-authorable collider friction / physics materials
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime

Every dynamic body hardcodes its friction coefficient at one Rust insertion site; static geometry
gets no `Friction` component at all. No schema field exposes any of this — a designer cannot author
a slippery-ice room or sticky-mud zone, not "hard to do" but structurally impossible without a Rust
change. It's also now driving engine-code churn: a v7 engine-internal constant was born directly
out of chasing this exact hardcoded value through a real bug fix — RON authoring should be
absorbing that need, not Rust constants.

### 2. `Action` enum has no `#[serde(deny_unknown_fields)]`
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; also `planning/claude_suggestions.md` ▸ Animation

A typo'd field on any of the ~40 `Action` variants parses cleanly and is silently dropped — zero
diagnostic anywhere, across every authoring surface (rules, state machine, behavior files,
dialogue) at once. This is the single largest systemic threat to "build entirely through RON
without recompiling": every other authoring mistake gets *some* signal; a mistyped `Action` field
gets none. *(Independently ranked #2 by system-architect and #1 by ux-gamedesigner-reviewer — the
clearest cross-stakeholder consensus item in this whole list.)*

### 3. Behavior-file `entry_actions` never receive `{target}` substitution
**Source:** `planning/backlog.md` ▸ Bugs (found `f9849ca`, system-architect plan review)

`entry_actions` firing on FSM state entry go through `rewrite_self` only; `rewrite_target` is
applied only to `on:` event-handler actions in the same file. The identical token `{target}` works
in one RON block and is a silent no-op literal string in a structurally adjacent block of the
*same* file, with no error either way — exactly the kind of authoring-path-dependent divergence
that undermines trust in the schema.

### 4. `ironhold_cli validate`'s `collect_actions` never walks dialogue `do_actions`
**Source:** `planning/backlog.md` ▸ Queued ▸ Designer Experience; `planning/claude_suggestions.md` (flagged independently ≥3 times)

`DialogueChoiceDef.do_actions`/`DialogueNodeDef.do_actions` are fully RON-authorable, identical in
shape and power to a rule's `do_actions` — but every action-based validate check is blind to them.
A designer authoring dialogue gets a strictly worse safety net than one authoring the same logic in
a rule file, for no principled reason.

### 5. Magic `tags` strings drive core spawn semantics instead of typed fields
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime

`collectable`, `player`, and `flycam` behavior are gated on free-form `tags: [...]` string matching
rather than typed, schema-validated fields. The single most foundational classification in the
engine — "is this entity the player" — rests on an untyped string convention with no RON-side
validation: a typo silently produces an entity invisible to every player-dependent system, with no
diagnostic anywhere.

---

## Game-World-Designer — missing/unstable features, world-building & player experience

*Lens: what kind of game world or moment-to-moment experience is currently impossible or fragile to build.*

### 1. Quest system — core loop (v1) + presentation layer (v2)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

There is no structural way to give a player a throughline today — no authored sequence of "go
here, do this, come back, get that," no state that persists a promise made by an NPC across a
scene. Every world designed so far has to fake progression entirely through raw `GameVariables` and
dialogue conditions, which works for a single gate but can't scale to a real questline with
branching states or a visible tracker. This is the biggest gap between "a scene with NPCs in it"
and "a world that remembers what you've done" — the whole premise of a designed world vs. a
diorama.

### 2. Item-gated interactable
**Source:** `planning/backlog.md` (Queued ▸ Gameplay & Environment); `.claude/agent-memory/game-world-designer/engine_limits_dialogue_audio_itemgate.md`

A concrete blocker sitting in front of an already-designed world (Greywatch's Seal Door needs
`old_key` to open) — today that can only be faked via a GameVariable set on *purchase* rather than
*possession*, so losing/trading the key wouldn't re-lock the door. Locked doors and "you need the
right item" gates are one of the oldest legible world-building tools there is: they communicate a
barrier spatially instead of through a dialogue wall, and make items feel like they matter. Small
schema surface, disproportionate narrative payoff.

### 3. Sound zones (zone-based ambient audio)
**Source:** `planning/backlog.md` (Queued ▸ Gameplay & Environment); `.claude/agent-memory/game-world-designer/engine_limits_dialogue_audio_itemgate.md`

Audio is doing none of the emotional-pacing work right now — no way to make a village feel safe and
the wilds feel tense purely through ambience, core to any world with a stated temperature gradient
across zones. Without a location-driven fade envelope, that gradient is visual-only, and a world
that looks tense but sounds identical everywhere reads as flat. Cheap to build (reuses existing
trigger zones + `PlayMusicLoop`/`StopMusic`) for how much atmospheric believability it buys back.

### 4. Day/night cycle
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

A world frozen at one lighting state forever can't sell the passage of time — one of the most
powerful low-cost tools for making a place feel inhabited rather than staged (lanterns lit at dusk,
NPCs behaving differently at night, danger scaling after dark). The event hooks (`time.dusk` etc.)
are what make it a *design* tool and not just a shader trick — they let rules/quests react to time
of day.

### 5. Loot system — roll + auto-loot (v1)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

Combat and exploration currently have no reward loop tied back to the world's own economy — kills
and searches don't produce anything the player can carry forward, so danger doesn't pay off and
scavenging isn't a reason to explore off the critical path. Also the direct unblock for Quest's
`Collect` objective type (item 1 above), so building it now pays down two wishlist items at once.

---

## UX-Gamedesigner-Reviewer — designer-authoring experience

*Lens: what silently goes wrong, is hard to discover, or wastes a non-programmer designer's iteration time.*

### 1. RON typos silently no-op with zero diagnostic anywhere
**Source:** `planning/claude_suggestions.md` ▸ Animation; `planning/backlog.md` ▸ Queued ▸ Engine/Runtime (same `Action` `deny_unknown_fields` item architect/alignment ranked #2/#1)

For a non-programmer, "I wrote what I thought was right and nothing happened" with no error message
is the single worst debugging position to be in — no stack trace, no red text, just silent RON that
doesn't do what it says. The corpse-pose bug this was found chasing was exactly this. Systemic
across every `Action` variant and every project, not confined to one feature.

### 2. CLI validate's reference-checking is broad but inconsistent, so "validate passed" doesn't mean "will work"
**Source:** `planning/backlog.md` ▸ Queued ▸ Designer Experience (dialogue actions, `spawn_point`/`item_key`/`currency_stat` references, UI trigger reachability, `join_prefab_keys` gamepad-index coverage — several items)

The whole pitch of `ironhold_cli validate` is "catch mistakes before you playtest" — but it checks
some reference classes and not close siblings, with no way for a designer to know which is which. A
clean `validate` run reasonably implies correctness; when it silently fails at runtime anyway, the
tool built to prevent exactly that becomes untrustworthy, and iteration reverts to trial-and-error
in the browser.

### 3. Missing demo projects for core authoring patterns
**Source:** `planning/backlog.md` ▸ Queued ▸ Designer Experience (`prefab_demo`, `ui_demo`, `audio_demo`, `scene_transitions_demo`, `parkour_demo`)

Every existing demo teaches a runtime *system* using prefabs/UI/audio incidentally — nothing
teaches the prefab schema itself (the first thing touched starting any project), or UI/audio
authoring as primary subjects. Without a canonical "one station per pattern" reference, a designer
has to reverse-engineer the right shape from whichever existing project happens to use a similar
feature — much slower and more error-prone than a dedicated teaching project.

### 4. Parse-breaking RON footguns that only `cli validate` catches, never predictable from the docs
**Source:** self-described, grounded in `.claude/agent-memory/ux-gamedesigner-reviewer/project_ron_enum_double_paren.md` and `project_quoted_string_vs_enum_house_style.md`

Enum variants wrapping a named struct need double parens (single-paren examples in docs fail to
parse), and quoted-string vs. bare-enum conventions are inconsistent across similar-looking fields.
A non-programmer has no intuition for this and can't pattern-match from other working examples,
because the pattern itself isn't consistent. A wrong paren count produces an opaque parser error,
not a designer-facing explanation.

### 5. Em-dash renders as a tofu box in any project's in-game text, with no lint or fix
**Source:** `planning/backlog.md` ▸ Bugs; `.claude/agent-memory/ux-gamedesigner-reviewer/project_em_dash_font_glyph_gap.md`

A small, recurring trap that's already bitten multiple unrelated projects — a designer typing a
normal em-dash gets a silent visual glitch discoverable only by looking at the rendered screenshot,
with nothing pointing at the character itself. Low effort to fix permanently (font glyph or a RON
lint flag), worth prioritizing precisely because it's cheap to close for good instead of being
rediscovered project after project.

---

## WASM-Perf-Reviewer — browser runtime performance & binary size

*Lens: frame-time impact, allocation/GC pressure, binary-size trajectory, first-load/first-frame stalls.*

**Binary size check-in:** `pkg/ironhold_web_bg.wasm` = 31 MB, ~64 MB below the 95 MB warn line —
size is a non-issue for ordinary feature work right now; no item below is size-motivated.

### 1. WASM terrain generation first-frame stall
**Source:** `planning/backlog.md` ▸ Performance

`AsyncComputeTaskPool` degrades to `block_on` on the WASM main thread, causing a 100–500 ms freeze
on first frame for large heightmaps — no worker-thread offload in a browser build. This is the
single biggest *first-impression* stall in the repo: it hits every session that loads a terrain
project, on the main thread, with no progressive fallback. Fixing it protects the moment a player
forms their opinion of the game's polish.

### 2. Per-frame collection allocations in always-on hot systems
**Source:** `planning/backlog.md` ▸ Performance

`message_interpreter_system` (event Vec rebuilt every frame, unconditionally — the core
Message→Action pipeline) and `player_movement_system`'s input HashMap allocate fresh collections
every tick regardless of whether anything happened. Unlike gated cases elsewhere on this list,
these run on literally every frame for every project, and WASM's allocator/GC pressure is
measurably worse than native for this pattern — steady-state tax paid by every scene.

### 3. Scene transition material cache
**Source:** `planning/backlog.md` ▸ Performance

`scene_loader` rebuilds *every* material in the asset catalog on each `LoadScene`, including ones
already built for the scene just left — an estimated 50–200 ms hitch per transition on large
projects, visible every time a player walks through a portal. The cost scales with catalog size, so
it gets worse as designers add more asset variety — exactly the wrong performance curve for a
data-driven engine meant to grow.

### 4. Paused/frozen animation clips are fully evaluated forever
**Source:** self-described — not yet logged (`.claude/agent-memory/wasm-perf-reviewer/project_animation_hot_path.md`)

bevy_animation 0.18 keeps sampling curves and writing bone `Transform`s for a `paused` clip every
frame — `freeze: true` stops event triggers, not per-frame work. Measured worst case ~0.2–0.5
ms/frame for 6 coexisting frozen corpses over a 300s despawn timer, and `par_iter_mut` is
effectively serial on WASM so there's no multi-core hiding this cost. The fix is already identified
and cheap (drop `AnimationGraphHandle` once frozen) — it just hasn't been promoted or implemented,
and it's a continuous drain that worsens as more corpse/prop-freeze patterns are authored.

### 5. Per-frame `format!` allocation before the change-detection guard (stat displays + target HUD)
**Source:** `planning/backlog.md` ▸ Performance (promoted 2026-09-03)

`stat_display.rs`'s update systems and `camera.rs`'s `target_hud_update_system` all compute
`format!(...)` unconditionally before the guard that gates the actual write — the allocation
happens even for hidden/unchanged widgets, up to 4x'd in split-screen and potentially ~200
allocations/frame with wave-spawned enemies. Smaller in magnitude than items 1–4, but a textbook
case of trivially-avoidable WASM allocator pressure.

---

## Cross-stakeholder signal

One item was independently placed in the **top 2** by three of the five stakeholders — the
strongest consensus signal in this snapshot:

- **`Action` enum needs `#[serde(deny_unknown_fields)]`** — ranked #2 by system-architect
  (schema-as-API-surface integrity), #2 by alignment-reviewer (systemic silent authoring-path
  divergence), and #1 by ux-gamedesigner-reviewer (worst-case designer debugging experience). It's
  already a logged `planning/backlog.md` ▸ Queued ▸ Engine/Runtime item — this snapshot is a strong
  signal to pull it forward rather than leave it queued behind newer work.
