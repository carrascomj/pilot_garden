/* lab_fog.wgsl – swirling volumetric fog for a cylinder
   ------------------------------------------------------ */

#import bevy_pbr::mesh_view_bindings::globals      // globals.time
#import bevy_render::view::View                    // for viewport aspect
#import bevy_pbr::forward_io::VertexOutput         // default vertex out

@group(0) @binding(0) var<uniform> view: View;

/* 2-D rotation (angle in radians) */
fn rot2(theta: f32) -> mat2x2<f32> {
    let c = cos(theta);
    let s = sin(theta);
    return mat2x2<f32>(c, -s, s, c);
}

/* very cheap pseudo-noise */
fn noise(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {

    /* ---------- UV remap: centre (0,0) at cylinder axis ---------- */
    var uv = in.uv * 2.0 - vec2f(1.0);         // now in [-1,1]²
    uv.x *= view.viewport.x / view.viewport.z; // keep circles round

    /* radial fall-off so fog fades near the glass */
    let r      = length(uv);
    let wall   = smoothstep(0.2, 0.7, r);    // 0 in centre, →1 at wall
    let core   = 1.0 - wall;                   // 1 in centre, →0 at wall

    /* ---------- animated swirl noise ----------------------------- */
    let t   = globals.time * 1.9;
    let q   = uv * 3.0;                       // base frequency
    let p1  = q + vec2f(t, -t);
    let p2  = q * 1.7 - vec2f(t * 1.2);

    // layered sine noise (fast, good enough for fog)
    var f = sin(p1.x) * sin(p1.y);
    f    += 0.5 * sin(p2.x) * sin(p2.y);
    f     = f * 0.5 + 0.5;                    // → 0‒1

    /* ---------- colour ------------------------------------------- */
    let fog_colour  = vec3f(0.2, 0.6, 0.9);   // green-cyan
    let glow_colour = vec3f(1.0, 0.3, 0.9);   // inner glow tint
    let glow_colour_2 = vec3f(0.3, 0.9, 0.6);   // inner glow tint

    // bright glows pulse where noise is high & near cylinder axis
    let glow = smoothstep(0.6, 0.95, f) * pow(core, 2.0);
    // bright glows pulse where noise is high & near cylinder axis
    var f_2 = sin(p1.x) * cos(p1.y);
    f_2    += 0.9 * sin(p2.x) * sin(p2.y);
    f_2     = f_2 * 0.5 + 0.3;                    // → 0‒1
    let glow_2 = smoothstep(0.6, 0.95, f_2) * pow(core, 2.0);

    let colour = mix(fog_colour, glow_colour, glow);
    let final_colour = mix(colour, glow_colour_2, glow_2);

    /* ---------- alpha:  fully opaque at centre, fades at wall ---- */
    let alpha = clamp(core * 0.8 + 0.2 * f, 0.0, 1.0);

    return vec4<f32>(final_colour.x, final_colour.y, final_colour.z, alpha);
}
