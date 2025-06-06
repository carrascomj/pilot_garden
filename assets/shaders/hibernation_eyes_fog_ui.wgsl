/* cloud_fog.wgsl – animated fog clouds with blinking eye‑shaped mask
   ------------------------------------------------------------------

- Uses Bevy UI’s UiVertexOutput so it can be drawn on a quad/9‑slice
- Time is supplied as a single f32 uniform (e.g. via `Time::<Realtime>::elapsed_seconds_f32()`)
- Disolve time is set by the rust code when the game starts (after clicking Start).
When disolve_time > 0, the eyes will open and it will decrease to show the world.
*/

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> time: f32;
@group(1) @binding(1) var<uniform> disolve_time: f32;

/* handy constants */
const PI = 3.14159265359;

/* 2‑D rotation (angle in radians) */
fn rot2(theta: f32) -> mat2x2<f32> {
    let c = cos(theta);
    let s = sin(theta);
    return mat2x2<f32>(c, -s, s, c);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // 1.  Base UV in [-1, +1]
    var uv = in.uv * 2.0 - vec2f(1.0);

    // 2.  Blink envelope  (0 = shut, 1 = fully open)
    // A sine‑based tri‑wave gives long open times and quick blinks.
    // Period ≈ 4 s  (adjust blink_speed)
    let blink_speed   = 1.3;                      // lower → slower blinks
    let phase         = sin(time * blink_speed) * 0.5 + 0.5;  // 0‒1
    let blink         = pow(phase, 4.0);          // emphasise closed moment

    // eye_height ∈ [0.05, 1] so we never divide by 0
    var eye_height = mix(0.05, 1.0, blink);
    // start game logic: open eyes and remove fog through linear interpolation
    let since_start = clamp(time - disolve_time, 0., 5.) / 5.;
    if disolve_time > 0 {
        eye_height = 2.0 * since_start + (1.0 - since_start) * eye_height;
    }

    // 3.  Eye‑shaped mask: squash/unsquash the vertical axis.
    var mask_uv = uv;
    mask_uv.y  = mask_uv.y / eye_height;          // closed eye ⇒ huge y ⇒ mask off

    let r      = length(mask_uv);
    let wall   = smoothstep(0.6, 1.0, r);         // 0 centre … 1 edge
    let core   = 1.0 - wall;

    // 4.  Animated swirling noise
    let t   = time * 0.7;
    var q   = uv * 3.2;
    q       = rot2(t * 0.17) * q;

    let p1  = q + vec2f(t, -t * 1.3);
    let p2  = q * 1.6 - vec2f(t * 0.8);

    var f = sin(p1.x) * sin(p1.y);
    f    += 0.5 * sin(p2.x) * sin(p2.y);
    f     = f * 0.5 + 0.5;

    // 5.  Colour palette (same as capsule PBR shader)
    let fog_colour   = vec3f(0.20, 0.60, 0.90);   // cyan‑green
    let glow_colour  = vec3f(1.00, 0.30, 0.90);   // magenta
    let glow_colour2 = vec3f(0.30, 0.90, 0.60);   // teal

    let glow  = smoothstep(0.6, 0.95, f)  * pow(core, 2.0);

    var f2 = sin(p1.x + PI) * cos(p1.y);
    f2    += 0.9 * sin(p2.x) * sin(p2.y);
    f2     = f2 * 0.5 + 0.5;

    let glow2 = smoothstep(0.6, 0.95, f2) * pow(core, 2.0);

    let colour       = mix(fog_colour, glow_colour,  glow);
    let final_colour = mix(colour,     glow_colour2, glow2);

    // 6.  Alpha & premultiply – eye mask controls visibility
    var alpha = clamp(core * 0.85 + 0.15 * f, 0.0, 1.0);
    // start game logic: remove fog
    if disolve_time > 0 {
        alpha = 0. * since_start + (1.0 - since_start) * alpha;
    }
    return vec4<f32>(final_colour * alpha, alpha);
}
