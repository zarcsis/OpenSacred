// A band of the interface (`ui_layers`): its picture, assembled at the original's pixels by its
// own camera, scaled onto the window. The picture holds the frame's sRGB values with their alpha
// premultiplied (its pieces blended over a transparent clear), stored in an sRGB texture, which
// decodes each texel as it is read; each is encoded back to its value, and the four about a point
// are blended in those values, as the interface's own images are filtered (`hud`, `srgb`) and the
// original's Direct3D 7 device filters, then drawn premultiplied on the window's frame, which
// holds sRGB values too (`ui_layers::Picture`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var picture: texture_2d<f32>;

/// The sRGB value of the linear `light`.
fn encoded(light: f32) -> f32 {
    if light <= 0.0031308 {
        return light * 12.92;
    }
    return 1.055 * pow(light, 1.0 / 2.4) - 0.055;
}

/// The texel at `at`, within the picture, in its sRGB values.
fn texel(at: vec2<i32>, last: vec2<i32>) -> vec4<f32> {
    let light = textureLoad(picture, clamp(at, vec2(0), last), 0);
    return vec4(encoded(light.r), encoded(light.g), encoded(light.b), light.a);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(picture));
    let last = size - vec2(1);
    let point = in.uv * vec2<f32>(size) - 0.5;
    let corner = floor(point);
    let share = point - corner;
    let at = vec2<i32>(corner);
    let top = mix(texel(at, last), texel(at + vec2(1, 0), last), share.x);
    let bottom = mix(texel(at + vec2(0, 1), last), texel(at + vec2(1, 1), last), share.x);
    return mix(top, bottom, share.y);
}
