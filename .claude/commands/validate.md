Run the ironhold CLI validator on a project.

Project: $ARGUMENTS

If no project name was provided, list all directories under `assets/projects/` and ask Frank which one to validate.

Run:
```
cargo run -p ironhold_cli -- validate assets/projects/<project_name>
```

Report the results:
- If all files are valid, confirm with the file count.
- If there are errors, list each one clearly with its file and line number.
- If there are cross-file reference errors, explain what is missing and where it is expected.

Optionally run with `--strict` to also catch defined keys that are never referenced anywhere:
```
cargo run -p ironhold_cli -- validate --strict assets/projects/<project_name>
```
