//! Granny animation curves as glTF keyframes (docs/02 §4.2, §4.5).
//!
//! A quadratic curve (mode 2) is a B-spline whose keys are control points: on the span from key
//! `j` to key `j + 1` the value blends keys `j − 1`, `j` and `j + 1` with weights quadratic in
//! time, [`basis`]; the neighbours are clamped to the curve, so it starts at the first key and ends
//! at the last. openheilig derived the basis and matched the game's own poses with it within
//! 0.005° (community-reported, `formats/granny-grn.md`). A quadratic polynomial is a cubic Hermite
//! segment, so a glTF cubic spline with a keyframe at every key carries a translation or a scale
//! curve exactly: the value at each key and the slopes on both sides of it.
//!
//! A rotation curve blends control quaternions of any length and normalizes the result, after
//! turning the two earlier keys of the span into the hemisphere of the third (openheilig). glTF
//! wants unit values, so the keyframes hold the normalized curve and its slopes; the spline that
//! glTF draws through them strays from the original by thousandths of a degree.

use opensacred_formats::grn::{Curve, Interpolation};

/// glTF sampler interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    Step,
    Linear,
    CubicSpline,
}

impl Sampling {
    pub fn name(self) -> &'static str {
        match self {
            Self::Step => "STEP",
            Self::Linear => "LINEAR",
            Self::CubicSpline => "CUBICSPLINE",
        }
    }
}

/// Keyframes of one glTF sampler: a value per time, or for a cubic spline three per time, the
/// in-tangent, the value and the out-tangent, as glTF orders them. Tangents are slopes per
/// second.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes<const N: usize> {
    pub sampling: Sampling,
    pub times: Vec<f32>,
    pub values: Vec<[f32; N]>,
}

/// Keyframes of a translation or scale curve whose keys `map` takes to glTF values; `map` must
/// be linear, as a change of frame is, so that it commutes with the blending. `None` for a curve
/// without keys.
pub fn vectors<const N: usize, const M: usize>(
    curve: &Curve<N>,
    map: impl Fn(&[f32; N]) -> [f64; M],
) -> Option<Keyframes<M>> {
    let keys: Vec<[f64; M]> = curve.keys.iter().map(map).collect();
    let first = *keys.first()?;
    if keys.iter().all(|k| *k == first) {
        return Some(constant(&curve.times, first));
    }
    let times = seconds(&curve.times);
    let values = match curve.interpolation {
        Interpolation::Constant | Interpolation::Linear => keys,
        Interpolation::Quadratic => {
            let mut values = vec![[0.0; M]; 3 * keys.len()];
            for j in 0..keys.len() - 1 {
                let (start, start_slope) = blend(&times, &keys, j, times[j]);
                let (end, end_slope) = blend(&times, &keys, j, times[j + 1]);
                values[3 * j + 1] = start;
                values[3 * j + 2] = start_slope;
                values[3 * j + 3] = end_slope;
                values[3 * j + 4] = end;
            }
            values
        }
    };
    Some(Keyframes {
        sampling: sampling(curve.interpolation),
        times: curve.times.clone(),
        values: values.iter().map(|v| v.map(narrow)).collect(),
    })
}

/// Keyframes of a rotation curve whose control quaternions `map` takes to glTF's frame (a
/// linear map); `None` for a curve without keys. Also returns the number of keys at which the
/// original curve jumps: where the hemisphere turn of one span differs from the next one's, the
/// spans meet at different rotations, and the glTF curve follows the later span.
pub fn rotations(
    curve: &Curve<4>,
    map: impl Fn(&[f32; 4]) -> [f64; 4],
) -> Option<(Keyframes<4>, usize)> {
    let keys: Vec<[f64; 4]> = curve.keys.iter().map(map).collect();
    let first = *keys.first()?;
    if keys.iter().all(|k| *k == first) {
        return Some((constant(&curve.times, unit(first)), 0));
    }
    if curve.interpolation != Interpolation::Quadratic {
        // Unit keys, each in the hemisphere of the previous one, so that glTF's spherical
        // interpolation takes the short way as blending neighbours does.
        let mut values: Vec<[f64; 4]> = Vec::with_capacity(keys.len());
        for key in &keys {
            let mut value = unit(*key);
            if values.last().is_some_and(|p| dot(p, &value) < 0.0) {
                value = value.map(|x| -x);
            }
            values.push(value);
        }
        let keyframes = Keyframes {
            sampling: sampling(curve.interpolation),
            times: curve.times.clone(),
            values: values.iter().map(|v| v.map(narrow)).collect(),
        };
        return Some((keyframes, 0));
    }

    let times = seconds(&curve.times);
    let mut spline = Spline::default();
    let mut jumps = 0;
    for j in 0..keys.len() - 1 {
        let (indices, _, _) = basis(&times, j, times[j]);
        let mut controls = indices.map(|i| keys[i]);
        let upper = controls[2];
        for control in &mut controls[..2] {
            if dot(control, &upper) < 0.0 {
                *control = control.map(|x| -x);
            }
        }
        let (start, start_slope) = normalized_blend(&times, &controls, j, times[j]);
        // The span's sign: glTF's keyframes stay in one hemisphere from key to key.
        let sign = match spline.last {
            Some(previous) if dot(&start, &previous) < 0.0 => -1.0,
            _ => 1.0,
        };
        let at = |t: f64| {
            let (r, dr) = normalized_blend(&times, &controls, j, t);
            (r.map(|x| sign * x), dr.map(|x| sign * x))
        };
        let (start, start_slope) = (start.map(|x| sign * x), start_slope.map(|x| sign * x));
        match spline.last {
            None => spline.push(times[j], start_slope, start, start_slope),
            Some(previous) => {
                // Within 1e-6 rad the spans meet; float noise stays far below that.
                if dot(&start, &previous) < (1e-6f64 / 2.0).cos() {
                    jumps += 1;
                }
                spline.restart(start, start_slope);
            }
        }
        let (end, end_slope) = at(times[j + 1]);
        spline.span(
            &at,
            (times[j], start, start_slope),
            (times[j + 1], end, end_slope),
            0,
        );
    }
    let keyframes = Keyframes {
        sampling: Sampling::CubicSpline,
        times: spline.times.iter().map(|&t| narrow(t)).collect(),
        values: spline.triples.iter().map(|v| v.map(narrow)).collect(),
    };
    Some((keyframes, jumps))
}

/// How far, in degrees, the glTF spline of a rotation may stray from the original curve before
/// a span gets a keyframe in its middle.
const ROTATION_TOLERANCE: f64 = 1e-3;
/// Most halvings of a span: 2⁸ pieces.
const MAX_HALVINGS: u32 = 8;

/// Cubic spline of unit rotations under construction: keyframe times and the glTF triples.
#[derive(Default)]
struct Spline {
    times: Vec<f64>,
    triples: Vec<[f64; 4]>,
    /// The value of the last keyframe.
    last: Option<[f64; 4]>,
}

impl Spline {
    fn push(&mut self, time: f64, in_slope: [f64; 4], value: [f64; 4], out_slope: [f64; 4]) {
        self.times.push(time);
        self.triples.extend([in_slope, value, out_slope]);
        self.last = Some(value);
    }

    /// The last keyframe starts the next span with this value and slope.
    fn restart(&mut self, value: [f64; 4], out_slope: [f64; 4]) {
        let n = self.triples.len();
        self.triples[n - 2] = value;
        self.triples[n - 1] = out_slope;
        self.last = Some(value);
    }

    /// Keyframes up to the end of the piece from `a` to `b` of a span whose unit curve and slope
    /// `at` gives; the keyframe at `a` is in place. A piece whose spline strays from `at` by more
    /// than [`ROTATION_TOLERANCE`] is halved.
    fn span(
        &mut self,
        at: &impl Fn(f64) -> ([f64; 4], [f64; 4]),
        a: (f64, [f64; 4], [f64; 4]),
        b: (f64, [f64; 4], [f64; 4]),
        halvings: u32,
    ) {
        let (ta, ra, da) = a;
        let (tb, rb, db) = b;
        let middle = 0.5 * (ta + tb);
        // A middle time must survive f32 between its neighbours.
        let separable = narrow(ta) < narrow(middle) && narrow(middle) < narrow(tb);
        if halvings < MAX_HALVINGS && separable {
            let strays = [1.0 / 6.0, 2.0 / 6.0, 0.5, 4.0 / 6.0, 5.0 / 6.0]
                .iter()
                .any(|&s| {
                    let t = ta + s * (tb - ta);
                    let spline = unit(hermite(&ra, &da, &rb, &db, tb - ta, s));
                    angle_degrees(&spline, &at(t).0) > ROTATION_TOLERANCE
                });
            if strays {
                let (rm, dm) = at(middle);
                self.span(at, a, (middle, rm, dm), halvings + 1);
                self.span(at, (middle, rm, dm), b, halvings + 1);
                return;
            }
        }
        self.push(tb, db, rb, db);
    }
}

/// glTF's cubic Hermite segment from `a` to `b` over `delta` seconds at the fraction `s`.
fn hermite(
    a: &[f64; 4],
    da: &[f64; 4],
    b: &[f64; 4],
    db: &[f64; 4],
    delta: f64,
    s: f64,
) -> [f64; 4] {
    let (s2, s3) = (s * s, s * s * s);
    std::array::from_fn(|m| {
        (2.0 * s3 - 3.0 * s2 + 1.0) * a[m]
            + (s3 - 2.0 * s2 + s) * delta * da[m]
            + (-2.0 * s3 + 3.0 * s2) * b[m]
            + (s3 - s2) * delta * db[m]
    })
}

/// Angle in degrees between two unit quaternions, as rotations.
fn angle_degrees(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    2.0 * dot(a, b).abs().min(1.0).acos().to_degrees()
}

/// Longest step between the samples of a sheared track, in seconds.
const SHEAR_STEP: f64 = 1.0 / 60.0;

/// 3×3 matrix, rows.
pub type Mat3 = [[f64; 3]; 3];

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A symmetric scale/shear matrix `SS = U · diag(scale) · Uᵀ` with `U` a rotation, given as the
/// unit quaternion `rotation`. glTF scales along a node's own axes only, so a bone with shear
/// becomes two nodes: one with the bone's translation, the rotation `R · U` and `scale`, and
/// under it the bone's own node turned by `Uᵀ`; together they make `T · R · SS` exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shear {
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
    /// `U` as a matrix, to keep the next decomposition close to this one.
    axes: Mat3,
}

impl Shear {
    /// Decomposes `ss`, choosing the order and the signs of the axes closest to `near` (the
    /// previous sample's), or to the identity: a sequence of matrices gets axes that turn
    /// smoothly instead of swapping places.
    pub fn new(ss: &Mat3, near: Option<&Self>) -> Self {
        let (values, vectors) = symmetric_eigen(ss);
        let reference = near.map_or(&IDENTITY, |s| &s.axes);
        let column = |m: &Mat3, j: usize| [m[0][j], m[1][j], m[2][j]];
        let mut best = ([0, 1, 2], f64::NEG_INFINITY);
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let fit: f64 = (0..3)
                .map(|j| dot(&column(&vectors, order[j]), &column(reference, j)).abs())
                .sum();
            if fit > best.1 {
                best = (order, fit);
            }
        }
        let order = best.0;
        let mut axes = [[0.0; 3]; 3];
        let mut fits = [0.0; 3];
        for j in 0..3 {
            let mut c = column(&vectors, order[j]);
            let fit = dot(&c, &column(reference, j));
            if fit < 0.0 {
                c = c.map(|x| -x);
            }
            fits[j] = fit.abs();
            for (i, &x) in c.iter().enumerate() {
                axes[i][j] = x;
            }
        }
        // A reflection: turning the worst-fitting axis around keeps U·D·Uᵀ and makes U a
        // rotation.
        if determinant(&axes) < 0.0 {
            let worst = (0..3)
                .min_by(|&a, &b| fits[a].total_cmp(&fits[b]))
                .unwrap_or(2);
            for row in &mut axes {
                row[worst] = -row[worst];
            }
        }
        Self {
            rotation: quaternion(&axes),
            scale: order.map(|k| values[k]),
            axes,
        }
    }
}

/// The scale/shear matrix of 9 stored values; symmetric, so rows and columns read the same.
pub fn matrix(values: &[f64; 9]) -> Mat3 {
    [
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ]
}

/// Keyframes of a sheared track ([`sheared`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ShearKeyframes {
    /// The rotation `R · U` of the outer node.
    pub outer_rotation: Keyframes<4>,
    /// The scale of the outer node.
    pub scale: Keyframes<3>,
    /// The rotation `Uᵀ` of the bone's own node.
    pub inner_rotation: Keyframes<4>,
}

/// Keyframes of a sheared track (see [`Shear`]), sampled at the keys of both curves and at least
/// every [`SHEAR_STEP`]; glTF interpolates between the samples linearly. `map_rotation` and
/// `map_scale_shear` take the keys to glTF's frame and must be linear. `None` if either curve
/// has no keys.
pub fn sheared(
    rotation: &Curve<4>,
    scale_shear: &Curve<9>,
    map_rotation: impl Fn(&[f32; 4]) -> [f64; 4],
    map_scale_shear: impl Fn(&[f32; 9]) -> [f64; 9],
) -> Option<ShearKeyframes> {
    let rotation_keys: Vec<[f64; 4]> = rotation.keys.iter().map(map_rotation).collect();
    let shear_keys: Vec<[f64; 9]> = scale_shear.keys.iter().map(map_scale_shear).collect();
    if rotation_keys.is_empty() || shear_keys.is_empty() {
        return None;
    }
    let (rotation_times, shear_times) = (seconds(&rotation.times), seconds(&scale_shear.times));

    let mut knots: Vec<f32> = rotation
        .times
        .iter()
        .chain(&scale_shear.times)
        .copied()
        .collect();
    knots.sort_by(f32::total_cmp);
    knots.dedup();
    let mut times: Vec<f32> = Vec::new();
    for pair in knots.windows(2) {
        let (a, b) = (f64::from(pair[0]), f64::from(pair[1]));
        let mut pieces = 1u32;
        while (b - a) / f64::from(pieces) > SHEAR_STEP {
            pieces += 1;
        }
        for k in 0..pieces {
            let t = narrow(a + (b - a) * f64::from(k) / f64::from(pieces));
            if times.last().is_none_or(|&last| t > last) {
                times.push(t);
            }
        }
    }
    if let Some(&end) = knots.last()
        && times.last().is_none_or(|&last| end > last)
    {
        times.push(end);
    }

    let mut previous: Option<Shear> = None;
    let (mut outer, mut scale, mut inner) = (Vec::new(), Vec::new(), Vec::new());
    for &t in &times {
        let t = f64::from(t);
        let r = evaluate(
            rotation.interpolation,
            &rotation_times,
            &rotation_keys,
            t,
            true,
        );
        let ss = evaluate(
            scale_shear.interpolation,
            &shear_times,
            &shear_keys,
            t,
            false,
        );
        let shear = Shear::new(&matrix(&ss), previous.as_ref());
        // Each rotation in the hemisphere of its previous sample, for the short way.
        let mut o = compose(&r, &shear.rotation);
        let mut i = conjugate(&shear.rotation);
        if outer.last().is_some_and(|p: &[f64; 4]| dot(p, &o) < 0.0) {
            o = o.map(|x| -x);
        }
        if inner.last().is_some_and(|p: &[f64; 4]| dot(p, &i) < 0.0) {
            i = i.map(|x| -x);
        }
        outer.push(o);
        inner.push(i);
        scale.push(shear.scale);
        previous = Some(shear);
    }
    let linear = |values: &[[f64; 4]]| Keyframes {
        sampling: Sampling::Linear,
        times: times.clone(),
        values: values.iter().map(|v| v.map(narrow)).collect(),
    };
    Some(ShearKeyframes {
        outer_rotation: linear(&outer),
        inner_rotation: linear(&inner),
        scale: Keyframes {
            sampling: Sampling::Linear,
            times: times.clone(),
            values: scale.iter().map(|v| v.map(narrow)).collect(),
        },
    })
}

/// Value of a curve whose times are `times` and whose (mapped) keys are `keys` at time `t`, as
/// the original evaluates it: the key at or before `t` (constant), a straight blend (linear) or
/// the B-spline of [`basis`] (quadratic). A rotation (`rotation`) blends its keys turned into
/// one hemisphere and is normalized.
pub fn evaluate<const M: usize>(
    interpolation: Interpolation,
    times: &[f64],
    keys: &[[f64; M]],
    t: f64,
    rotation: bool,
) -> [f64; M] {
    let last = times.len() - 1;
    let finish = |v: [f64; M]| {
        if rotation {
            let length = dot(&v, &v).sqrt();
            if length > 1e-12 {
                return v.map(|x| x / length);
            }
        }
        v
    };
    if t <= times[0] || last == 0 {
        return finish(keys[0]);
    }
    if t >= times[last] {
        return finish(keys[last]);
    }
    let j = times.partition_point(|&x| x <= t) - 1;
    match interpolation {
        Interpolation::Constant => finish(keys[j]),
        Interpolation::Linear => {
            let s = (t - times[j]) / (times[j + 1] - times[j]);
            let (a, mut b) = (keys[j], keys[j + 1]);
            if rotation && dot(&a, &b) < 0.0 {
                b = b.map(|x| -x);
            }
            finish(std::array::from_fn(|m| a[m] + s * (b[m] - a[m])))
        }
        Interpolation::Quadratic => {
            let (indices, weights, _) = basis(times, j, t);
            let mut controls = indices.map(|i| keys[i]);
            if rotation {
                let upper = controls[2];
                for control in &mut controls[..2] {
                    if dot(control, &upper) < 0.0 {
                        *control = control.map(|x| -x);
                    }
                }
            }
            let mut value = [0.0; M];
            for (control, w) in controls.iter().zip(weights) {
                for m in 0..M {
                    value[m] += w * control[m];
                }
            }
            finish(value)
        }
    }
}

/// Eigenvalues and eigenvectors (the columns of the second matrix) of a symmetric matrix, by
/// cyclic Jacobi rotations: `m = U · diag(λ) · Uᵀ`.
fn symmetric_eigen(m: &Mat3) -> ([f64; 3], Mat3) {
    let mut a = *m;
    let mut v = IDENTITY;
    for _ in 0..64 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        let scale = a.iter().flatten().map(|x| x * x).sum::<f64>();
        if off <= 1e-30 * scale.max(1e-300) {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            // The classical Jacobi rotation (Golub and Van Loan, §8.5): the rotation J that zeroes
            // a[p][q] in Jᵀ·a·J.
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            let mut j = IDENTITY;
            j[p][p] = c;
            j[q][q] = c;
            j[p][q] = s;
            j[q][p] = -s;
            a = multiply(&multiply(&transpose(&j), &a), &j);
            v = multiply(&v, &j);
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

fn multiply(a: &Mat3, b: &Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn transpose(a: &Mat3) -> Mat3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

fn determinant(m: &Mat3) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Unit quaternion `[x, y, z, w]` of a rotation matrix (Shepperd's method).
fn quaternion(m: &Mat3) -> [f64; 4] {
    let trace = m[0][0] + m[1][1] + m[2][2];
    let q = if trace > 0.0 {
        let s = 2.0 * (trace + 1.0).sqrt();
        [
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
            0.25 * s,
        ]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = 2.0 * (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt();
        [
            0.25 * s,
            (m[0][1] + m[1][0]) / s,
            (m[0][2] + m[2][0]) / s,
            (m[2][1] - m[1][2]) / s,
        ]
    } else if m[1][1] > m[2][2] {
        let s = 2.0 * (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt();
        [
            (m[0][1] + m[1][0]) / s,
            0.25 * s,
            (m[1][2] + m[2][1]) / s,
            (m[0][2] - m[2][0]) / s,
        ]
    } else {
        let s = 2.0 * (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt();
        [
            (m[0][2] + m[2][0]) / s,
            (m[1][2] + m[2][1]) / s,
            0.25 * s,
            (m[1][0] - m[0][1]) / s,
        ]
    };
    unit(q)
}

/// Hamilton product `a · b` of quaternions `[x, y, z, w]`: `b` turns first.
pub fn compose(a: &[f64; 4], b: &[f64; 4]) -> [f64; 4] {
    let [ax, ay, az, aw] = *a;
    let [bx, by, bz, bw] = *b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// The inverse of a unit quaternion.
pub fn conjugate(q: &[f64; 4]) -> [f64; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

/// The quadratic basis on the span from key `j` to key `j + 1` of a curve with key times
/// `times`, at time `t`: the keys `j − 1`, `j`, `j + 1` (clamped to the curve), their weights and
/// the weights' derivatives in time. With the neighbouring times `ta ≤ tb < tc ≤ td`:
/// `f1 = (t − tb)/(tc − tb)`, `f2 = (t − ta)/(tc − ta)`, `g = f1 + f2 − f1·f2`,
/// `h = f1·(t − tb)/(td − tb)`; the weights are `1 − g`, `g − h`, `h` (openheilig).
fn basis(times: &[f64], j: usize, t: f64) -> ([usize; 3], [f64; 3], [f64; 3]) {
    let last = times.len() - 1;
    let indices = [j.saturating_sub(1), j, (j + 1).min(last)];
    let [ta, tb, tc] = indices.map(|i| times[i]);
    let td = times[(j + 2).min(last)];
    let (f1, df1) = ((t - tb) / (tc - tb), 1.0 / (tc - tb));
    let (f2, df2) = ((t - ta) / (tc - ta), 1.0 / (tc - ta));
    let g = f1 + f2 - f1 * f2;
    let dg = df1 + df2 - df1 * f2 - f1 * df2;
    let (e, de) = ((t - tb) / (td - tb), 1.0 / (td - tb));
    let (h, dh) = (e * f1, de * f1 + e * df1);
    (indices, [1.0 - g, g - h, h], [-dg, dg - dh, dh])
}

/// Value and slope of a quadratic curve on span `j` at time `t`.
fn blend<const M: usize>(
    times: &[f64],
    keys: &[[f64; M]],
    j: usize,
    t: f64,
) -> ([f64; M], [f64; M]) {
    let (indices, weights, slopes) = basis(times, j, t);
    let mut value = [0.0; M];
    let mut slope = [0.0; M];
    for ((&i, w), s) in indices.iter().zip(weights).zip(slopes) {
        for m in 0..M {
            value[m] += w * keys[i][m];
            slope[m] += s * keys[i][m];
        }
    }
    (value, slope)
}

/// Unit value and slope of the rotation blended from `controls` (the keys of span `j`, turned
/// into one hemisphere) at time `t`: with `q` the blend, `r = q/|q|` and
/// `r′ = (q′ − r (r·q′))/|q|`.
fn normalized_blend(
    times: &[f64],
    controls: &[[f64; 4]; 3],
    j: usize,
    t: f64,
) -> ([f64; 4], [f64; 4]) {
    let (_, weights, slopes) = basis(times, j, t);
    let mut q = [0.0; 4];
    let mut dq = [0.0; 4];
    for ((control, w), s) in controls.iter().zip(weights).zip(slopes) {
        for m in 0..4 {
            q[m] += w * control[m];
            dq[m] += s * control[m];
        }
    }
    let length = dot(&q, &q).sqrt();
    if length < 1e-12 {
        return ([0.0, 0.0, 0.0, 1.0], [0.0; 4]);
    }
    let r = q.map(|x| x / length);
    let along = dot(&r, &dq);
    let mut dr = [0.0; 4];
    for m in 0..4 {
        dr[m] = (dq[m] - r[m] * along) / length;
    }
    (r, dr)
}

/// A curve whose keys are all `value`: two keyframes at its first and last time, so that the
/// clip keeps its length; one for a curve with one key.
fn constant<const M: usize>(times: &[f32], value: [f64; M]) -> Keyframes<M> {
    let mut ends = times[..1].to_vec();
    ends.extend(times.last().filter(|_| times.len() > 1));
    Keyframes {
        sampling: Sampling::Linear,
        values: vec![value.map(narrow); ends.len()],
        times: ends,
    }
}

fn sampling(interpolation: Interpolation) -> Sampling {
    match interpolation {
        Interpolation::Constant => Sampling::Step,
        Interpolation::Linear => Sampling::Linear,
        Interpolation::Quadratic => Sampling::CubicSpline,
    }
}

fn seconds(times: &[f32]) -> Vec<f64> {
    times.iter().map(|&t| f64::from(t)).collect()
}

fn dot<const M: usize>(a: &[f64; M], b: &[f64; M]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// `q` scaled to unit length; the identity for a zero quaternion.
fn unit(q: [f64; 4]) -> [f64; 4] {
    let length = dot(&q, &q).sqrt();
    if length < 1e-12 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        q.map(|x| x / length)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "glTF stores f32; rounding to the nearest f32 is intended"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// The original curve at time `t`, straight from the formula in docs/02 §4.2, written apart
    /// from [`basis`]: on the span `(tb, tc)` with neighbour times `ta`, `td`, the keys
    /// `ka, kb, kc` weigh `1 − g`, `g − h`, `h`; a rotation turns `ka`, `kb` into the hemisphere
    /// of `kc` and is normalized after blending.
    pub fn original<const N: usize>(curve: &Curve<N>, t: f32, rotation: bool) -> [f64; N] {
        let times = &curve.times;
        let n = times.len();
        let key = |i: usize| curve.keys[i].map(f64::from);
        if t <= times[0] {
            let first = key(0);
            let length = if rotation {
                dot(&first, &first).sqrt()
            } else {
                1.0
            };
            return first.map(|x| x / length);
        }
        let j = times.iter().rposition(|&x| x <= t).unwrap().min(n - 2);
        let (ia, ic, id) = (j.saturating_sub(1), j + 1, (j + 2).min(n - 1));
        let [ta, tb, tc, td, t] = [times[ia], times[j], times[ic], times[id], t].map(f64::from);
        let f1 = (t - tb) / (tc - tb);
        let f2 = (t - ta) / (tc - ta);
        let g = f1 + f2 - f1 * f2;
        let h = (t - tb) / (td - tb) * f1;
        let (mut ka, mut kb, kc) = (key(ia), key(j), key(ic));
        if rotation {
            for k in [&mut ka, &mut kb] {
                if dot(k, &kc) < 0.0 {
                    *k = k.map(|x| -x);
                }
            }
        }
        let mut out = [0.0; N];
        for m in 0..N {
            out[m] = (1.0 - g) * ka[m] + (g - h) * kb[m] + h * kc[m];
        }
        if rotation {
            let length = dot(&out, &out).sqrt();
            out = out.map(|x| x / length);
        }
        out
    }

    /// The glTF curve of `keyframes` at time `t`, as glTF 2.0 Appendix C defines its samplers;
    /// spherical linear interpolation is not needed by these tests.
    pub fn sample<const N: usize>(keyframes: &Keyframes<N>, t: f32) -> [f64; N] {
        let times = &keyframes.times;
        let cubic = keyframes.sampling == Sampling::CubicSpline;
        let value = |k: usize| {
            let i = if cubic { 3 * k + 1 } else { k };
            keyframes.values[i].map(f64::from)
        };
        if t <= times[0] || times.len() == 1 {
            return value(0);
        }
        let last = times.len() - 1;
        if t >= times[last] {
            return value(last);
        }
        let k = times.iter().rposition(|&x| x <= t).unwrap();
        let delta = f64::from(times[k + 1] - times[k]);
        let s = f64::from(t - times[k]) / delta;
        let mut out = [0.0; N];
        match keyframes.sampling {
            Sampling::Step => out = value(k),
            Sampling::Linear => {
                let (a, b) = (value(k), value(k + 1));
                for m in 0..N {
                    out[m] = a[m] + s * (b[m] - a[m]);
                }
            }
            Sampling::CubicSpline => {
                let (a, b) = (value(k), value(k + 1));
                let out_tangent = keyframes.values[3 * k + 2].map(f64::from);
                let in_tangent = keyframes.values[3 * k + 3].map(f64::from);
                let (s2, s3) = (s * s, s * s * s);
                for m in 0..N {
                    out[m] = (2.0 * s3 - 3.0 * s2 + 1.0) * a[m]
                        + (s3 - 2.0 * s2 + s) * delta * out_tangent[m]
                        + (-2.0 * s3 + 3.0 * s2) * b[m]
                        + (s3 - s2) * delta * in_tangent[m];
                }
            }
        }
        out
    }

    fn quadratic<const N: usize>(times: &[f32], keys: &[[f32; N]]) -> Curve<N> {
        Curve {
            interpolation: Interpolation::Quadratic,
            times: times.to_vec(),
            keys: keys.to_vec(),
        }
    }

    fn widen<const N: usize>(key: &[f32; N]) -> [f64; N] {
        key.map(f64::from)
    }

    /// Times across the curve, 50 per span, the keys included.
    fn probes(times: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        for pair in times.windows(2) {
            for s in 0u8..50 {
                out.push(pair[0] + (pair[1] - pair[0]) * f32::from(s) / 50.0);
            }
        }
        out.push(*times.last().unwrap());
        out
    }

    #[test]
    fn the_basis_is_the_uniform_b_spline_inside_and_pins_the_ends() {
        let times = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let (indices, weights, _) = basis(&times, 2, 2.5);
        assert_eq!(indices, [1, 2, 3]);
        for (w, expected) in weights.iter().zip([0.125, 0.75, 0.125]) {
            assert!((w - expected).abs() < 1e-12, "{weights:?}");
        }
        // The curve starts at the first key and ends at the last.
        let (indices, weights, _) = basis(&times, 0, 0.0);
        assert_eq!((indices, weights[0] + weights[1]), ([0, 0, 1], 1.0));
        let (indices, weights, _) = basis(&times, 4, 5.0);
        assert_eq!((indices, weights[2]), ([3, 4, 5], 1.0));
        // The derivatives are those of the weights, which always add up to 1.
        for t in [0.1, 0.5, 0.9] {
            let (_, w0, d) = basis(&times, 0, t);
            let (_, w1, _) = basis(&times, 0, t + 1e-6);
            assert!(d.iter().sum::<f64>().abs() < 1e-12);
            for k in 0..3 {
                assert!(((w1[k] - w0[k]) / 1e-6 - d[k]).abs() < 1e-5, "{t}: {d:?}");
            }
        }
    }

    #[test]
    fn a_cubic_spline_carries_a_quadratic_curve_exactly() {
        // Uneven times and an even number of keys, as in 9,956 translation curves.
        let times = [0.0, 0.1, 0.25, 0.3, 0.7, 1.3];
        let keys = [
            [0.0, 1.0, 2.0],
            [3.0, -1.0, 0.5],
            [2.0, 2.0, 2.0],
            [-4.0, 0.0, 1.0],
            [1.0, 1.0, -3.0],
            [0.5, 0.5, 0.5],
        ];
        let curve = quadratic(&times, &keys);
        let keyframes = vectors(&curve, widen).unwrap();
        assert_eq!(keyframes.sampling, Sampling::CubicSpline);
        assert_eq!(keyframes.times, times);
        assert_eq!(keyframes.values.len(), 18);
        for t in probes(&times) {
            let (a, b) = (original(&curve, t, false), sample(&keyframes, t));
            for m in 0..3 {
                assert!((a[m] - b[m]).abs() < 1e-5, "t {t}: {a:?} != {b:?}");
            }
        }
        // The ends are the first and the last key.
        assert_eq!(keyframes.values[1], keys[0]);
        assert_eq!(keyframes.values[16], keys[5]);
    }

    #[test]
    fn the_map_is_applied_to_the_keys() {
        let curve = Curve {
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            keys: vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
        };
        let keyframes = vectors(&curve, |&[x, y, z]| [x, z, -y].map(f64::from)).unwrap();
        assert_eq!(keyframes.sampling, Sampling::Linear);
        assert_eq!(keyframes.values, [[1.0, 3.0, -2.0], [4.0, 6.0, -5.0]]);
        let step = Curve {
            interpolation: Interpolation::Constant,
            ..curve
        };
        assert_eq!(vectors(&step, widen).unwrap().sampling, Sampling::Step);
    }

    #[test]
    fn a_constant_curve_keeps_two_keyframes_at_its_ends() {
        let curve = quadratic(&[0.0, 0.5, 2.0], &[[1.0, 2.0, 3.0]; 3]);
        let keyframes = vectors(&curve, widen).unwrap();
        assert_eq!(
            keyframes,
            Keyframes {
                sampling: Sampling::Linear,
                times: vec![0.0, 2.0],
                values: vec![[1.0, 2.0, 3.0]; 2],
            }
        );
        let empty: Curve<3> = quadratic(&[], &[]);
        assert!(vectors(&empty, widen).is_none());
        // A constant rotation is stored unit.
        let curve = quadratic(&[0.0, 1.0, 2.0], &[[0.0, 0.0, 0.0, 2.0]; 3]);
        let (keyframes, jumps) = rotations(&curve, widen).unwrap();
        assert_eq!(
            (keyframes.values, jumps),
            (vec![[0.0, 0.0, 0.0, 1.0]; 2], 0)
        );
    }

    /// Keys turning about an axis by `steps` degrees each, with lengths off unit as in the game
    /// and every other key in the opposite hemisphere.
    fn spinning(axis: [f32; 3], steps: &[f32]) -> Vec<[f32; 4]> {
        let mut angle = 0.0f32;
        let mut keys = Vec::new();
        for (k, step) in steps.iter().enumerate() {
            angle += step;
            let (s, c) = (angle.to_radians() / 2.0).sin_cos();
            let length = 1.0 + 0.02 * if k % 3 == 1 { 1.0 } else { -0.5 };
            let sign = if k % 2 == 1 { -1.0 } else { 1.0 };
            let q = [axis[0] * s, axis[1] * s, axis[2] * s, c];
            keys.push(q.map(|x| x * length * sign));
        }
        keys
    }

    #[test]
    fn a_rotation_curve_follows_the_original_within_a_thousandth_of_a_degree() {
        let times = [0.0, 0.2, 0.3, 0.6, 0.8, 1.2, 1.25];
        let axis = [0.6, 0.0, 0.8];
        let keys = spinning(axis, &[0.0, 25.0, 10.0, 30.0, -20.0, 35.0, 5.0]);
        let curve = quadratic(&times, &keys);
        let (keyframes, jumps) = rotations(&curve, widen).unwrap();
        assert_eq!((keyframes.sampling, jumps), (Sampling::CubicSpline, 0));
        // glTF's values are unit and in one hemisphere, key after key.
        assert!(keyframes.times.len() > times.len(), "{:?}", keyframes.times);
        for k in 0..keyframes.times.len() {
            let v = keyframes.values[3 * k + 1].map(f64::from);
            assert!((dot(&v, &v) - 1.0).abs() < 1e-6);
            if k > 0 {
                assert!(dot(&v, &keyframes.values[3 * k - 2].map(f64::from)) > 0.0);
            }
        }
        let mut worst = 0.0f64;
        for t in probes(&times) {
            let mut glt = sample(&keyframes, t);
            let length = dot(&glt, &glt).sqrt();
            glt = glt.map(|x| x / length);
            worst = worst.max(angle_degrees(&original(&curve, t, true), &glt));
        }
        assert!(worst < 1e-3, "{worst}°");
    }

    #[test]
    fn a_jump_between_spans_is_counted() {
        // Turns of 0°, 100°, 200° and 210° about Z: keys 0 and 2 are more than 180° apart, so
        // span 1 turns key 0 into the hemisphere of key 2 and starts at another rotation than
        // span 0 ends at.
        let keys = [0.0f32, 100.0, 200.0, 210.0].map(|a| {
            let (s, c) = (a.to_radians() / 2.0).sin_cos();
            [0.0, 0.0, s, c]
        });
        let curve = quadratic(&[0.0, 1.0, 2.0, 3.0], &keys);
        let (_, jumps) = rotations(&curve, widen).unwrap();
        assert_eq!(jumps, 1);
    }

    #[test]
    fn linear_rotations_are_unit_and_take_the_short_way() {
        let curve = Curve {
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.02, 0.04],
            keys: vec![
                [0.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, -0.1, -0.99],
                [0.0, 0.0, 0.2, 0.98],
            ],
        };
        let (keyframes, _) = rotations(&curve, widen).unwrap();
        assert_eq!(keyframes.sampling, Sampling::Linear);
        let values: Vec<[f64; 4]> = keyframes.values.iter().map(|v| v.map(f64::from)).collect();
        for pair in values.windows(2) {
            assert!(dot(&pair[0], &pair[1]) > 0.0, "{values:?}");
        }
        for v in &values {
            assert!((dot(v, v) - 1.0).abs() < 1e-6);
        }
    }
}

#[cfg(test)]
mod shear_tests {
    use super::tests::sample;
    use super::*;

    /// The wolf's `Bip01 Head` in `WOLF_ATTACK_BH_A.GRN`, a strong shear.
    const HEAD: [f64; 9] = [
        4.389, 0.741, -0.504, 0.741, 1.014, -0.116, -0.504, -0.116, 1.406,
    ];

    fn rotation_matrix(q: &[f64; 4]) -> Mat3 {
        let [x, y, z, w] = *q;
        [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - z * w),
                2.0 * (x * z + y * w),
            ],
            [
                2.0 * (x * y + z * w),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - x * w),
            ],
            [
                2.0 * (x * z - y * w),
                2.0 * (y * z + x * w),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ]
    }

    fn diagonal(d: &[f64; 3]) -> Mat3 {
        [[d[0], 0.0, 0.0], [0.0, d[1], 0.0], [0.0, 0.0, d[2]]]
    }

    fn assert_close(a: &Mat3, b: &Mat3, tolerance: f64) {
        for i in 0..3 {
            for j in 0..3 {
                assert!((a[i][j] - b[i][j]).abs() < tolerance, "{a:?} != {b:?}");
            }
        }
    }

    /// `R(outer) · diag(scale) · R(inner)`, what the two glTF nodes make.
    fn nodes(outer: &[f64; 4], scale: &[f64; 3], inner: &[f64; 4]) -> Mat3 {
        multiply(
            &multiply(&rotation_matrix(outer), &diagonal(scale)),
            &rotation_matrix(inner),
        )
    }

    #[test]
    fn a_shear_is_a_rotated_scale() {
        for values in [
            HEAD,
            [1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0],
            [-0.148, 0.0, 0.0, 0.0, -0.096, 0.0, 0.0, 0.0, -0.075],
            [2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0],
        ] {
            let m = matrix(&values);
            let shear = Shear::new(&m, None);
            let u = rotation_matrix(&shear.rotation);
            assert!((determinant(&u) - 1.0).abs() < 1e-9);
            let rebuilt = multiply(&multiply(&u, &diagonal(&shear.scale)), &transpose(&u));
            assert_close(&rebuilt, &m, 1e-9);
        }
        // A diagonal matrix keeps its axes.
        let shear = Shear::new(
            &matrix(&[1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0]),
            None,
        );
        assert_eq!(shear.scale, [1.0, 2.0, 3.0]);
        assert!((shear.rotation[3].abs() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn axes_follow_a_turning_shear() {
        // A stretch of 3 along an axis that turns about Z: the axes turn with it, the stretch
        // stays on the same axis.
        let mut previous: Option<Shear> = None;
        for step in 0..40u8 {
            let angle = f64::from(step) * 0.05;
            let (s, c) = angle.sin_cos();
            let r = [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]];
            let m = multiply(&multiply(&r, &diagonal(&[3.0, 1.0, 2.0])), &transpose(&r));
            let shear = Shear::new(&m, previous.as_ref());
            assert!(
                (shear.scale[0] - 3.0).abs() < 1e-9,
                "{step}: {:?}",
                shear.scale
            );
            if let Some(p) = &previous {
                assert!(dot(&p.rotation, &shear.rotation).abs() > 0.99, "{step}");
            }
            previous = Some(shear);
        }
    }

    #[test]
    fn a_sheared_track_is_exact_at_its_samples_and_close_between() {
        let rotation = Curve {
            interpolation: Interpolation::Quadratic,
            times: vec![0.0, 0.3, 0.5, 1.0],
            keys: vec![
                [0.0, 0.0, 0.0, 1.0],
                [0.0, 0.3, 0.0, 0.95],
                [0.2, 0.3, 0.0, 0.93],
                [0.3, 0.0, 0.1, 0.95],
            ],
        };
        let scale_shear = Curve {
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.7, 1.0],
            keys: vec![
                HEAD.map(narrow),
                [2.871, 1.7, -0.3, 1.7, 1.626, 0.2, -0.3, 0.2, 2.85],
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            ],
        };
        let widen4 = |k: &[f32; 4]| k.map(f64::from);
        let widen9 = |k: &[f32; 9]| k.map(f64::from);
        let keyframes = sheared(&rotation, &scale_shear, widen4, widen9).unwrap();
        let times = &keyframes.scale.times;
        assert!(times.contains(&0.7) && times.contains(&1.0), "{times:?}");
        assert!(
            times
                .windows(2)
                .all(|w| w[1] > w[0] && f64::from(w[1] - w[0]) <= SHEAR_STEP + 1e-6)
        );

        let exact = |t: f32| {
            let t = f64::from(t);
            let rt = seconds(&rotation.times);
            let rk: Vec<_> = rotation.keys.iter().map(widen4).collect();
            let st = seconds(&scale_shear.times);
            let sk: Vec<_> = scale_shear.keys.iter().map(widen9).collect();
            let r = evaluate(Interpolation::Quadratic, &rt, &rk, t, true);
            let ss = evaluate(Interpolation::Linear, &st, &sk, t, false);
            multiply(&rotation_matrix(&r), &matrix(&ss))
        };
        let glt = |t: f32| {
            let unit_of = |q: [f64; 4]| unit(q);
            let o = unit_of(sample(&keyframes.outer_rotation, t));
            let i = unit_of(sample(&keyframes.inner_rotation, t));
            nodes(&o, &sample(&keyframes.scale, t), &i)
        };
        for &t in times {
            assert_close(&glt(t), &exact(t), 1e-5);
        }
        // Between the samples the linear blend stays within 1% of the largest entry.
        for k in 0..100u8 {
            let t = f32::from(k) / 100.0;
            assert_close(&glt(t), &exact(t), 0.05);
        }
    }
}

/// Tests on the GOG installation files: every curve of every motion, converted, against the
/// original evaluated by [`tests::original`]. CI does not have the game files.
#[cfg(test)]
mod game_tests {
    use opensacred_formats::grn::Layout;
    use opensacred_formats::mdl::MdlPak;

    use super::tests::{original, sample};
    use super::*;
    use crate::verify::read_game_file;

    /// Times across a curve: four per span and the keys.
    fn probes(times: &[f32]) -> impl Iterator<Item = f32> + '_ {
        times
            .windows(2)
            .flat_map(|w| (0u8..4).map(move |s| w[0] + (w[1] - w[0]) * f32::from(s) / 4.0))
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_curve_converts_within_its_bounds() {
        let game = opensacred_locate::original();
        let (mut translations, mut rotations_checked, mut with_jumps) = (0, 0, 0);
        let (mut worst_translation, mut worst_rotation) = (0.0f64, 0.0f64);
        for path in ["PAK/models.pak", "PAK/models03.pak"] {
            let data = read_game_file(&game.join(path)).unwrap();
            let pak = MdlPak::parse(&data).unwrap();
            for index in 0..pak.len() {
                let record = pak.record(index).unwrap();
                let Some(grn) = record.parse_grn().unwrap() else {
                    continue;
                };
                for track in grn.animation().unwrap().tracks {
                    if track.layout != Layout::Split {
                        continue;
                    }
                    let t = &track.translation;
                    let keyframes = vectors(t, |k| k.map(f64::from)).unwrap();
                    let size = t
                        .keys
                        .iter()
                        .flatten()
                        .fold(1.0f64, |m, &v| m.max(f64::from(v).abs()));
                    for time in probes(&t.times) {
                        let (a, b) = (original(t, time, false), sample(&keyframes, time));
                        let error = (0..3).map(|m| (a[m] - b[m]).abs()).fold(0.0, f64::max) / size;
                        worst_translation = worst_translation.max(error);
                    }
                    translations += 1;

                    let r = &track.rotation;
                    let (keyframes, jumps) = rotations(r, |k| k.map(f64::from)).unwrap();
                    if jumps > 0 {
                        with_jumps += 1;
                        continue;
                    }
                    for time in probes(&r.times) {
                        let mut glt = sample(&keyframes, time);
                        let length = dot(&glt, &glt).sqrt();
                        glt = glt.map(|x| x / length);
                        let error = angle_degrees(&original(r, time, true), &glt);
                        worst_rotation = worst_rotation.max(error);
                    }
                    rotations_checked += 1;
                }
            }
        }
        assert_eq!(
            (translations, rotations_checked + with_jumps),
            (255_493, 255_493)
        );
        // f32 keys: a relative error of a few ulps.
        assert!(worst_translation < 1e-5, "translation: {worst_translation}");
        // The halving looks at five points of a piece; the worst stray lies between them.
        assert!(worst_rotation < 2.5e-3, "rotation: {worst_rotation}°");
        assert!(with_jumps < 1000, "{with_jumps} curves with jumps");
    }
}
