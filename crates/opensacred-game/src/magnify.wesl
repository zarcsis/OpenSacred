// How the art is filtered where it is magnified (`magnify::Filter`, #42): the weights between the
// four texels about a point, which the ground's (`ground.wesl`) and the sprites' (`sprites.wesl`)
// shaders mix by hand. Shared by both.

/// How far toward the next texel along each axis a point at `p` takes its colour, `p` in texels
/// with the first texel's centre at 0: the fraction itself, linear filtering; with `POINT` the
/// nearest texel's, as the original draws its sprites; with `SHARP` the nearest texel's but within
/// a screen pixel of the texels' edge, blended across that pixel ("sharp bilinear").
fn weights(p: vec2<f32>) -> vec2<f32> {
    let f = p - floor(p);
    var w = f;
@if(POINT) {
    w = step(vec2(0.5), f);
}
@if(SHARP) {
    // Texels per pixel of the screen, along each axis.
    let per_pixel = max(fwidth(p), vec2(0.000001));
    w = clamp((f - 0.5) / per_pixel + 0.5, vec2(0.0), vec2(1.0));
}
    return w;
}
