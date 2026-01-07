# Ironhold-lib
A cross-platform Web First Game Engine library built on top of Bevy. It gives a uniform interface that works practically the same in WASM/Web as is does on Native builds. The game is defined by loading .ron files, which are similar to .json files, but allow comments. 
The library comes by default with many systems and features enabled. This is a trade-off that more easily allows people to create game with less coding setup.
You only need to define your data in the ron files.
The downside is, that the library can get bloated over time as more features get added. 
To build a more minimal version of the library, you can disable features in the cargo.toml file.

## Status
Currently working on:
- Defining the uniform cross-platform API setup
- Loading and rendering .ron scenes files
- Loading and rendering .glb files


## WASM / Web
your-project-folder
    assets
        scene.ron
    ironhold-lib.wasm
    ironhold-lib.js
    index.html

# Native
your-project-folder
    assets
        scene.ron
    src
        main.rs
    cargo.toml
    


