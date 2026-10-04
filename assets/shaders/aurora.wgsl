// ============================================================================
// File: assets/shaders/aurora.wgsl
// ============================================================================
// Real-time planetary aurora sky dome raymarching shader for SpacetimeRTS.
// Incorporates Nimitz triangle noise curtain folds, spectral emerald/cyan/violet
// color gradient synthesis, and dynamic altitude striations across a sky dome.
// ============================================================================

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_view_bindings::globals

const TAU: f32 = 6.283185307179586;

struct SkyUniforms {
    night_factor: f32,
    weather_intensity: f32,
    speed: f32,
    brightness: f32,
};

@group(2) @binding(0) var<uniform> uniforms: SkyUniforms;

// --- Nimitz Triangle Noise Curtains ---

fn mm2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(vec2<f32>(c, s), vec2<f32>(-s, c));
}

fn tri(x: f32) -> f32 {
    return clamp(abs(fract(x) - 0.5), 0.01, 0.49);
}

fn tri2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(tri(p.x) + tri(p.y), tri(p.y + tri(p.x)));
}

// Generates sharp, shifting curtain folds
fn auroraCurtainNoise(p_in: vec2<f32>, time: f32) -> f32 {
    let m2 = mat2x2<f32>(vec2<f32>(0.95534, 0.29552), vec2<f32>(-0.29552, 0.95534));
    var z = 1.8;
    var z2 = 2.5;
    var rz = 0.0;
    
    // Anisotropic stretch along vertical axis to emulate altitude striations
    var p = p_in * vec2<f32>(1.0, 0.25);
    p = mm2(p.x * 0.06) * p;
    var bp = p;
    
    // 4 octaves is plenty for ribbon geometry (saves ALU over the original 5)
    for (var i = 0u; i < 4u; i = i + 1u) {
        var dg = tri2(bp * 1.85) * 0.75;
        dg = mm2(time * 0.05) * dg;
        p = p - (dg / z2);

        bp = bp * 1.3;
        z2 = z2 * 0.45;
        z = z * 0.42;
        p = p * (1.21 + (rz - 1.0) * 0.02);
        
        rz = rz + tri(p.x + tri(p.y)) * z;
        p = -(m2 * p);
    }
    
    return clamp(1.0 / pow(rz * 29.0, 1.3), 0.0, 0.55);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. Inactivity & Early-out Horizon Clipping (Zero ALU overhead when aurora inactive or below horizon)
    let total_activity = uniforms.weather_intensity * uniforms.night_factor;
    if (uniforms.weather_intensity * uniforms.night_factor <= 0.01) {
        discard;
    }

    // Sky dome direction vector (normalized view direction)
    let rd = normalize(in.world_position.xyz - view.world_position);
    if (rd.y <= 0.015) {
        discard;
    }

    let t = globals.time * uniforms.speed;
    let horizon_fade = smoothstep(0.015, 0.12, rd.y);

    // 2. Procedural Sky Dome Raymarching through Ionospheric Aurora Shells
    var accum_rgb = vec3<f32>(0.0);
    var accum_alpha = 0.0;

    // Raymarch 12 altitude slices through the ionospheric layer
    let steps = 12u;
    for (var i = 0u; i < steps; i = i + 1u) {
        let step_ratio = f32(i) / f32(steps);
        // Altitude layer from h = 2.0 to 4.5
        let h = 2.0 + step_ratio * 2.5;
        let pt = (rd.xz / rd.y) * h;

        let curtain = auroraCurtainNoise(pt * 0.08, t);

        // Vertical envelope fade across altitude slices
        let v_env = sin(step_ratio * 3.14159265);
        let density = (curtain / 0.55) * v_env;

        // Spectral Emerald/Cyan/Violet synthesis: x*0.5 (red), y (green), x (blue)
        let x = clamp(1.0 - (pt.x * 0.02 + step_ratio * 0.4), 0.25, 1.0);
        let y = clamp(1.0 - abs(step_ratio * 2.0 - 1.0), 0.1, 1.0);
        let spectral_color = vec3<f32>(x * 0.5, y, x);

        let slice_alpha = density * 0.18;
        accum_rgb += spectral_color * slice_alpha * (1.0 - accum_alpha);
        accum_alpha += slice_alpha * (1.0 - accum_alpha);

        if (accum_alpha >= 0.95) {
            break;
        }
    }

    let final_intensity = total_activity * uniforms.brightness * horizon_fade;
    let final_rgb = accum_rgb * final_intensity;
    let final_a = clamp(accum_alpha * final_intensity, 0.0, 1.0);

    return vec4<f32>(final_rgb, final_a);
}
