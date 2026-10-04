// ============================================================================
// File: client/assets/shaders/aurora.wgsl
// ============================================================================
// Real-time planetary aurora curtain shader for SpacetimeRTS.
// Faithfully incorporates domain-warped dynamic ribbon thickness pulsing,
// spectral emerald/cyan/violet color gradient synthesis, and micro-particle
// high-energy electron collision shimmer.
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

// Procedural 2D Hash function
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Procedural 2D Value Noise with quintic smoothstep interpolation
fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Multi-octave fractal noise for organic magnetic drapery folds
fn aurora_noise(p: vec2<f32>) -> f32 {
    let n1 = noise2d(p);
    let n2 = noise2d(p * 2.05 + vec2<f32>(3.1, 1.7));
    return n1 * 0.7 + n2 * 0.3;
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

    // 1. Slow upward flowing disturbance layer
    let o = aurora_noise(uv_scaled * 0.25 + vec2<f32>(0.0, t * 0.025));

    // 2. Domain-warped undulating magnetic wave (displacements between -1.0 and 1.0)
    let d = aurora_noise(uv_scaled * 0.25 - vec2<f32>(0.0, t * 0.02 + o * 0.02)) * 2.0 - 1.0;

    // 3. Dynamic curtain profile: folds symmetrically around vertical center with breathing exponent
    let v_dist = uv.y + d * 0.1;
    var v = 1.0 - abs(v_dist * 2.0 - 1.0);
    v = clamp(v, 0.0, 1.0);
    let breathe_exponent = 2.0 + sin((t * 0.2 + d * 0.25) * TAU) * 0.5;
    v = pow(v, max(breathe_exponent, 0.1));

    // 4. Color Gradient formulation:
    // x varies across the ribbon width (from 1.0 down to 0.25)
    // y peaks at 1.0 in the curtain vertical core
    var color = vec3<f32>(0.0);
    let x = 1.0 - uv.x * 0.75;
    let y = 1.0 - abs(uv.y * 2.0 - 1.0);
    // Emerald/Cyan/Violet spectral synthesis: x*0.5 (red), y (green), x (blue)
    color += vec3<f32>(x * 0.5, y, x) * v;

    // 5. High-energy ionized particle micro-sparkle twinkle
    let seed = in.position.xy;
    var r: vec2<f32>;
    r.x = fract(sin(seed.x * 12.9898 + seed.y * 78.2330) * 43758.5453);
    r.y = fract(sin(seed.x * 53.7842 + seed.y * 47.5134) * 43758.5453);

    let s = mix(r.x, (sin((t * 2.5 + 60.0) * r.y) * 0.5 + 0.5) * ((r.y * r.y) * (r.y * r.y)), 0.04);
    color += vec3<f32>(pow(s, 70.0) * (1.0 - v));

    // 6. Modulation by vertex color, color tint, atmospheric weather intensity, and diurnal night factor
    #ifdef VERTEX_COLORS
    color = color * in.color.rgb;
    #endif
    color = color * uniforms.color_tint.rgb;

    let total_intensity = uniforms.intensity * uniforms.night_factor;
    let final_rgb = color * total_intensity;
    let final_alpha = clamp(v * total_intensity, 0.0, 1.0);

    return vec4<f32>(final_rgb, final_alpha);
}
