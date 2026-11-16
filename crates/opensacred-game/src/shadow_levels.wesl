// A level of the shadows' mask (`shadows::ShadowMask`, #2739): each texel a block of the mask's
// pixels, [`BLOCK`] across, with the two casters that cover the most of it and the share of it
// that each covers. `shadows.wesl` blurs a shadow's mask at close zooms over the level whose
// blocks are as wide as its samples lie apart: every pixel of a thin part's silhouette, a blade
// one or two pixels high, falls in a block, and a sample takes in as much of it in every row.

/// The mask: its red whole where a shadow lies, its green and blue the nearest's caster's number.
@group(0) @binding(0) var mask: texture_2d<f32>;
/// The level made: its red and green the casters' numbers, none 0, and its blue and alpha the
/// shares of the block that they cover, of [`WHOLE`].
@group(0) @binding(1) var level: texture_storage_2d<rgba16uint, write>;

/// The side of a block, in the mask's pixels: two to the level.
const BLOCK: u32 = constants::BLOCK;
/// A whole block's share.
const WHOLE: f32 = 65535.0;

/// Up to four casters of a block, none 0, and the shares of the block that they cover: kept in
/// vectors, which stay in a thread's registers where arrays indexed by a variable do not.
struct Shares {
    casters: vec4<u32>,
    amounts: vec4<f32>,
};

/// `amount` more of the block covered by `caster`, in its slot or the first free one; a fifth
/// caster, where so many shadows meet within one block, is dropped, a hole as small as the share
/// it covered.
fn add(shares: Shares, caster: u32, amount: f32) -> Shares {
    var out = shares;
    let same = out.casters == vec4(caster);
    if any(same) {
        out.amounts += select(vec4(0.0), vec4(amount), same);
        return out;
    }
    let free = out.casters == vec4(0u);
    let before = vec4(false, free.x, free.x || free.y, free.x || free.y || free.z);
    let first = free & !before;
    out.casters = select(out.casters, vec4(caster), first);
    out.amounts = select(out.amounts, vec4(amount), first);
    return out;
}

/// The block of a texel of the level: its pixels' casters' shares added, and the two that cover
/// the most kept.
@compute @workgroup_size(8, 8, 1)
fn reduce(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(level);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let limit = textureDimensions(mask);
    let corner = id.xy * BLOCK;
    let amount = 1.0 / f32(BLOCK * BLOCK);
    var shares = Shares(vec4(0u), vec4(0.0));
    for (var y = 0u; y < BLOCK; y++) {
        for (var x = 0u; x < BLOCK; x++) {
            let pixel = corner + vec2(x, y);
            if pixel.x >= limit.x || pixel.y >= limit.y {
                continue;
            }
            let texel = textureLoad(mask, pixel, 0);
            if texel.r >= 0.5 {
                let caster = u32(round(texel.g * 255.0)) | (u32(round(texel.b * 255.0)) << 8u);
                shares = add(shares, caster, amount);
            }
        }
    }
    var casters = vec2(0u);
    var amounts = vec2(0.0);
    for (var i = 0; i < 4; i++) {
        let share = shares.amounts[i];
        if share > amounts.x {
            casters = vec2(shares.casters[i], casters.x);
            amounts = vec2(share, amounts.x);
        } else if share > amounts.y {
            casters.y = shares.casters[i];
            amounts.y = share;
        }
    }
    let kept = vec2<u32>(round(min(amounts, vec2(1.0)) * WHOLE));
    textureStore(level, id.xy, vec4(casters, kept));
}
