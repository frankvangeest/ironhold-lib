Check Rust documentation quality for ironhold_core.

Run:
```
cargo doc --no-deps -p ironhold_core 2>&1
```

Parse the output and report:

- **Warnings** — Group by file. Common types:
  - `missing_docs` — public item has no doc comment
  - Broken intra-doc links (`[SomeType]` that doesn't resolve)
  - Unused `#[doc]` attributes

- **Errors** — Any `error:` lines that prevent doc generation.

If there are no warnings or errors, confirm the docs are clean.

Do not open the generated HTML docs — just surface the compiler output.

Optionally suggest fixes for any missing docs warnings by reading the relevant source file and drafting one-line doc comments for the undocumented items.
