// The ground's fragment shader (`terrain::GroundMaterial`). The cells' diamonds lie in the slots
// of the sector's atlas; the shader filters the ground as the one image the original's frame
// makes of them, each texel taken from the slot of the cell it lies in, so that filtering runs
// across the cells' edges as within the cells. The texel, as stored in sRGB values, is multiplied
// by the corners' light, as the original multiplies the 8-bit texel by the diffuse colour of its
// corners (docs/02 §5.3); the product, decoded to linear light for Bevy's last steps, is what
// the frame holds (`post::post_process`).

import bevy_pbr::render::forward_io::VertexOutput;
import package::post::post_process;

/// The slots of the sector's distinct stacks of layers (`terrain::compose`).
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
/// The slot of every cell of the sector, row by row.
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var cells: texture_2d<u32>;

/// Cells per side of a sector, and the slots' layout (`terrain::SLOT_W`, `SLOT_H`,
/// `SLOTS_PER_ROW`): a 96 x 48 box and a border of one texel.
const SIDE: i32 = 64;
const SLOT_W: i32 = 98;
const SLOT_H: i32 = 50;
const SLOTS_PER_ROW: i32 = 64;

/// sRGB values to linear light.
fn decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

/// The texel of the ground whose centre is `t`, in the sector's flat screen pixels: cell
/// `(c, r)` has its diamond's top corner at `(48 (c - r), 24 (c + r))`. Past the sector's edge the
/// nearest cell of the sector stands in.
fn texel(t: vec2<f32>) -> vec4<f32> {
    let c = clamp(i32(floor((t.x / 48.0 + t.y / 24.0) * 0.5)), 0, SIDE - 1);
    let r = clamp(i32(floor((t.y / 24.0 - t.x / 48.0) * 0.5)), 0, SIDE - 1);
    let slot = i32(textureLoad(cells, vec2(c, r), 0).r);
    // The texel in the cell's box, whose top left lies at (48 (c - r) - 48, 24 (c + r)).
    let x = clamp(i32(floor(t.x - f32(48 * (c - r) - 48))), -1, 96);
    let y = clamp(i32(floor(t.y - f32(24 * (c + r)))), -1, 48);
    let at = vec2((slot % SLOTS_PER_ROW) * SLOT_W + 1 + x, (slot / SLOTS_PER_ROW) * SLOT_H + 1 + y);
    return textureLoad(atlas, at, 0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Bilinear filtering by hand, between the four texels around the fragment.
    let p = in.uv - 0.5;
    let base = floor(p) + 0.5;
    let f = p - floor(p);
    let top = mix(texel(base), texel(base + vec2(1.0, 0.0)), f.x);
    let bottom = mix(texel(base + vec2(0.0, 1.0)), texel(base + vec2(1.0, 1.0)), f.x);
    let colour = mix(top, bottom, f.y);
    return post_process(vec4(decode(colour.rgb * in.color.rgb), 1.0), in.position.xy);
}
