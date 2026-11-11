// The ground's fragment shader (`terrain::GroundMaterial`). Each texel of the ground is composed
// of the stack of layers of the cell it lies in, from the pool of tile sheets that the loaded
// sectors share (`sheet_pool`, #2484): the ground tile, then each overlay's colour through its
// mask's alpha or its own, in integers, as the atlas's slots were composed before. The shader
// filters the ground as the one image the original's frame makes of the cells, so that filtering
// runs across the cells' edges as within the cells. The texel, as stored in sRGB values, is
// multiplied by the corners' light, as the original multiplies the 8-bit texel by the diffuse
// colour of its corners (docs/02 §5.3); the product, decoded to linear light for Bevy's last
// steps, is what the frame holds (`post::post_process`). Where the sun's soft shadows fall
// (`soft.wesl`, #1436), the product is darkened in its sRGB values, as the original's shadows
// darken the frame's.

import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};
import package::post::post_process;
import package::soft;

/// How far back along the line of sight the ground's mesh lies from its place
/// (`iso::GROUND_BEHIND`).
const GROUND_BEHIND: f32 = 5000.0;

/// The tile sheets of the loaded sectors, a layer each.
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var sheets: texture_2d_array<u32>;
/// Where the stack of every cell of the sector begins in `stacks`, row by row.
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var cells: texture_2d<u32>;
/// The sector's stacks (`terrain::encode`): each its count of layers, then a word a layer, the
/// pool's layer of its art in the low 10 bits and its diamond in the next 5, its mask's the same
/// in the 15 bits above, and bit 30 where it has a mask.
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var stacks: texture_2d<u32>;

/// Cells per side of a sector, and words per row of `stacks` (`terrain::STACKS_ROW`).
const SIDE: i32 = 64;
const STACKS_ROW: i32 = 4096;

/// sRGB values to linear light.
fn decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

/// Word `i` of the stacks.
fn word(i: i32) -> u32 {
    return textureLoad(stacks, vec2(i % STACKS_ROW, i / STACKS_ROW), 0).r;
}

/// The top left of diamond `k` (0..18) in its sheet: nine rows 25 texels apart, two columns 104
/// apart, odd rows shifted by 52, a margin of 2 (`terrain::diamond_origin`, docs/02 §5.4).
fn diamond_origin(k: u32) -> vec2<i32> {
    let row = i32(k / 2u);
    let column = i32(k % 2u);
    return vec2(104 * column + 52 * (row % 2) + 2, 25 * row);
}

/// Texel `p` of the diamond box of a tile, the low 15 bits of `tile`: its layer and its diamond.
fn tile_texel(tile: u32, p: vec2<i32>) -> vec4<u32> {
    return textureLoad(sheets, diamond_origin((tile >> 10u) & 31u) + p, i32(tile & 1023u), 0);
}

/// Texel `p` of the diamond box of the stack that begins at `at`: from nothing, each layer's
/// colour laid over through its alpha, `(c a + o (255 - a)) / 255` in integers, its alpha opaque.
fn composed(at: i32, p: vec2<i32>) -> vec4<f32> {
    let count = i32(word(at));
    var out = vec3<u32>(0u);
    for (var i = 1; i <= count; i++) {
        let layer = word(at + i);
        let colour = tile_texel(layer, p);
        var alpha = colour.a;
        if (layer & (1u << 30u)) != 0u {
            alpha = tile_texel(layer >> 15u, p).a;
        }
        if alpha == 255u {
            out = colour.rgb;
        } else if alpha != 0u {
            out = (colour.rgb * alpha + out * (255u - alpha)) / 255u;
        }
    }
    return vec4(vec3<f32>(out) / 255.0, 1.0);
}

/// The texel of the ground whose centre is `t`, in the sector's flat screen pixels: cell
/// `(c, r)` has its diamond's top corner at `(48 (c - r), 24 (c + r))`. Past the sector's edge the
/// nearest cell of the sector stands in.
fn texel(t: vec2<f32>) -> vec4<f32> {
    let c = clamp(i32(floor((t.x / 48.0 + t.y / 24.0) * 0.5)), 0, SIDE - 1);
    let r = clamp(i32(floor((t.y / 24.0 - t.x / 48.0) * 0.5)), 0, SIDE - 1);
    let at = i32(textureLoad(cells, vec2(c, r), 0).r);
    // The texel in the cell's box, whose top left lies at (48 (c - r) - 48, 24 (c + r)): within
    // the sector, one of its diamond's, as the texel lies in the cell. Past the sector's edge the
    // nearest cell's row is held at the diamond's edge: row y of the diamond runs from |2y - 47|
    // to 95 - |2y - 47|.
    let y = clamp(i32(floor(t.y - f32(24 * (c + r)))), 0, 47);
    let edge = abs(2 * y - 47);
    let x = clamp(i32(floor(t.x - f32(48 * (c - r) - 48))), edge, 95 - edge);
    return composed(at, vec2(x, y));
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
    // The sun's shadows at the ground's own place, not its mesh's.
    let place = in.world_position.xyz + view.world_from_view[2].xyz * GROUND_BEHIND;
    let sun = soft::lit(vec4(place, 1.0), vec3(0.0, soft::NORMAL_BIAS, 0.0), in.position.xy);
    return post_process(vec4(decode(colour.rgb * in.color.rgb * sun), 1.0), in.position.xy);
}
