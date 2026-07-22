use euclid::default::Point2D;
use euclid::Trig;
use num_traits::{Float, FromPrimitive};
use palette::Srgba;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone, PartialEq, Debug, Copy, Eq)]
pub enum FillStyle {
    Solid,
    Hachure,
    ZigZag,
    CrossHatch,
    Dots,
    Dashed,
    ZigZagLine,
}

impl fmt::Display for FillStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FillStyle::Solid => "Solid",
            FillStyle::Hachure => "Hachure",
            FillStyle::ZigZag => "ZigZag",
            FillStyle::CrossHatch => "CrossHatch",
            FillStyle::Dots => "Dots",
            FillStyle::Dashed => "Dashed",
            FillStyle::ZigZagLine => "ZigZagLine",
        };
        f.write_str(s)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Options for angled joins in strokes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LineJoin {
    Miter { limit: f64 },
    Round,
    Bevel,
}
impl LineJoin {
    pub const DEFAULT_MITER_LIMIT: f64 = 10.0;
}
impl Default for LineJoin {
    fn default() -> Self {
        LineJoin::Miter {
            limit: LineJoin::DEFAULT_MITER_LIMIT,
        }
    }
}

/// A Rough.js seed represented as a JavaScript Number without early integer normalization.
///
/// The Number is stored without early integer normalization. This preserves Rough.js behavior at
/// boundaries where `cloneOptionsAlterSeed` applies JavaScript `seed + 1` before `Math.imul`
/// performs its 32-bit coercion.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RoughJsSeed {
    number_bits: u64,
}

impl RoughJsSeed {
    pub const fn new(number: f64) -> Self {
        Self {
            number_bits: number.to_bits(),
        }
    }

    pub const fn number(self) -> f64 {
        f64::from_bits(self.number_bits)
    }

    /// Returns whether the base or `seed + 1` stream can reach Rough.js' `Math.random()` branch.
    pub fn may_use_math_random(self) -> bool {
        fn reaches_fallback(number: f64) -> bool {
            !javascript_truthy(number) || javascript_to_uint32(number) == 0
        }

        let number = self.number();
        reaches_fallback(number) || (javascript_truthy(number) && reaches_fallback(number + 1.0))
    }

    pub(crate) fn for_second_stroke(self) -> Self {
        let number = self.number();
        let number = if javascript_truthy(number) {
            number + 1.0
        } else {
            number
        };
        Self::new(number)
    }
}

#[derive(Clone, Debug)]
struct DeterministicMathRandom {
    state: u64,
}

impl DeterministicMathRandom {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> f64 {
        // SplitMix64 supplies a stable, non-constant stream without ambient process state.
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        ((value >> 11) as f64) / 9_007_199_254_740_992.0
    }
}

/// An explicitly injected deterministic replacement for the operation's shared `Math.random()`.
///
/// Clones retain the same ordered stream. This mirrors JavaScript's process-wide `Math.random()`
/// ownership across Rough.js generator calls without reading ambient host state.
#[derive(Clone, Debug)]
pub struct RoughMathRandom {
    initial_seed: u64,
    state: Arc<Mutex<DeterministicMathRandom>>,
}

impl RoughMathRandom {
    pub fn new(seed: u64) -> Self {
        Self {
            initial_seed: seed,
            state: Arc::new(Mutex::new(DeterministicMathRandom::new(seed))),
        }
    }

    pub const fn initial_seed(&self) -> u64 {
        self.initial_seed
    }

    /// Creates an independent stream at this stream's original operation-owned seed.
    ///
    /// This is reserved for non-emitting geometry estimates. Rendering code must clone the
    /// existing handle so all generated Rough.js shapes consume one ordered stream.
    pub fn isolated_copy(&self) -> Self {
        Self::new(self.initial_seed)
    }

    fn next(&self) -> f64 {
        self.state
            .lock()
            .expect("Rough Math.random stream mutex poisoned")
            .next()
    }
}

/// Complete caller-owned randomness contract for one Rough.js options object.
#[derive(Clone, Debug)]
pub struct RoughRandomness {
    seed: RoughJsSeed,
    math_random: RoughMathRandom,
}

impl RoughRandomness {
    pub fn new(seed: RoughJsSeed, math_random: RoughMathRandom) -> Self {
        Self { seed, math_random }
    }

    pub const fn seed(&self) -> RoughJsSeed {
        self.seed
    }

    pub const fn math_random(&self) -> &RoughMathRandom {
        &self.math_random
    }

    /// Creates a fresh deterministic stream for non-emitting geometry estimates.
    ///
    /// Do not use this for generated SVG paths: cloned `RoughRandomness` values intentionally
    /// share the operation-owned fallback stream.
    pub fn isolated_copy(&self) -> Self {
        Self::new(self.seed, self.math_random.isolated_copy())
    }

    pub(crate) fn for_second_stroke(&self) -> Self {
        Self {
            seed: self.seed.for_second_stroke(),
            math_random: self.math_random.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RoughRandomizer {
    number: f64,
    math_random: RoughMathRandom,
}

impl RoughRandomizer {
    fn new(randomness: &RoughRandomness) -> Self {
        Self {
            number: randomness.seed.number(),
            math_random: randomness.math_random.clone(),
        }
    }

    fn next(&mut self) -> f64 {
        if !javascript_truthy(self.number) {
            return self.math_random.next();
        }

        // `Math.imul(48271, seed)` first applies JavaScript ToUint32 to both operands, then
        // exposes the signed 32-bit product as the next Number-valued PRNG state.
        let product = 48_271_u32.wrapping_mul(javascript_to_uint32(self.number));
        let signed = product as i32;
        self.number = f64::from(signed);
        f64::from(signed & 0x7fff_ffff) / 2_147_483_648.0
    }
}

fn javascript_truthy(number: f64) -> bool {
    number != 0.0 && !number.is_nan()
}

fn javascript_to_uint32(number: f64) -> u32 {
    if !number.is_finite() || number == 0.0 {
        return 0;
    }

    // Decode the IEEE-754 Number directly so the low 32 integer bits match ECMAScript ToUint32
    // even for values too large for Rust's saturating float-to-integer cast.
    let bits = number.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
    if exponent < 0 {
        return 0;
    }
    let significand = (1_u64 << 52) | (bits & ((1_u64 << 52) - 1));
    let shift = exponent - 52;
    let magnitude = if shift >= 32 {
        0
    } else if shift >= 0 {
        (significand as u32).wrapping_shl(shift as u32)
    } else {
        (significand >> (-shift as u32)) as u32
    };
    if negative {
        magnitude.wrapping_neg()
    } else {
        magnitude
    }
}

#[derive(Clone, Builder)]
#[builder(setter(strip_option))]
pub struct Options {
    #[builder(default = "Some(2.0)")]
    pub max_randomness_offset: Option<f32>,
    #[builder(default = "Some(1.0)")]
    pub roughness: Option<f32>,
    #[builder(default = "Some(2.0)")]
    pub bowing: Option<f32>,
    #[builder(default = "Some(Srgba::new(0.0, 0.0, 0.0, 1.0))")]
    pub stroke: Option<Srgba>,
    #[builder(default = "Some(1.0)")]
    pub stroke_width: Option<f32>,
    #[builder(default = "Some(0.95)")]
    pub curve_fitting: Option<f32>,
    #[builder(default = "Some(0.0)")]
    pub curve_tightness: Option<f32>,
    #[builder(default = "Some(9.0)")]
    pub curve_step_count: Option<f32>,
    #[builder(default = "None")]
    pub fill: Option<Srgba>,
    #[builder(default = "None")]
    pub fill_style: Option<FillStyle>,
    #[builder(default = "Some(-1.0)")]
    pub fill_weight: Option<f32>,
    #[builder(default = "Some(-41.0)")]
    pub hachure_angle: Option<f32>,
    #[builder(default = "Some(-1.0)")]
    pub hachure_gap: Option<f32>,
    #[builder(default = "Some(1.0)")]
    pub simplification: Option<f32>,
    #[builder(default = "Some(-1.0)")]
    pub dash_offset: Option<f32>,
    #[builder(default = "Some(-1.0)")]
    pub dash_gap: Option<f32>,
    #[builder(default = "Some(-1.0)")]
    pub zigzag_offset: Option<f32>,
    pub randomness: RoughRandomness,
    #[builder(default = "None")]
    pub stroke_line_dash: Option<Vec<f64>>,
    #[builder(default = "None")]
    pub stroke_line_dash_offset: Option<f64>,
    #[builder(default = "None")]
    pub line_cap: Option<LineCap>,
    #[builder(default = "None")]
    pub line_join: Option<LineJoin>,
    #[builder(default = "None")]
    pub fill_line_dash: Option<Vec<f64>>,
    #[builder(default = "None")]
    pub fill_line_dash_offset: Option<f64>,
    #[builder(default = "Some(false)")]
    pub disable_multi_stroke: Option<bool>,
    #[builder(default = "Some(false)")]
    pub disable_multi_stroke_fill: Option<bool>,
    #[builder(default = "Some(false)")]
    pub preserve_vertices: Option<bool>,
    #[builder(default = "None")]
    pub fixed_decimal_place_digits: Option<f32>,
    // Rough.js stores the evolving seeded PRNG state in `ops.randomizer`.
    // This is internal-only and must not be user-set.
    #[builder(default = "None", setter(skip))]
    pub(crate) randomizer: Option<RoughRandomizer>,
}

impl Options {
    pub fn random(&mut self) -> f64 {
        // Match Rough.js `random(ops)` in `bin/renderer.js`:
        //
        // - `ops.seed` is represented by `ops.randomness.seed` and is stable across calls.
        // - `ops.randomizer` is lazily created and holds the evolving 32-bit state.
        self.randomizer
            .get_or_insert_with(|| RoughRandomizer::new(&self.randomness))
            .next()
    }

    pub fn set_hachure_angle(&mut self, angle: Option<f32>) -> &mut Self {
        self.hachure_angle = angle;
        self
    }

    pub fn set_hachure_gap(&mut self, gap: Option<f32>) -> &mut Self {
        self.hachure_gap = gap;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{
        javascript_to_uint32, Options, OptionsBuilder, RoughJsSeed, RoughMathRandom,
        RoughRandomness,
    };

    fn sequence(seed: f64) -> Vec<f64> {
        let mut options = OptionsBuilder::default()
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(seed),
                RoughMathRandom::new(7),
            ))
            .build()
            .unwrap();
        (0..4).map(|_| options.random()).collect()
    }

    #[test]
    fn options_require_an_explicit_randomness_contract() {
        assert!(OptionsBuilder::default().build().is_err());
    }

    #[test]
    fn falsy_seed_uses_a_stable_nonconstant_fallback_sequence() {
        let first = sequence(0.0);
        assert_eq!(first, sequence(0.0));
        assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
        assert_ne!(first, sequence(1.0));
    }

    #[test]
    fn cloned_math_random_handles_continue_one_shared_ordered_stream() {
        let shared = RoughMathRandom::new(7);
        let mut first = OptionsBuilder::default()
            .randomness(RoughRandomness::new(RoughJsSeed::new(0.0), shared.clone()))
            .build()
            .unwrap();
        let mut second = OptionsBuilder::default()
            .randomness(RoughRandomness::new(RoughJsSeed::new(0.0), shared))
            .build()
            .unwrap();
        let split = [
            first.random(),
            first.random(),
            second.random(),
            second.random(),
        ];

        let mut continuous = OptionsBuilder::default()
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(0.0),
                RoughMathRandom::new(7),
            ))
            .build()
            .unwrap();
        let expected = [
            continuous.random(),
            continuous.random(),
            continuous.random(),
            continuous.random(),
        ];
        assert_eq!(split, expected);
    }

    #[test]
    fn javascript_number_is_coerced_only_when_imul_runs() {
        let fallback = sequence(0.0);
        let power_of_two = sequence(4_294_967_296.0);
        assert_eq!(power_of_two[0], 0.0);
        assert_eq!(&power_of_two[1..], &fallback[..3]);

        assert_eq!(sequence(-1.0), sequence(4_294_967_295.0));
        assert_eq!(sequence(1.75), sequence(1.0));
    }

    #[test]
    fn javascript_to_uint32_covers_signed_and_large_number_boundaries() {
        assert_eq!(javascript_to_uint32(-1.0), u32::MAX);
        assert_eq!(javascript_to_uint32(-1.75), u32::MAX);
        assert_eq!(javascript_to_uint32(4_294_967_295.0), u32::MAX);
        assert_eq!(javascript_to_uint32(4_294_967_296.0), 0);
        assert_eq!(javascript_to_uint32(4_294_967_297.0), 1);
        assert_eq!(javascript_to_uint32(f64::INFINITY), 0);
        assert_eq!(javascript_to_uint32(f64::NAN), 0);
    }

    #[test]
    fn roughjs_random_seed_1_matches_known_sequence() {
        // Matches Rough.js `Random.next()` from `bin/math.js` with `seed = 1`.
        let denom = 2147483648.0_f64; // 2^31
        let expected_out: [u32; 10] = [
            48_271,
            182_605_793,
            1_291_342_511,
            1_533_981_633,
            1_591_223_503,
            902_075_297,
            1_698_214_639,
            773_027_713,
            144_866_575,
            647_683_937,
        ];
        let expected: Vec<f64> = expected_out.iter().map(|&n| (n as f64) / denom).collect();

        let mut opts: Options = OptionsBuilder::default()
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(1.0),
                RoughMathRandom::new(7),
            ))
            .build()
            .unwrap();
        let got: Vec<f64> = (0..expected.len()).map(|_| opts.random()).collect();

        assert_eq!(got, expected);
    }
}

#[derive(Clone, PartialEq, Debug, Eq)]
pub enum OpType {
    Move,
    BCurveTo,
    LineTo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpSetType {
    Path,
    FillPath,
    FillSketch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op<F: Float + Trig> {
    pub op: OpType,
    pub data: Vec<F>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpSet<F: Float + Trig> {
    pub op_set_type: OpSetType,
    pub ops: Vec<Op<F>>,
    pub size: Option<Point2D<F>>,
    pub path: Option<String>,
}

pub struct Drawable<F: Float + Trig> {
    pub shape: String,
    pub options: Options,
    pub sets: Vec<OpSet<F>>,
}

pub struct PathInfo {
    pub d: String,
    pub stroke: Option<Srgba>,
    pub stroke_width: Option<f32>,
    pub fill: Option<Srgba>,
}

pub fn _c<U: Float + FromPrimitive>(inp: f32) -> U {
    U::from(inp).expect("can not parse from f32")
}

pub fn _cc<U: Float + FromPrimitive>(inp: f64) -> U {
    U::from(inp).expect("can not parse from f64")
}
