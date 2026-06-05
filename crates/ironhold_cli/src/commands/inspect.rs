use clap::Subcommand;
use image::GenericImageView;
use std::path::{Path, PathBuf};

use crate::output::OutputMode;

#[derive(Subcommand)]
pub enum InspectCommand {
    #[command(
        about = "List animations, meshes, materials, and root nodes of a GLB/GLTF file",
        after_help = "Examples:\n  ironhold inspect glb assets/shared/models/creatures/orc-enemy.glb\n  ironhold --json inspect glb assets/shared/models/creatures/dragon.glb"
    )]
    Glb {
        /// Path to the .glb or .gltf file
        path: PathBuf,
    },
    #[command(
        about = "Report dimensions, format, channels, and file size of an image",
        after_help = "Supported formats: PNG, JPEG, WebP, GIF, BMP, TIFF. AVIF is not supported.\n\nExamples:\n  ironhold inspect texture assets/shared/textures/decals/circle_filled.png\n  ironhold --json inspect texture assets/shared/textures/Cobblestone_001_SD/Cobblestone_001_COLOR.jpg"
    )]
    Texture {
        /// Path to the image file (png, jpg, webp, gif, bmp, tiff)
        path: PathBuf,
    },
    #[command(
        about = "Report format, duration, sample rate, and channel count of an audio file",
        after_help = "Supported formats: WAV, MP3. Duration is useful for setting delay_secs in EmitEventAfterDelay.\n\nExamples:\n  ironhold inspect audio assets/shared/audio/boulder/boulder-push1.wav\n  ironhold --json inspect audio assets/shared/audio/bg-music-balance.mp3"
    )]
    Audio {
        /// Path to the audio file (wav, mp3)
        path: PathBuf,
    },
}

pub fn run(cmd: InspectCommand, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        InspectCommand::Glb { path } => inspect_glb(&path, mode),
        InspectCommand::Texture { path } => inspect_texture(&path, mode),
        InspectCommand::Audio { path } => inspect_audio(&path, mode),
    }
}

// ── GLB ──────────────────────────────────────────────────────────────────────

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
        glb_print_json(path, &animations, &meshes, &materials, &root_nodes);
    } else {
        glb_print_human(path, &animations, &meshes, &materials, &root_nodes);
    }

    Ok(())
}

fn glb_print_human(
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

fn glb_print_json(
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

// ── Texture ───────────────────────────────────────────────────────────────────

fn inspect_texture(path: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    let file_size = std::fs::metadata(path)?.len();

    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let format = reader.format();
    let img = reader.decode()?;

    let (width, height) = img.dimensions();
    let channels  = color_type_channels(img.color());
    let bit_depth = color_type_bit_depth(img.color());
    let format_str = format.map(image_format_name).unwrap_or("Unknown");

    if mode.json {
        let val = serde_json::json!({
            "path": path.display().to_string(),
            "width": width,
            "height": height,
            "format": format_str,
            "channels": channels,
            "bit_depth": bit_depth,
            "file_size_bytes": file_size,
        });
        println!("{}", serde_json::to_string_pretty(&val).unwrap());
    } else {
        println!("{}", path.display());
        println!();
        println!("  Dimensions   {} × {}", width, height);
        println!("  Format       {}", format_str);
        println!("  Channels     {}", channels);
        println!("  Bit depth    {}", bit_depth);
        println!("  File size    {}", human_file_size(file_size));
    }

    Ok(())
}

fn image_format_name(fmt: image::ImageFormat) -> &'static str {
    match fmt {
        image::ImageFormat::Png => "PNG",
        image::ImageFormat::Jpeg => "JPEG",
        image::ImageFormat::WebP => "WebP",
        image::ImageFormat::Avif => "AVIF",
        image::ImageFormat::Gif => "GIF",
        image::ImageFormat::Bmp => "BMP",
        image::ImageFormat::Tiff => "TIFF",
        _ => "Unknown",
    }
}

fn color_type_channels(ct: image::ColorType) -> &'static str {
    match ct {
        image::ColorType::L8   | image::ColorType::L16   => "Grayscale",
        image::ColorType::La8  | image::ColorType::La16  => "Grayscale+Alpha",
        image::ColorType::Rgb8 | image::ColorType::Rgb16 | image::ColorType::Rgb32F  => "RGB",
        image::ColorType::Rgba8| image::ColorType::Rgba16| image::ColorType::Rgba32F => "RGBA",
        _ => "Unknown",
    }
}

fn color_type_bit_depth(ct: image::ColorType) -> &'static str {
    match ct {
        image::ColorType::L8   | image::ColorType::La8
        | image::ColorType::Rgb8  | image::ColorType::Rgba8  => "8 bpc",
        image::ColorType::L16  | image::ColorType::La16
        | image::ColorType::Rgb16 | image::ColorType::Rgba16 => "16 bpc",
        image::ColorType::Rgb32F | image::ColorType::Rgba32F => "32 bpc (float)",
        _ => "Unknown",
    }
}

fn human_file_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{} KB", (bytes + 512) / 1_024)
    }
}

// ── Audio ─────────────────────────────────────────────────────────────────────

fn inspect_audio(path: &Path, mode: &OutputMode) -> Result<(), Box<dyn std::error::Error>> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file_size = std::fs::metadata(path)?.len();
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("no audio track found")?;

    let params = &track.codec_params;
    let sample_rate = params.sample_rate.unwrap_or(0);
    let channels = params.channels.map(|c| c.count()).unwrap_or(0);

    // Duration: prefer n_frames / sample_rate; fall back to time_base * n_frames
    let duration_secs = if let (Some(frames), Some(rate)) = (params.n_frames, params.sample_rate) {
        frames as f64 / rate as f64
    } else if let (Some(frames), Some(tb)) = (params.n_frames, params.time_base) {
        frames as f64 * tb.numer as f64 / tb.denom as f64
    } else {
        0.0
    };

    let format_str = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| "Unknown".to_string());

    let channels_str = match channels {
        1 => "Mono",
        2 => "Stereo",
        _ => "Multi",
    };

    if mode.json {
        let val = serde_json::json!({
            "path": path.display().to_string(),
            "format": format_str,
            "duration_secs": duration_secs,
            "sample_rate_hz": sample_rate,
            "channels": channels,
            "file_size_bytes": file_size,
        });
        println!("{}", serde_json::to_string_pretty(&val).unwrap());
    } else {
        println!("{}", path.display());
        println!();
        println!("  Format       {}", format_str);
        println!("  Duration     {:.2} s", duration_secs);
        println!("  Sample rate  {} Hz", sample_rate);
        println!("  Channels     {}", channels_str);
        println!("  File size    {}", human_file_size(file_size));
    }

    Ok(())
}
