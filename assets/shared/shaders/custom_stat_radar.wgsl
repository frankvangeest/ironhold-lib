#import bevy_ui::ui_vertex_output::UiVertexOutput

struct RadarUniforms {
    ratios_0: vec4<f32>,        // stat ratios 0-3  (0.0 = empty, 1.0 = full)
    ratios_1: vec4<f32>,        // stat ratios 4-7
    config: vec4<f32>,          // x=stat_count, y=grid_steps, z=outline_width (UV), w=unused
    fill_color: vec4<f32>,
    outline_color: vec4<f32>,
    grid_color: vec4<f32>,
    background_color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> material: RadarUniforms;

const PI: f32   = 3.14159265358979;
const TAU: f32  = 6.28318530717959;
const MAX_R: f32 = 0.45;

fn get_ratio(idx: i32) -> f32 {
    var r: array<f32, 8>;
    r[0] = material.ratios_0.x;
    r[1] = material.ratios_0.y;
    r[2] = material.ratios_0.z;
    r[3] = material.ratios_0.w;
    r[4] = material.ratios_1.x;
    r[5] = material.ratios_1.y;
    r[6] = material.ratios_1.z;
    r[7] = material.ratios_1.w;
    return r[clamp(idx, 0, 7)];
}

// Alpha-composite src over dst.
fn alpha_over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let a = src.a;
    return vec4<f32>(src.rgb * a + dst.rgb * (1.0 - a),
                     1.0 - (1.0 - a) * (1.0 - dst.a));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p  = in.uv - vec2<f32>(0.5, 0.5);
    let r  = length(p);
    let n  = i32(clamp(material.config.x, 1.0, 8.0));
    let gs = i32(clamp(material.config.y, 1.0, 8.0));

    // outline_width is in UV fractions (e.g. 0.008 ≈ 2 px on a 250 px node).
    let half_ow = material.config.z * 0.5;

    // Discard pixels outside the radar circle + outline ring.
    if r > MAX_R + half_ow {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Clock angle: 0 at top, increasing clockwise.
    // UV y grows downward, so top of node = p.y < 0.
    // atan2(px, -py) gives 0 at top, π/2 at right.
    var theta = atan2(p.x, -p.y);
    if theta < 0.0 { theta += TAU; }

    // Which sector (pie slice) are we in?
    let sector_f   = theta * f32(n) / TAU;
    let sector_idx = i32(floor(sector_f));
    let t          = fract(sector_f);

    let r_a = get_ratio(sector_idx % n);
    let r_b = get_ratio((sector_idx + 1) % n);

    // Polygon boundary at this angle (angle-linear interpolation between vertices).
    // Slightly concave between vertices — acceptable for radar chart style.
    let boundary = mix(r_a, r_b, t) * MAX_R;

    // ── Base layer: background inside max circle ───────────────────────────────
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if r < MAX_R {
        color = material.background_color;
    }

    // ── Grid: concentric N-gons (approximated as circles) ─────────────────────
    let grid_w = 0.004;
    for (var s = 1; s < gs; s++) {
        let grid_r = MAX_R * f32(s) / f32(gs);
        if abs(r - grid_r) < grid_w {
            color = material.grid_color;
        }
    }

    // ── Grid: radial spokes from center to each vertex ────────────────────────
    if r > 0.02 {
        let spoke_half_angle = 0.025 / max(r / MAX_R, 0.1); // wider near center
        for (var k = 0; k < n; k++) {
            let spoke_angle = TAU * f32(k) / f32(n);
            var diff = abs(theta - spoke_angle);
            diff = min(diff, TAU - diff);
            if diff < spoke_half_angle {
                color = material.grid_color;
            }
        }
    }

    // ── Fill: polygon interior, alpha-composited so grid shows through ─────────
    if r < boundary - half_ow {
        color = alpha_over(color, material.fill_color);
    }

    // ── Outline: polygon boundary ─────────────────────────────────────────────
    if abs(r - boundary) < half_ow {
        color = material.outline_color;
    }

    return color;
}
