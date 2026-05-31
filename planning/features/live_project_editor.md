# Feature: Ironhold Live Project Editor

_Status: Concept_
_Planned at: `dbcdf02` (2026-05-31)_

## What
A web application to manage the Rust Object Notation (RON) files that follow the fixed RON schema of the ironhold_core.

## Why
To make it easy for AI agents to manage game projects for the game engine runtime

## Approach
ToDo

## Tasks
- [ ] ToDo

## Open questions
- Do we need file watcher?
    - wiring up the watcher feature flag?
- Define a trait for data operations so the application does not care where the files live?
- make AssetCatalog and PrefabCatalog assets hot-reloadable?
- handling parse errors gracefully in the UI before applying changes?
- State preservation across reload? What do we want to reload (reset) and what not?
- How will it fit into this project, folder structure wise? A seperate crate?

## Acceptance criteria
- ToDo (Given X, when Y, then Z)
- Hostable as a web application in a container and on local pc. 
- Frontend components perfectly match backend data structures
- The editor prevents the user from entering data that is not allowed by the RON schema. Make RON file editing foolproof for users.
