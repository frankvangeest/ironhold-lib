---
name: substitution-token-surfaces
description: Substitution tokens ({self}/{target}/{new_id}, planned {attacker}) have NO single docs table; doc locations, no validate "unresolvable token" check, exact-match means tokens are never capture wildcards
metadata:
  type: project
---

There is no single substitution-token table in docs. Locations (as of 2026-10-02):
- `{target}`: docs/20_data_formats.md ~1202 (action bar section); co-op override caveat ~1191-1200
- `{self}`: docs/30_runtime_events_and_logic.md ~498-515
- `{new_id}`: docs/30 ~517-536
- Action table rows (docs/20 ~3828-3848) mention only `{self}`.
- There is no `{player}` token (plans sometimes write `entity.hit:{player}:{self}`; never let that into docs).

`ironhold validate` has NO check for a token used where it cannot resolve, and none for unknown/misspelled
`{word}` tokens. The `{self}` code in validate.rs is skip-logic only. A typo'd token silently becomes a literal id.

FSM event matching is exact string equality, so a token in an `on:` pattern is never a capture wildcard.
Designers will try `on: "entity.hit:{self}:{attacker}"`. Multi-part events can only be bound per known id.

**Why:** the attacker_identity_on_hit_events plan review (2026-10-02) found the docs task pointing at a
nonexistent "token table" and only an EmitEvent-scoped runtime warning as the misuse safety net.
**How to apply:** for any new token or multi-part event plan, require (a) a docs/30 section beside `{new_id}`,
(b) validate coverage for unresolvable contexts plus unknown tokens, (c) an explicit "how to bind 'by anyone'"
answer. Related: [[self-substitution-pattern]], [[entity-event-doc-surfaces]], [[validate-coverage-gaps]].
