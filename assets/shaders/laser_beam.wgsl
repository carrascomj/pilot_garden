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
    let pulse = 0.10 * sin(globals.time * 10.0 + i.position.y * 9.0);
    let width = max(0.45, 0.9 + ramp * 3.2 + pulse);

    let spin_speed = mix(2.0, 18.0, ramp);
    let twist = i.position.y * 9.0 + globals.time * spin_speed;
    let c = cos(twist);
    let s = sin(twist);
    let spiral = vec2<f32>(
        i.position.x * c - i.position.z * s,
        i.position.x * s + i.position.z * c
    );

    // add a helix-like radius modulation so the spiral reads clearly
    let helix = 0.55 + 0.45 * sin((i.uv.x * 6.283185307) + i.position.y * 5.5);
    var local_pos = vec3<f32>(spiral.x * width * helix, i.position.y, spiral.y * width * helix);

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
    let base_color = mix(vec3<f32>(0.72, 0.45, 0.98), vec3<f32>(0.95, 0.10, 1.00), charge);
    let hot_color = mix(base_color, vec3<f32>(1.00, 0.05, 0.85), smoothstep(0.6, 1.0, charge));
    let core = smoothstep(0.16, 0.0, i.radial);
    let rim = smoothstep(0.40, 0.14, i.radial);
    let bands = 0.6 + 0.4 * sin(i.height * 16.0 - globals.time * 7.0);
    let spiral_band = 0.4 + 0.6 * smoothstep(
        0.25,
        0.95,
        sin(i.uv.x * 12.5663706 + i.height * 8.0 - globals.time * 4.0)
    );

    let glow = (0.25 + 1.2 * core) * bands;
    let color = hot_color * glow * spiral_band;
    let alpha = clamp(core * (0.20 + 0.60 * charge) + rim * 0.16, 0.0, 1.0);

    return vec4<f32>(color, alpha);
}
