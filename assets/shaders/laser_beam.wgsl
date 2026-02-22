#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::globals,
    view_transformations::position_world_to_clip,
}

struct Vertex {
    @builtin(instance_index) instance_index : u32,
    @location(0) position : vec3<f32>,
    @location(1) normal   : vec3<f32>,
    @location(2) uv       : vec2<f32>,
};

struct VOut {
    @builtin(position) clip_pos : vec4<f32>,
    @location(0) uv             : vec2<f32>,
    @location(1) radial         : f32,
    @location(2) height         : f32,
};

@group(2) @binding(0) var<uniform> fraction : f32;

@vertex
fn vertex(i: Vertex) -> VOut {
    var o: VOut;

    let charge = clamp(fraction, 0.0, 1.0);
    let ramp = smoothstep(0.05, 1.0, charge);
    let pulse = 0.12 * sin(globals.time * 12.0 + i.position.y * 8.0);
    let width = max(0.6, 1.0 + ramp * 4.5 + pulse);

    var local_pos = i.position;
    local_pos = vec3<f32>(local_pos.x * width, local_pos.y, local_pos.z * width);

    let world_from_local = mesh_functions::get_world_from_local(i.instance_index);
    let world_pos = (world_from_local * vec4<f32>(local_pos, 1.0)).xyz;

    o.clip_pos = position_world_to_clip(world_pos);
    o.uv = i.uv;
    o.radial = length(local_pos.xz);
    o.height = i.position.y;
    return o;
}

@fragment
fn fragment(i: VOut) -> @location(0) vec4<f32> {
    let charge = clamp(fraction, 0.0, 1.0);
    let base_color = mix(vec3<f32>(0.55, 0.15, 1.0), vec3<f32>(1.0, 0.25, 0.10), charge);
    let hot = smoothstep(0.75, 1.0, charge);
    let hot_color = mix(base_color, vec3<f32>(1.0, 0.95, 0.75), hot);

    let core = smoothstep(0.16, 0.0, i.radial);
    let rim = smoothstep(0.35, 0.12, i.radial);
    let bands = 0.6 + 0.4 * sin(i.height * 16.0 - globals.time * 7.0);

    let glow = (0.25 + 1.2 * core) * bands;
    let color = hot_color * glow;
    let alpha = clamp(core * (0.35 + 0.65 * charge) + rim * 0.25, 0.0, 1.0);

    return vec4<f32>(color, alpha);
}
