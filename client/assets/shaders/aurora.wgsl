// ============================================================================
// File: client/assets/shaders/aurora.wgsl
// ============================================================================
// Real-time planetary aurora curtain shader for SpacetimeRTS.
// Incorporates Nimitz triangle noise curtain folds, spectral emerald/cyan/violet
// color gradient synthesis, and dynamic altitude striations.
// ============================================================================

#import bevy_pbr::forward_io::VertexOutput

const TAU: f32 = 6.283185307179586;

struct AuroraUniforms {
    time: f32,
    intensity: f32,
    speed: f32,
    night_factor: f32,
    uv_scale: vec2<f32>,
    _padding: vec2<f32>,
    color_tint: vec4<f32>,
};

@group(2) @binding(0) var<uniform> uniforms: AuroraUniforms;

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
    let t = uniforms.time * uniforms.speed;
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = vec2<f32>(0.5, 0.5);
#endif

    // Scale UV coordinates for multi-fold celestial drapery across the ribbon arc
    let uv_scaled = vec2<f32>(uv.x * uniforms.uv_scale.x, uv.y * uniforms.uv_scale.y);

    // 1. Generate sharp, shifting curtain folds with Nimitz triangle noise
    let curtain = auroraCurtainNoise(uv_scaled, t);

    // 2. Dynamic curtain profile: normalize curtain folds and apply vertical ribbon envelope
    let v_fade = smoothstep(0.0, 0.12, uv.y) * smoothstep(1.0, 0.75, uv.y);
    var v = clamp((curtain / 0.55) * v_fade, 0.0, 1.0);

    // 3. Color Gradient formulation:
    // x varies across the ribbon width (from 1.0 down to 0.25)
    // y peaks at 1.0 in the curtain vertical core
    var color = vec3<f32>(0.0);
    let x = 1.0 - uv.x * 0.75;
    let y = 1.0 - abs(uv.y * 2.0 - 1.0);
    // Emerald/Cyan/Violet spectral synthesis: x*0.5 (red), y (green), x (blue)
    color += vec3<f32>(x * 0.5, y, x) * v;

    // 4. Modulation by vertex color, color tint, atmospheric weather intensity, and diurnal night factor
#ifdef VERTEX_COLORS
    color = color * in.color.rgb;
#endif
    color = color * uniforms.color_tint.rgb;

    let total_intensity = uniforms.intensity * uniforms.night_factor;
    let final_rgb = color * total_intensity;
    let final_alpha = clamp(v * total_intensity, 0.0, 1.0);

    return vec4<f32>(final_rgb, final_alpha);
}
