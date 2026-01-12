
# Contributing

## Ground rules
- Keep behavior data-driven (RON) wherever possible.
- New capabilities must register:
  - events they emit
  - actions they execute
  - validation rules

## Pull request checklist
- Documentation updated
- Example project updated or new example added
- Tests added (unit/integration as appropriate)
- Schema compatibility considered (version bump + migration notes if needed)

## Coding conventions
- Keep `use` statements multi-line for clarity (project style).
- Prefer small modules over giant `lib.rs`.
