use clap::Subcommand;
use std::path::{Path, PathBuf};

use crate::output::OutputMode;

#[derive(Subcommand)]
pub enum InspectCommand {
    #[command(about = "List animations, meshes, materials, and root nodes of a GLB/GLTF file")]
    Glb {
        /// Path to the .glb or .gltf file
        path: PathBuf,
    },
}

pub fn run(cmd: InspectCommand, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InspectCommand::Glb { path } => inspect_glb(&path, mode),
    }
}

struct AnimationInfo {
    name: String,
    duration_secs: f32,
}

struct MeshInfo {
    name: String,
    vertex_count: usize,
    triangle_count: usize,
}

fn inspect_glb(path: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    let (document, buffers, _images) = gltf::import(path)?;

    let animations: Vec<AnimationInfo> = document
        .animations()
        .map(|animation| {
            let mut max_time = 0.0_f32;
            for channel in animation.channels() {
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(inputs) = reader.read_inputs() {
                    for t in inputs {
                        max_time = max_time.max(t);
                    }
                }
            }
            AnimationInfo {
                name: animation.name().unwrap_or("<unnamed>").to_string(),
                duration_secs: max_time,
            }
        })
        .collect();

    let meshes: Vec<MeshInfo> = document
        .meshes()
        .map(|mesh| {
            let mut vertex_count = 0usize;
            let mut triangle_count = 0usize;
            for primitive in mesh.primitives() {
                for (semantic, accessor) in primitive.attributes() {
                    if semantic == gltf::Semantic::Positions {
                        vertex_count += accessor.count();
                        break;
                    }
                }
                if let Some(indices) = primitive.indices() {
                    triangle_count += indices.count() / 3;
                } else {
                    // Non-indexed: each 3 vertices form a triangle
                    let prim_verts: usize = primitive
                        .attributes()
                        .find(|(s, _)| *s == gltf::Semantic::Positions)
                        .map(|(_, a)| a.count())
                        .unwrap_or(0);
                    triangle_count += prim_verts / 3;
                }
            }
            MeshInfo {
                name: mesh.name().unwrap_or("<unnamed>").to_string(),
                vertex_count,
                triangle_count,
            }
        })
        .collect();

    let materials: Vec<String> = document
        .materials()
        .map(|m| m.name().unwrap_or("<unnamed>").to_string())
        .collect();

    let root_nodes: Vec<String> = if let Some(scene) = document.default_scene() {
        scene
            .nodes()
            .filter_map(|n| n.name().map(|s| s.to_string()))
            .collect()
    } else {
        document
            .scenes()
            .flat_map(|s| s.nodes())
            .filter_map(|n| n.name().map(|s| s.to_string()))
            .collect()
    };

    if mode.json {
        print_json(path, &animations, &meshes, &materials, &root_nodes);
    } else {
        print_human(path, &animations, &meshes, &materials, &root_nodes);
    }

    Ok(())
}

fn print_human(
    path: &Path,
    animations: &[AnimationInfo],
    meshes: &[MeshInfo],
    materials: &[String],
    root_nodes: &[String],
) {
    println!("{}", path.display());
    println!();

    println!("  Animations ({})", animations.len());
    if animations.is_empty() {
        println!("    (none)");
    } else {
        for anim in animations {
            println!("    {:<28} {:.2} s", anim.name, anim.duration_secs);
        }
    }
    println!();

    println!("  Meshes ({})", meshes.len());
    if meshes.is_empty() {
        println!("    (none)");
    } else {
        for mesh in meshes {
            println!(
                "    {:<28} verts: {:>6}   tris: {:>6}",
                mesh.name, mesh.vertex_count, mesh.triangle_count
            );
        }
    }
    println!();

    println!("  Materials ({})", materials.len());
    if materials.is_empty() {
        println!("    (none)");
    } else {
        for mat in materials {
            println!("    {mat}");
        }
    }
    println!();

    println!("  Root nodes");
    if root_nodes.is_empty() {
        println!("    (none)");
    } else {
        for node in root_nodes {
            println!("    {node}");
        }
    }
}

fn print_json(
    path: &Path,
    animations: &[AnimationInfo],
    meshes: &[MeshInfo],
    materials: &[String],
    root_nodes: &[String],
) {
    let val = serde_json::json!({
        "path": path.display().to_string(),
        "animations": animations.iter().map(|a| serde_json::json!({
            "name": a.name,
            "duration_secs": a.duration_secs,
        })).collect::<Vec<_>>(),
        "meshes": meshes.iter().map(|m| serde_json::json!({
            "name": m.name,
            "vertex_count": m.vertex_count,
            "triangle_count": m.triangle_count,
        })).collect::<Vec<_>>(),
        "materials": materials,
        "root_nodes": root_nodes,
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap());
}
