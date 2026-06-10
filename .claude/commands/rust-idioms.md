# Idiomatic Rust & Memory Ownership Skill

Use this skill whenever the user asks to write, refactor, or debug Rust code involving structs, generics, async/await, or data passing.

## Implementation Steps
Before writing code, you MUST reason through these 3 layers:
1. **Ownership Map**: Who owns this data? Is it allocated on the stack or heap?
2. **Borrowing Check**: Are there overlapping mutable borrows? Do lifetimes match?
3. **Idiomatic Patterns**: Prefer traits and generic bounds over dynamic dispatch (`dyn`). Avoid unnecessary `.clone()` or `.unwrap()` operations.

## Reference Commands
If code fails compilation, run `cargo check` and review the compiler's primary error code explanation before modifying the file.
