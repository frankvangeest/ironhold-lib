#import bevy_ui::ui_vertex_output::UiVertexOutput

struct RadarUniforms {
    ratios_0: vec4<f32>,        // stat ratios 0-3  (0.0 = empty, 1.0 = full)
    ratios_1: vec4<f32>,        // stat ratios 4-7
    ratios_2: vec4<f32>,        // stat ratios 8-11
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
    var r: array<f32, 12>;
    r[0]  = material.ratios_0.x;
    r[1]  = material.ratios_0.y;
    r[2]  = material.ratios_0.z;
    r[3]  = material.ratios_0.w;
    r[4]  = material.ratios_1.x;
    r[5]  = material.ratios_1.y;
    r[6]  = material.ratios_1.z;
    r[7]  = material.ratios_1.w;
    r[8]  = material.ratios_2.x;
    r[9]  = material.ratios_2.y;
    r[10] = material.ratios_2.z;
    r[11] = material.ratios_2.w;
    return r[clamp(idx, 0, 11)];
}

// Returns the radial fraction [cos(π/N)..1.0] for a regular N-gon with
// circumradius 1 at angle theta.  Multiply by MAX_R to get the UV boundary.
// Value is 1.0 at vertex angles, cos(π/N) (the inradius fraction) at sector midpoints.
fn poly_boundary(theta: f32, n: f32) -> f32 {
    let sector = TAU / n;
    let th = ((theta % TAU) + TAU) % TAU;
    let i = floor(th / sector);
    let t = th - (i + 0.5) * sector;  // local angle from sector midpoint, in [-π/N, π/N]
    let half_sector = sector * 0.5;
    return cos(half_sector) / cos(t);
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
    let n_f = clamp(material.config.x, 3.0, 12.0);
    let n  = i32(n_f);
    let gs = i32(clamp(material.config.y, 1.0, 20.0));

    // outline_width is in UV fractions (e.g. 0.008 ≈ 2 px on a 250 px node).
    let half_ow = material.config.z * 0.5;

    // Clock angle: 0 at top, increasing clockwise.
    var theta = atan2(p.x, -p.y);
    if theta < 0.0 { theta += TAU; }

    // Outer polygon boundary at this angle (circumradius = MAX_R).
    // All background, grid, and clipping follow this polygon — no circles.
    let max_poly_r = MAX_R * poly_boundary(theta, n_f);

    // Discard outside outer polygon + outline band.
    if r > max_poly_r + half_ow {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Stat fill polygon: linear interpolation between adjacent stat vertices.
    let sector_f   = theta * n_f / TAU;
    let sector_idx = i32(floor(sector_f));
    let t_interp   = fract(sector_f);
    let r_a = get_ratio(sector_idx % n);
    let r_b = get_ratio((sector_idx + 1) % n);
    let stat_boundary = mix(r_a, r_b, t_interp) * MAX_R;

    // ── Background: inside outer polygon ──────────────────────────────────────
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if r < max_poly_r {
        color = material.background_color;
    }

    // ── Grid: concentric polygon rings (straight-edged, not circles) ──────────
    // Each ring at step s is a regular N-gon scaled by s/gs.
    let grid_w = 0.004;
    for (var s = 1; s < gs; s++) {
        let ring_r = max_poly_r * f32(s) / f32(gs);
        if abs(r - ring_r) < grid_w {
            color = material.grid_color;
        }
    }

    // ── Grid: radial spokes from center to each vertex ────────────────────────
    if r > 0.02 {
        let spoke_half_angle = 0.025 / max(r / MAX_R, 0.1);
        for (var k = 0; k < n; k++) {
            let spoke_angle = TAU * f32(k) / n_f;
            var diff = abs(theta - spoke_angle);
            diff = min(diff, TAU - diff);
            if diff < spoke_half_angle {
                color = material.grid_color;
            }
        }
    }

    // ── Fill: stat polygon interior, alpha-composited so grid shows through ───
    if r < stat_boundary - half_ow {
        color = alpha_over(color, material.fill_color);
    }

    // ── Outline: stat polygon boundary ────────────────────────────────────────
    if abs(r - stat_boundary) < half_ow {
        color = material.outline_color;
    }

    return color;
}
