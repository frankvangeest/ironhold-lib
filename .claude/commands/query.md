Run an ironhold CLI query against a project.

Usage: /query <type> <project>
  Examples: /query prefabs terrain_demo
            /query actions 3rd_person_game_demo
            /query scenes foliage_demo

Arguments: $ARGUMENTS

Parse the arguments to extract the query type and project name. If either is missing, list the available query types and all directories under `assets/projects/`, then ask Frank for the missing values.

Available query types:
- `prefabs`  — list prefabs (kind, model, tags, behavior)
- `effects`  — list particle effects (count, layers, flags)
- `scenes`   — list scenes (entities, ui, player, overlay)
- `rules`    — list rules.ron and/or state_machine.ron
- `actions`  — list all action types used across logic files
- `events`   — list all event triggers used across logic files

Run:
```
cargo run -p ironhold_cli -- query <type> assets/projects/<project>
```

Optional flags:
- `--keys-only` — one key per line (pipe-friendly)
- `--filter field=value` — filter results (e.g. `--filter additive=true` for effects)
- `--json` — machine-readable output

Report results clearly. If the query returns no results, confirm the project was found and the query type is valid.
