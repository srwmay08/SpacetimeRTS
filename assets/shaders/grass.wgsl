// ============================================================================
// File: assets/shaders/grass.wgsl
// ============================================================================
// Procedural wind-swaying vertex displacement shader for low-poly grass in SpacetimeRTS.
// Extends Bevy's StandardMaterial PBR pipeline while displacing blade tips dynamically.
// ============================================================================

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::{Vertex, VertexOutput},
}

struct GrassExtension {
    wind_params: vec4<f32>, // x: speed, y: strength, z: frequency, w: time
};

@group(2) @binding(100)
var<uniform> grass: GrassExtension;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;

    var model = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var world_pos = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(vertex_in.position, 1.0));

    // Wind wave sway:
    // vertex_in.uv.y encodes the height factor: 0.0 at blade root (firmly anchored), 1.0 at blade tip
    let height_factor = vertex_in.uv.y;
    let time = grass.wind_params.w;
    let speed = grass.wind_params.x;
    let strength = grass.wind_params.y;
    let freq = grass.wind_params.z;

    // Dual-wave diagonal wind gust sweeping across the landscape
    let wave1 = sin(time * speed + world_pos.x * freq + world_pos.z * (freq * 0.7));
    let wave2 = cos(time * (speed * 1.35) + world_pos.x * (freq * 0.45) - world_pos.z * (freq * 0.85));
    let gust = (wave1 * 0.7 + wave2 * 0.3);

    // Tip displacement: leans in wind direction and dips down slightly
    let wind_disp = vec3<f32>(
        gust * strength * height_factor,
        -abs(gust) * strength * 0.25 * height_factor,
        (wave1 * 0.5 - wave2 * 0.5) * strength * height_factor
    );

    world_pos = vec4<f32>(world_pos.xyz + wind_disp, 1.0);

    out.world_position = world_pos;
    out.position = position_world_to_clip(world_pos.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex_in.normal, vertex_in.instance_index);
    out.uv = vertex_in.uv;
    out.color = vertex_in.color;

    return out;
}
