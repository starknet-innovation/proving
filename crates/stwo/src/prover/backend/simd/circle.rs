use std::iter::zip;
use std::mem::transmute;
use std::simd::Simd;

use bytemuck::Zeroable;
#[cfg(not(feature = "parallel"))]
use itertools::Itertools;
use num_traits::{One, Zero};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use tracing::{Level, span};

use super::SimdBackend;
use super::fft::{CACHED_FFT_LOG_SIZE, MIN_FFT_LOG_SIZE, ifft, rfft};
use super::m31::{LOG_N_LANES, N_LANES, PackedBaseField};
use super::qm31::PackedSecureField;
use crate::core::circle::{CirclePoint, CirclePointIndex, Coset, M31_CIRCLE_LOG_ORDER};
use crate::core::constraints::{coset_vanishing, coset_vanishing_derivative, point_vanishing};
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::fields::{Field, FieldExpOps, batch_inverse};
use crate::core::poly::circle::{CanonicCoset, CircleDomain};
use crate::core::poly::utils::{domain_line_twiddles_from_tree, fold, get_folding_alphas};
use crate::core::utils::{SliceExt, bit_reverse_index};
use crate::prover::backend::cpu::circle::slow_precompute_twiddles;
use crate::prover::backend::simd::column::{BaseColumn, advise_pages};
use crate::prover::backend::simd::fft::transpose_vecs;
use crate::prover::backend::simd::fri::fold_circle_evaluation_into_line;
use crate::prover::backend::simd::m31::PackedM31;
use crate::prover::backend::{Col, Column, CpuBackend};
use crate::prover::fri::FriOps;
use crate::prover::mempool::BaseColumnPool;
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::secure_column::SecureColumnByCoords;

impl SimdBackend {
    // TODO(Ohad): optimize.
    fn twiddle_at<F: Field>(mappings: &[F], mut index: usize) -> F {
        debug_assert!(
            (1 << mappings.len()) as usize >= index,
            "Index out of bounds. mappings log len = {}, index = {index}",
            mappings.len().ilog2()
        );

        let mut product = F::one();
        for num in mappings.iter() {
            if index & 1 == 1 {
                product *= *num;
            }
            index >>= 1;
            if index == 0 {
                break;
            }
        }

        product
    }

    // TODO(Ohad): consider moving this to to a more general place.
    // Note: CACHED_FFT_LOG_SIZE is specific to the backend.
    fn generate_evaluation_mappings<F: Field>(point: CirclePoint<F>, log_size: u32) -> Vec<F> {
        // Mappings are the factors used to compute the evaluation twiddle.
        // Every twiddle (i) is of the form (m[0])^b_0 * (m[1])^b_1 * ... * (m[log_size -
        // 1])^b_log_size.
        // Where (m)_j are the mappings, and b_i is the j'th bit of i.
        let mut mappings = vec![point.y, point.x];
        let mut x = point.x;
        for _ in 2..log_size {
            x = CirclePoint::double_x(x);
            mappings.push(x);
        }

        // The caller function expects the mapping in natural order. i.e. (y,x,h(x),h(h(x)),...).
        // If the polynomial is large, the fft does a transpose in the middle in a granularity of 16
        // (avx512). The coefficients would then be in transposed order of 16-sized chunks.
        // i.e. (a_(n-15), a_(n-14), ..., a_(n-1), a_(n-31), ..., a_(n-16), a_(n-32), ...).
        // To compute the twiddles in the correct order, we need to transpose the coprresponding
        // 'transposed bits' in the mappings. The result order of the mappings would then be
        // (y, x, h(x), h^2(x), h^(log_n-1)(x), h^(log_n-2)(x) ...). To avoid code
        // complexity for now, we just reverse the mappings, transpose, then reverse back.
        // TODO(Ohad): optimize. consider changing the caller to expect the mappings in
        // reversed-transposed order.
        if log_size > CACHED_FFT_LOG_SIZE {
            mappings.reverse();
            let n = mappings.len();
            let n0 = (n - LOG_N_LANES as usize) / 2;
            let n1 = (n - LOG_N_LANES as usize).div_ceil(2);
            let (ab, c) = mappings.split_at_mut(n1);
            let (a, _b) = ab.split_at_mut(n0);
            // Swap content of a,c.
            a.swap_with_slice(&mut c[0..n0]);
            mappings.reverse();
        }

        mappings
    }

    // Generates twiddle steps for efficiently computing the twiddles.
    // steps[i] = t_i/(t_0*t_1*...*t_i-1).
    fn twiddle_steps<F: Field + FieldExpOps>(mappings: &[F]) -> Vec<F> {
        let mut denominators: Vec<F> = vec![mappings[0]];

        for i in 1..mappings.len() {
            denominators.push(denominators[i - 1] * mappings[i]);
        }

        let denom_inverses = F::batch_inverse(&denominators);

        let mut steps = vec![mappings[0]];

        mappings.iter().skip(1).zip(denom_inverses.iter()).for_each(|(m, d)| {
            steps.push(*m * *d);
        });
        steps.push(F::one());
        steps
    }

    // Advances the twiddle by multiplying it by the next step. e.g:
    //      If idx(t) = 0b100..1010 , then f(t) = t * step[0]
    //      If idx(t) = 0b100..0111 , then f(t) = t * step[3]
    fn advance_twiddle<F: Field>(twiddle: F, steps: &[F], curr_idx: usize) -> F {
        twiddle * steps[curr_idx.trailing_ones() as usize]
    }
}

// TODO(shahars): Everything is returned in redundant representation, where values can also be P.
// Decide if and when it's ok and what to do if it's not.
/// [`PolyOps::interpolate`] for the SIMD backend, with the coefficients of large columns written to
/// a buffer taken from `pool` when there is one.
fn interpolate_ex(
    eval: CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>,
    twiddles: &TwiddleTree<SimdBackend>,
    pool: Option<&BaseColumnPool<SimdBackend>>,
) -> CircleCoefficients<SimdBackend> {
    let _span = span!(Level::TRACE, "", class = "iFFT").entered();
    let log_size = eval.values.length.ilog2();
    if log_size < MIN_FFT_LOG_SIZE {
        let cpu_poly = eval.to_cpu().interpolate();
        return CircleCoefficients::new(cpu_poly.coeffs.into_iter().collect());
    }

    let mut values = eval.values;
    let twiddles = domain_line_twiddles_from_tree(eval.domain, &twiddles.itwiddles);

    // TODO(alont): Cache this inversion.
    let inv = PackedBaseField::broadcast(BaseField::from(eval.domain.size()).inverse());
    let log_n_elements = log_size as usize;
    let log_n_vecs = log_n_elements - LOG_N_LANES as usize;

    // With a pool, the coefficients of a large column are written to a buffer of the pool (or a
    // fresh one), through the fused first pass and transposition of `ifft_out_of_place`, and
    // the evaluation buffer goes back to the pool.
    let pooled = match pool {
        Some(pool) if log_n_elements > CACHED_FFT_LOG_SIZE as usize => Some((
            pool,
            pool.try_take(log_size).unwrap_or_else(|| BaseColumn::zeros(1 << log_size)),
        )),
        _ => None,
    };

    // Safe because [PackedBaseField] is aligned on 64 bytes.
    unsafe {
        let ptr = transmute::<*mut PackedBaseField, *mut u32>(values.data.as_mut_ptr());
        if let Some((pool, mut coeffs)) = pooled {
            ifft::ifft_out_of_place(
                ptr.cast_const(),
                transmute::<*mut PackedBaseField, *mut u32>(coeffs.data.as_mut_ptr()),
                &twiddles,
                log_n_elements,
                inv,
            );
            pool.give_back(log_size, values);
            return CircleCoefficients::new(coeffs);
        }
        if log_n_elements <= CACHED_FFT_LOG_SIZE as usize {
            ifft::ifft(ptr, &twiddles, log_n_elements);
            #[cfg(not(feature = "parallel"))]
            values.data.iter_mut().for_each(|x| *x *= inv);
            #[cfg(feature = "parallel")]
            values
                .data
                .par_chunks_mut(1 << 10)
                .for_each(|chunk| chunk.iter_mut().for_each(|x| *x *= inv));
        } else {
            // The passes of `ifft::ifft`, with the scaling by `inv` done by the last pass while
            // each chunk is still in cache instead of by a pass of its own over the array.
            let fft_layers_pre_transpose = log_n_vecs.div_ceil(2);
            let fft_layers_post_transpose = log_n_vecs / 2;
            ifft::ifft_lower_with_vecwise(
                ptr,
                &twiddles[..3 + fft_layers_pre_transpose],
                log_n_elements,
                fft_layers_pre_transpose + LOG_N_LANES as usize,
            );
            transpose_vecs(ptr, log_n_vecs);
            ifft::ifft_lower_without_vecwise(
                ptr,
                &twiddles[3 + fft_layers_pre_transpose..],
                log_n_elements,
                fft_layers_post_transpose,
                Some(inv),
            );
        }
    }

    CircleCoefficients::new(values)
}

/// Weights for the first `2^s` bit-reversed entries of an evaluation on `coset`'s domain: the twin
/// coset `±(G_{n+1} + <G_{s-1}>)`, which interpolates the FFT space of size `2^s` exactly. Its
/// vanishing polynomial is `pi^(s-1)(x) - pi^(s-1)(x_0)`, its derivative the canonic one.
fn twin_coset_barycentric_weights_into(
    coset: CanonicCoset,
    p: CirclePoint<SecureField>,
    mut buffer: SecureColumnByCoords<SimdBackend>,
) -> SecureColumnByCoords<SimdBackend> {
    let s = buffer.len().ilog2();
    assert!(s >= LOG_N_LANES && s < coset.log_size() && buffer.len() == 1 << s);
    let initial = CirclePointIndex::subgroup_gen(coset.log_size() + 1);
    let domain = CircleDomain::new(Coset::new(initial, s - 1));
    let weights_vec_len = domain.size() / N_LANES;

    let p = p.into_ef::<SecureField>();
    let p_0 = domain.at(0).into_ef::<SecureField>();
    let si_0 = SecureField::one()
        / ((p_0.y * SecureField::from(-2)) * coset_vanishing_derivative(Coset::new(initial, s), p_0));

    #[cfg(not(feature = "parallel"))]
    let vi_p = (0..weights_vec_len)
        .map(|i| {
            PackedSecureField::from_array(std::array::from_fn(|j| {
                point_vanishing(
                    domain.at(bit_reverse_index(i * N_LANES + j, s)).into_ef::<SecureField>(),
                    p,
                )
            }))
        })
        .collect_vec();

    #[cfg(feature = "parallel")]
    let vi_p: Vec<PackedSecureField> = (0..weights_vec_len)
        .into_par_iter()
        .map(|i| {
            PackedSecureField::from_array(std::array::from_fn(|j| {
                point_vanishing(
                    domain.at(bit_reverse_index(i * N_LANES + j, s)).into_ef::<SecureField>(),
                    p,
                )
            }))
        })
        .collect();

    let vi_p_inverse = batch_inverse(&vi_p);

    let mut vn_p = p.x;
    let mut vn_0 = SecureField::from(p_0.x);
    for _ in 1..s {
        vn_p = CirclePoint::double_x(vn_p);
        vn_0 = CirclePoint::double_x(vn_0);
    }
    let vn_p = vn_p - vn_0;

    // Even bit-reversed indices lie on the half coset, odd ones on its conjugate.
    let si_i_vn_p = PackedSecureField::from_array(std::array::from_fn(|i| {
        if i.is_multiple_of(2) { si_0 * vn_p } else { -si_0 * vn_p }
    }));

    #[cfg(not(feature = "parallel"))]
    for (i, vi_p_inverse) in vi_p_inverse.iter().enumerate() {
        for (column, value) in buffer.columns.iter_mut().zip((*vi_p_inverse * si_i_vn_p).into_packed_m31s()) {
            column.data[i] = value;
        }
    }

    #[cfg(feature = "parallel")]
    {
        const CHUNK_SIZE: usize = 1 << 10;
        buffer.par_chunks_mut(CHUNK_SIZE).zip(vi_p_inverse.par_chunks(CHUNK_SIZE)).for_each(
            |(mut chunk, vi_p_inverse_chunk)| {
                for (i, vi_p_inverse) in vi_p_inverse_chunk.iter().enumerate() {
                    for (column, value) in chunk.0.iter_mut().zip((*vi_p_inverse * si_i_vn_p).into_packed_m31s()) {
                        column.0[i] = value;
                    }
                }
            },
        );
    }

    buffer
}

/// Constants shared by all 256-word chunks of a twin-coset interpolation.
/// Adapted from Claude/Fable v12's grouped, blocked OODS evaluation.
struct TwinCosetWeightSetup {
    vanishing_scale: PackedSecureField,
    point_x: PackedSecureField,
    point_y: PackedSecureField,
    lanes: [PackedM31; 5],
    initial: CirclePointIndex,
    low_points: Vec<CirclePoint<BaseField>>,
}

fn twin_coset_weight_setup(
    coset: CanonicCoset,
    p: CirclePoint<SecureField>,
    len: usize,
) -> TwinCosetWeightSetup {
    let s = len.ilog2();
    assert!(s >= LOG_N_LANES && s < coset.log_size() && len == 1 << s);
    let initial = CirclePointIndex::subgroup_gen(coset.log_size() + 1);
    let index = |value: usize, bits: u32, shift: u32| {
        CirclePointIndex(value.reverse_bits() >> (usize::BITS - bits) << shift)
    };
    let (mut vanishing_at_point, mut vanishing_at_initial) = (p.x, initial.to_point().x);
    for _ in 1..s {
        vanishing_at_point = CirclePoint::double_x(vanishing_at_point);
        vanishing_at_initial = CirclePoint::double_x(vanishing_at_initial);
    }
    let derivative = -BaseField::from_u32_unchecked(1 << s)
        * CirclePointIndex::subgroup_gen(coset.log_size() - s + 2).to_point().y;
    let vanishing_scale = PackedSecureField::broadcast(
        (vanishing_at_point - SecureField::from(vanishing_at_initial)) * derivative.inverse(),
    );

    // A packed word uses three circle-index bits and the conjugation bit. The next eight
    // bits select a word within a 256-word chunk; the remaining twenty select the chunk.
    let low_points =
        (0..(len / N_LANES).min(256)).map(|word| index(word, 8, 20).to_point()).collect();
    let low: [_; 8] = std::array::from_fn(|lane| index(lane, 3, 28).to_point());
    let sign = |lane: usize, value: BaseField| if lane & 1 == 0 { value } else { -value };
    let pack = |f: &dyn Fn(usize) -> BaseField| PackedM31::from_array(std::array::from_fn(f));
    let lanes = [
        pack(&|lane| low[lane >> 1].x),
        pack(&|lane| low[lane >> 1].y),
        pack(&|lane| sign(lane, low[lane >> 1].x)),
        pack(&|lane| sign(lane, low[lane >> 1].y)),
        pack(&|lane| sign(lane, BaseField::one())),
    ];
    TwinCosetWeightSetup {
        vanishing_scale,
        point_x: PackedSecureField::broadcast(p.x),
        point_y: PackedSecureField::broadcast(p.y),
        lanes,
        initial,
        low_points,
    }
}

/// Generates at most 256 packed weights, reused across every column before advancing.
fn twin_coset_weight_chunk(
    setup: &TwinCosetWeightSetup,
    chunk_index: usize,
    len: usize,
) -> Vec<PackedSecureField> {
    let [lane_x, lane_y, signed_x, signed_y, signs] = setup.lanes;
    let one = PackedM31::broadcast(BaseField::one());
    let high_index = CirclePointIndex(chunk_index.reverse_bits() >> (usize::BITS - 20));
    let high_point = (setup.initial + high_index).to_point();
    let mut numerators = Vec::with_capacity(len);
    let mut denominators = Vec::with_capacity(len);
    for low_point in &setup.low_points[..len] {
        let point = high_point + *low_point;
        let x = PackedM31::broadcast(point.x);
        let y = PackedM31::broadcast(point.y);
        let domain_x = x * lane_x - y * lane_y;
        let domain_y = x * signed_y + y * signed_x;
        numerators.push((setup.point_x * domain_x + setup.point_y * domain_y + one) * signs);
        denominators.push(setup.point_y * domain_x - setup.point_x * domain_y);
    }
    let mut inverses = vec![PackedSecureField::zero(); len];
    super::qm31::batch_inverse_packed_qm31(&denominators, &mut inverses);
    zip(numerators, inverses)
        .map(|(numerator, inverse)| numerator * inverse * setup.vanishing_scale)
        .collect()
}

impl PolyOps for SimdBackend {
    // The twiddles type is i32, and not BaseField. This is because the fast AVX mul implementation
    //  requires one of the numbers to be shifted left by 1 bit. This is not a reduced
    //  representation of the field.
    type Twiddles = Vec<u32>;

    fn interpolate(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleCoefficients<Self> {
        interpolate_ex(eval, twiddles, None)
    }

    fn interpolate_pooled(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        twiddles: &TwiddleTree<Self>,
        pool: &BaseColumnPool<Self>,
    ) -> CircleCoefficients<Self> {
        interpolate_ex(eval, twiddles, Some(pool))
    }

    fn eval_at_point(
        poly: &CircleCoefficients<Self>,
        point: CirclePoint<SecureField>,
    ) -> SecureField {
        // If the polynomial is small, fallback to evaluate directly.
        // TODO(Ohad): it's possible to avoid falling back. Consider fixing.
        if poly.log_size() <= 8 {
            return slow_eval_at_point(poly, point);
        }

        let mappings = Self::generate_evaluation_mappings(point, poly.log_size());

        // 8 lowest mappings produce the first 2^8 twiddles. Separate to optimize each calculation.
        let (map_low, map_high) = mappings.split_at(4);
        let twiddle_lows =
            PackedSecureField::from_array(std::array::from_fn(|i| Self::twiddle_at(map_low, i)));
        let (map_mid, map_high) = map_high.split_at(4);
        let twiddle_mids =
            PackedSecureField::from_array(std::array::from_fn(|i| Self::twiddle_at(map_mid, i)));

        // Compute the high twiddle steps.
        let twiddle_steps = Self::twiddle_steps(map_high);

        // Every twiddle is a product of mappings that correspond to '1's in the bit representation
        // of the current index. For every 2^n aligned chunk of 2^n elements, the twiddle
        // array is the same, denoted twiddle_low. Use this to compute sums of (coeff *
        // twiddle_high) mod 2^n, then multiply by twiddle_low, and sum to get the final result.
        let compute_chunk_sum = |coeff_chunk: &[PackedBaseField],
                                 twiddle_mids: PackedSecureField,
                                 offset: usize| {
            let mut sum = PackedSecureField::zeroed();
            let mut twiddle_high = Self::twiddle_at(&mappings, offset * N_LANES);
            for (i, coeff_chunk) in coeff_chunk.checked_as_chunks::<N_LANES>().iter().enumerate() {
                // For every chunk of 2 ^ 4 * 2 ^ 4 = 2 ^ 8 elements, the twiddle high is the same.
                // Multiply it by every mid twiddle factor to get the factors for the current chunk.
                let high_twiddle_factors =
                    (PackedSecureField::broadcast(twiddle_high) * twiddle_mids).to_array();

                // Sum the coefficients multiplied by each corrseponsing twiddle. Result is
                // effectively an array[16] where the value at index 'i' is the sum
                // of all coefficients at indices that are i mod 16.
                for (&packed_coeffs, mid_twiddle) in zip(coeff_chunk, high_twiddle_factors) {
                    sum += PackedSecureField::broadcast(mid_twiddle) * packed_coeffs;
                }

                // Advance twiddle high.
                twiddle_high = Self::advance_twiddle(twiddle_high, &twiddle_steps, offset + i);
            }
            sum
        };

        #[cfg(not(feature = "parallel"))]
        let sum = compute_chunk_sum(&poly.coeffs.data, twiddle_mids, 0);

        #[cfg(feature = "parallel")]
        let sum: PackedSecureField = {
            const CHUNK_SIZE: usize = 1 << 10;
            let chunks = poly.coeffs.data.par_chunks(CHUNK_SIZE).enumerate();
            chunks
                .into_par_iter()
                .map(|(i, chunk)| compute_chunk_sum(chunk, twiddle_mids, i * CHUNK_SIZE))
                .sum()
        };

        (sum * twiddle_lows).pointwise_sum()
    }

    fn barycentric_log_size(log_size: u32, log_blowup: u32) -> u32 {
        log_size.saturating_sub(log_blowup).max(LOG_N_LANES).min(log_size)
    }

    fn barycentric_weights_into(
        coset: CanonicCoset,
        p: CirclePoint<SecureField>,
        mut buffer: SecureColumnByCoords<SimdBackend>,
    ) -> SecureColumnByCoords<SimdBackend> {
        let domain = coset.circle_domain();
        let log_size = domain.log_size();
        if buffer.len() < domain.size() {
            return twin_coset_barycentric_weights_into(coset, p, buffer);
        }
        assert_eq!(buffer.len(), domain.size());
        let weights_vec_len = domain.size().div_ceil(N_LANES);
        if weights_vec_len == 1 {
            // The domain fits in a single packed element, whose unused lanes are zeroed.
            let weights =
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::barycentric_weights(
                    coset, p,
                )
                .to_vec();
            let mut lanes = [SecureField::zero(); N_LANES];
            lanes[..weights.len()].copy_from_slice(&weights);
            // Safety: the buffer holds exactly one packed element.
            unsafe { buffer.set_packed(0, PackedSecureField::from_array(lanes)) };
            return buffer;
        }

        let p = p.into_ef::<SecureField>();
        let p_0 = domain.at(0).into_ef::<SecureField>();
        let si_0 = SecureField::one()
            / ((p_0.y * SecureField::from(-2))
                * coset_vanishing_derivative(
                    Coset::new(CirclePointIndex::generator(), log_size),
                    p_0,
                ));

        #[cfg(not(feature = "parallel"))]
        let vi_p = (0..weights_vec_len)
            .map(|i| {
                PackedSecureField::from_array(std::array::from_fn(|j| {
                    point_vanishing(
                        domain
                            .at(bit_reverse_index(i * N_LANES + j, log_size))
                            .into_ef::<SecureField>(),
                        p,
                    )
                }))
            })
            .collect_vec();

        #[cfg(feature = "parallel")]
        let vi_p: Vec<PackedSecureField> = (0..weights_vec_len)
            .into_par_iter()
            .map(|i| {
                PackedSecureField::from_array(std::array::from_fn(|j| {
                    point_vanishing(
                        domain
                            .at(bit_reverse_index(i * N_LANES + j, log_size))
                            .into_ef::<SecureField>(),
                        p,
                    )
                }))
            })
            .collect();

        let vi_p_inverse = batch_inverse(&vi_p);

        let vn_p: SecureField = coset_vanishing(CanonicCoset::new(log_size).coset, p);

        // S_i(i) is invariant under G_(n−1) and alternate under J, meaning the S_i(i) values are
        // the same for each half coset, and the second half coset values are the conjugate
        // of the first half coset values.
        // weights_vec_len is even because domain.size() is a power of 2 (we already dealt with the
        // case where domain.size() < N_LANES).
        let si_i_vn_p = PackedSecureField::from_array(std::array::from_fn(|i| {
            if i.is_multiple_of(2) { si_0 * vn_p } else { -si_0 * vn_p }
        }));

        #[cfg(not(feature = "parallel"))]
        for (i, vi_p_inverse) in vi_p_inverse.iter().enumerate() {
            // Safety: the buffer holds `weights_vec_len` packed elements.
            unsafe { buffer.set_packed(i, *vi_p_inverse * si_i_vn_p) };
        }

        #[cfg(feature = "parallel")]
        {
            const CHUNK_SIZE: usize = 1 << 10;
            buffer.par_chunks_mut(CHUNK_SIZE).zip(vi_p_inverse.par_chunks(CHUNK_SIZE)).for_each(
                |(mut chunk, vi_p_inverse_chunk)| {
                    for (i, vi_p_inverse) in vi_p_inverse_chunk.iter().enumerate() {
                        // Safety: the chunks are of equal length.
                        unsafe { chunk.set_packed(i, *vi_p_inverse * si_i_vn_p) };
                    }
                },
            );
        }

        buffer
    }

    fn barycentric_eval_at_point(
        evals: &CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>,
        weights: &SecureColumnByCoords<SimdBackend>,
    ) -> SecureField {
        // The weights are held by coordinates, so the product with a base field value is the
        // coordinate-wise product, and the sum is accumulated coordinate-wise as well.
        let weight_at = |i: usize| {
            PackedSecureField::from_packed_m31s(std::array::from_fn(|j| weights.columns[j].data[i]))
        };

        #[cfg(not(feature = "parallel"))]
        return (0..weights.len().div_ceil(N_LANES))
            .fold(PackedSecureField::zero(), |acc, i| acc + (weight_at(i) * evals.values.data[i]))
            .pointwise_sum();

        #[cfg(feature = "parallel")]
        return (0..weights.len().div_ceil(N_LANES))
            .into_par_iter()
            .fold(PackedSecureField::zero, |acc: PackedSecureField, i: usize| {
                acc + (weight_at(i) * evals.values.data[i])
            })
            .sum::<PackedSecureField>()
            .to_array()
            .into_par_iter()
            .sum::<SecureField>();
    }

    fn barycentric_eval_group(
        evals: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        weights: &SecureColumnByCoords<Self>,
    ) -> Vec<SecureField> {
        if evals.is_empty() {
            return Vec::new();
        }
        // One pass over the weights for all the columns, as in `subdomain_eval_group`.
        let n_words = weights.len().div_ceil(N_LANES);
        let zero = || vec![PackedSecureField::zero(); evals.len()];
        let fold = |mut accumulators: Vec<PackedSecureField>, chunk_index: usize| {
            let start = chunk_index * 256;
            let chunk_len = 256.min(n_words - start);
            let chunk_weights = (start..start + chunk_len)
                .map(|i| {
                    PackedSecureField::from_packed_m31s(std::array::from_fn(|j| {
                        weights.columns[j].data[i]
                    }))
                })
                .collect::<Vec<_>>();
            for (accumulator, eval) in accumulators.iter_mut().zip(evals) {
                let values = &eval.values.data[start..start + chunk_len];
                *accumulator += zip(&chunk_weights, values)
                    .fold(PackedSecureField::zero(), |sum, (weight, value)| sum + *weight * *value);
            }
            accumulators
        };
        #[cfg(not(feature = "parallel"))]
        let sums = (0..n_words.div_ceil(256)).fold(zero(), fold);
        #[cfg(feature = "parallel")]
        let sums = (0..n_words.div_ceil(256))
            .into_par_iter()
            .fold(zero, fold)
            .reduce(zero, |a, b| zip(a, b).map(|(x, y)| x + y).collect());
        sums.into_iter().map(|sum| sum.pointwise_sum()).collect()
    }

    fn subdomain_eval_group(
        coset: CanonicCoset,
        log_blowup: u32,
        p: CirclePoint<SecureField>,
        evals: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
    ) -> Vec<SecureField> {
        if evals.is_empty() {
            return Vec::new();
        }
        let log_size = Self::barycentric_log_size(coset.log_size(), log_blowup);
        let len = 1usize << log_size;
        if len < N_LANES || log_size == coset.log_size() {
            // Keep the existing full-domain path, including zeroed padding for tiny domains.
            let buffer = SecureColumnByCoords::<Self>::zeros(len);
            let weights = Self::barycentric_weights_into(coset, p, buffer);
            return evals
                .iter()
                .map(|eval| Self::barycentric_eval_at_point(eval, &weights))
                .collect();
        }

        let setup = twin_coset_weight_setup(coset, p, len);
        let n_words = len / N_LANES;
        let zero = || vec![PackedSecureField::zero(); evals.len()];
        let fold = |mut accumulators: Vec<PackedSecureField>, chunk_index: usize| {
            let start = chunk_index * 256;
            let chunk_len = 256.min(n_words - start);
            let weights = twin_coset_weight_chunk(&setup, chunk_index, chunk_len);
            for (accumulator, eval) in accumulators.iter_mut().zip(evals) {
                let values = &eval.values.data[start..start + chunk_len];
                *accumulator += zip(&weights, values)
                    .fold(PackedSecureField::zero(), |sum, (weight, value)| sum + *weight * *value);
            }
            accumulators
        };
        #[cfg(not(feature = "parallel"))]
        let sums = (0..n_words.div_ceil(256)).fold(zero(), fold);
        #[cfg(feature = "parallel")]
        let sums = (0..n_words.div_ceil(256))
            .into_par_iter()
            .fold(zero, fold)
            .reduce(zero, |a, b| zip(a, b).map(|(x, y)| x + y).collect());
        sums.into_iter().map(|sum| sum.pointwise_sum()).collect()
    }

    fn eval_at_point_by_folding(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        point: CirclePoint<SecureField>,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureField {
        let log_size = evals.domain.log_size();
        let mut folding_alphas = get_folding_alphas(point, log_size as usize);

        let mut layer_evaluation =
            fold_circle_evaluation_into_line(evals, folding_alphas.pop().unwrap(), twiddles);

        while layer_evaluation.len() > 1 {
            let alpha = folding_alphas.pop().unwrap();
            layer_evaluation = SimdBackend::fold_line(&layer_evaluation, &[alpha], twiddles);
        }

        layer_evaluation.values.at(0) / SecureField::from(2_u32.pow(log_size))
    }

    fn extend(poly: &CircleCoefficients<Self>, log_size: u32) -> CircleCoefficients<Self> {
        // TODO(shahars): Get rid of extends.
        poly.evaluate(CanonicCoset::new(log_size).circle_domain()).interpolate()
    }

    fn evaluate(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder> {
        // SAFETY: evaluate_into writes all values via FFT before they are read.
        let buffer = unsafe { Col::<Self, BaseField>::uninitialized(domain.size()) };
        Self::evaluate_into(poly, domain, twiddles, buffer)
    }

    fn evaluate_into(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
        mut buffer: Col<Self, BaseField>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder> {
        let _span = span!(Level::TRACE, "", class = "rFFT").entered();
        let log_size = domain.log_size();
        let fft_log_size = poly.log_size();
        assert!(log_size >= fft_log_size, "Can only evaluate on larger domains");
        assert_eq!(buffer.len(), domain.size());

        if fft_log_size < MIN_FFT_LOG_SIZE {
            let cpu_poly: CircleCoefficients<CpuBackend> =
                CircleCoefficients::new(poly.coeffs.to_cpu());
            let cpu_eval = cpu_poly.evaluate(domain);
            return CircleEvaluation::new(
                cpu_eval.domain,
                Col::<SimdBackend, BaseField>::from_iter(cpu_eval.values),
            );
        }

        let twiddles = domain_line_twiddles_from_tree(domain, &twiddles.twiddles);

        // Evaluate on big domains by evaluating on several subdomains.
        let log_subdomains = log_size - fft_log_size;

        // The twiddles of each subdomain are a slice of the large domain twiddles.
        let subdomain_twiddles = (0..(1usize << log_subdomains))
            .map(|i| {
                (0..(fft_log_size - 1))
                    .map(|layer_i| {
                        &twiddles[layer_i as usize][i << (fft_log_size - 2 - layer_i)
                            ..(i + 1) << (fft_log_size - 2 - layer_i)]
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // Map the pages of the destination in one call before the FFT's stores fault them in one
        // by one (a hint: no-op where the kernel does not support it).
        advise_pages(&buffer.data, true);

        // FFT from the coefficients buffer directly into the provided buffer.
        fft_subdomains_into(poly, &mut buffer, &subdomain_twiddles);

        CircleEvaluation::new(domain, buffer)
    }

    const STRIPES: bool = true;

    fn evaluate_from_prefix(
        prefix: &Col<Self, BaseField>,
        log_size: u32,
        domain: CircleDomain,
        targets: &[(usize, Vec<usize>)],
        scratch: &mut Col<Self, BaseField>,
    ) -> Option<Vec<Vec<BaseField>>> {
        evaluate_from_prefix_simd(prefix, log_size, domain, targets, scratch)
    }

    fn evaluate_stripe_into(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
        stripe: usize,
        dst: &mut Col<Self, BaseField>,
    ) {
        let fft_log_size = poly.log_size();
        let len = 1usize << fft_log_size;
        assert!(domain.log_size() >= fft_log_size);
        assert_eq!(dst.len(), len);
        if fft_log_size < MIN_FFT_LOG_SIZE {
            // Small polynomials go through the CPU path of `evaluate`, as a whole.
            let full = Self::evaluate(poly, domain, twiddles);
            Self::copy_block(&full.values, stripe * len, dst, 0, len);
            return;
        }
        // The same twiddle slice as subdomain `stripe` of `evaluate_into`.
        let line_twiddles = domain_line_twiddles_from_tree(domain, &twiddles.twiddles);
        let stripe_twiddles = vec![
            (0..(fft_log_size - 1))
                .map(|layer_i| {
                    &line_twiddles[layer_i as usize][stripe << (fft_log_size - 2 - layer_i)
                        ..(stripe + 1) << (fft_log_size - 2 - layer_i)]
                })
                .collect::<Vec<_>>(),
        ];
        fft_subdomains_into(poly, dst, &stripe_twiddles);
    }

    fn evaluate_block_into(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
        log_block: u32,
        block: usize,
        dst: &mut Col<Self, BaseField>,
    ) {
        evaluate_block_into(poly, domain, twiddles, log_block, block, dst)
    }

    fn copy_block(
        src: &Col<Self, BaseField>,
        src_start: usize,
        dst: &mut Col<Self, BaseField>,
        dst_start: usize,
        len: usize,
    ) {
        if src_start % N_LANES == 0 && dst_start % N_LANES == 0 && len % N_LANES == 0 {
            dst.data[dst_start / N_LANES..(dst_start + len) / N_LANES]
                .copy_from_slice(&src.data[src_start / N_LANES..(src_start + len) / N_LANES]);
        } else {
            for i in 0..len {
                dst.set(dst_start + i, src.at(src_start + i));
            }
        }
    }

    fn subdomain_twiddles(coset: Coset) -> std::sync::Arc<TwiddleTree<Self>> {
        let key = (coset.initial_index.0, coset.log_size);
        if let Some(twiddles) = SUBDOMAIN_TWIDDLES.lock().unwrap().get(&key) {
            return twiddles.clone();
        }
        let twiddles = std::sync::Arc::new(Self::precompute_twiddles(coset));
        SUBDOMAIN_TWIDDLES.lock().unwrap().insert(key, twiddles.clone());
        twiddles
    }

    /// Precomputes the (doubled) twiddles for a given coset tower.
    /// The twiddles are the x values of each coset in bit-reversed order.
    /// Note: the coset point are symmetrical over the x-axis so only the first half of the coset is
    /// needed.
    fn precompute_twiddles(mut coset: Coset) -> TwiddleTree<Self> {
        let _span = span!(Level::TRACE, "", class = "PrecomputeTwiddles").entered();
        let root_coset = coset;

        if root_coset.size() < N_LANES {
            return compute_small_coset_twiddles(root_coset);
        }

        let mut twiddles = Vec::with_capacity(coset.size() / N_LANES);
        while coset.log_size() > LOG_N_LANES {
            compute_coset_twiddles(coset, &mut twiddles);
            coset = coset.double();
        }

        // Handle cosets smaller than `N_LANES`.
        let remaining_twiddles = slow_precompute_twiddles(coset);

        twiddles.push(PackedM31::from_array(remaining_twiddles.try_into().unwrap()));

        let itwiddles = PackedBaseField::batch_inverse(&twiddles);

        let dbl_twiddles = twiddles
            .into_iter()
            .flat_map(|x| (x.into_simd() * Simd::splat(2)).to_array())
            .collect();
        let dbl_itwiddles = itwiddles
            .into_iter()
            .flat_map(|x| (x.into_simd() * Simd::splat(2)).to_array())
            .collect();

        TwiddleTree { root_coset, twiddles: dbl_twiddles, itwiddles: dbl_itwiddles }
    }

    fn split_at_mid(
        mut poly: CircleCoefficients<Self>,
    ) -> (CircleCoefficients<Self>, CircleCoefficients<Self>) {
        let length = poly.coeffs.length;

        // If the length fits only in one SIMD vector, need to split from the cpu vector.
        if length <= 1 << LOG_N_LANES {
            let mut cpu_vec = poly.coeffs.to_cpu();
            let right = cpu_vec.split_off(cpu_vec.len() / 2);
            return (
                CircleCoefficients::new(cpu_vec.into_iter().collect()),
                CircleCoefficients::new(right.into_iter().collect()),
            );
        }

        let log_length = length.ilog2();
        let log_n_vecs = log_length - LOG_N_LANES;

        // When the poly is large, IFFT doesn't end with a transpose, so we need to transpose the
        // coefficients before splitting.
        if log_length > CACHED_FFT_LOG_SIZE {
            unsafe {
                transpose_vecs(
                    transmute::<*mut PackedBaseField, *mut u32>(poly.coeffs.data.as_mut_ptr()),
                    log_n_vecs as usize,
                );
            }
        }

        let mut second = poly.coeffs.data.split_off(poly.coeffs.data.len() / 2);

        // If the new polynomials are large, we need to transpose the coefficients back before
        // returning because the FFT algorithm assumes the coefficients are transposed.
        if log_length - 1 > CACHED_FFT_LOG_SIZE {
            // transpose first and second
            unsafe {
                transpose_vecs(
                    transmute::<*mut PackedBaseField, *mut u32>(poly.coeffs.data.as_mut_ptr()),
                    (log_n_vecs - 1) as usize,
                );
                transpose_vecs(
                    transmute::<*mut PackedBaseField, *mut u32>(second.as_mut_ptr()),
                    (log_n_vecs - 1) as usize,
                );
            }
        }

        let left_length = length / 2;
        let right_length = length - left_length;

        (
            CircleCoefficients::new(BaseColumn { data: poly.coeffs.data, length: left_length }),
            CircleCoefficients::new(BaseColumn { data: second, length: right_length }),
        )
    }
}

/// Evaluates `poly` on `subdomain_twiddles.len()` subdomains of its size, the `i`-th written to
/// the `i`-th block of `dst`, which must hold all of them.
fn fft_subdomains_into(
    poly: &CircleCoefficients<SimdBackend>,
    dst: &mut BaseColumn,
    subdomain_twiddles: &[Vec<&[u32]>],
) {
    assert!(dst.len() >= subdomain_twiddles.len() << poly.log_size());
    unsafe {
        rfft::fft_subdomains(
            transmute::<*const PackedBaseField, *const u32>(poly.coeffs.data.as_ptr()),
            transmute::<*mut PackedBaseField, *mut u32>(dst.data.as_mut_ptr()),
            subdomain_twiddles,
            poly.log_size() as usize,
        );
    }
}

/// Where `transpose_vecs` moves the vector of index `v` of an array of `2^log_n_vecs` vectors:
/// `(a, b, c)` to `(c, b, a)`, with `|a| = |c| = log_n_vecs / 2`. The map is its own inverse.
fn transposed_vec_index(v: usize, log_n_vecs: u32) -> usize {
    let post = log_n_vecs / 2;
    let n_b = log_n_vecs & 1;
    let a = v >> (post + n_b);
    let b = (v >> post) & ((1 << n_b) - 1);
    let c = v & ((1 << post) - 1);
    (c << (post + n_b)) | (b << post) | a
}

/// Writes the evaluation of `poly` on block `block` of `domain`'s bit-reversed order, of
/// `2^log_block` rows, into `dst`: the evaluation on a subdomain of that size. A block smaller
/// than the polynomial folds the coefficients first, one top layer of the FFT at a time
/// (`lo + t * hi` or `lo - t * hi`, `t` the layer's one twiddle for the block), and evaluates the
/// folded coefficients with an FFT of the block's size; a block of the polynomial's size is
/// [`PolyOps::evaluate_stripe_into`].
pub fn evaluate_block_into(
    poly: &CircleCoefficients<SimdBackend>,
    domain: CircleDomain,
    twiddles: &TwiddleTree<SimdBackend>,
    log_block: u32,
    block: usize,
    dst: &mut BaseColumn,
) {
    let log_size = poly.log_size();
    assert!(log_block <= log_size && log_block >= MIN_FFT_LOG_SIZE && log_block < domain.log_size());
    assert_eq!(dst.len(), 1 << log_block);
    if log_block == log_size {
        return SimdBackend::evaluate_stripe_into(poly, domain, twiddles, block, dst);
    }
    let line_twiddles = domain_line_twiddles_from_tree(domain, &twiddles.twiddles);
    // Fold from the polynomial's size down to the block's.
    let mut folded: Option<BaseColumn> = None;
    for level in (log_block + 1..=log_size).rev() {
        let stripe = block >> (level - log_block);
        let sign = (block >> (level - 1 - log_block)) & 1;
        let t = BaseField::from_u32_unchecked(line_twiddles[(level - 2) as usize][stripe] / 2);
        let t = PackedBaseField::broadcast(if sign == 0 { t } else { -t });
        let half_vecs = 1usize << (level - 1 - LOG_N_LANES);
        match &mut folded {
            None => {
                // Large coefficients are stored transposed (see `split_at_mid`): read the vector of
                // natural index `v` from where the transposition put it.
                let src = &poly.coeffs.data;
                let log_n_vecs = log_size - LOG_N_LANES;
                let at = |v: usize| {
                    if log_size <= CACHED_FFT_LOG_SIZE {
                        return src[v];
                    }
                    src[transposed_vec_index(v, log_n_vecs)]
                };
                let mut out = BaseColumn::zeros(1 << (level - 1));
                out.data
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(i, x)| *x = at(i) + at(i + half_vecs) * t);
                folded = Some(out);
            }
            Some(buf) => {
                let (lo, hi) = buf.data[..2 * half_vecs].split_at_mut(half_vecs);
                lo.par_iter_mut().zip(hi.par_iter()).for_each(|(l, h)| *l = *l + *h * t);
                buf.data.truncate(half_vecs);
                buf.length = 1 << (level - 1);
            }
        }
    }
    let mut folded = folded.unwrap();
    if log_block > CACHED_FFT_LOG_SIZE {
        // The FFT reads large coefficients transposed.
        let natural = folded;
        let log_n_vecs = log_block - LOG_N_LANES;
        folded = BaseColumn::zeros(1 << log_block);
        folded
            .data
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = natural.data[transposed_vec_index(i, log_n_vecs)]);
    }
    let folded = CircleCoefficients::<SimdBackend>::new(folded);
    let stripe_twiddles = vec![
        (0..(log_block - 1))
            .map(|layer_i| {
                &line_twiddles[layer_i as usize][block << (log_block - 2 - layer_i)
                    ..(block + 1) << (log_block - 2 - layer_i)]
            })
            .collect::<Vec<_>>(),
    ];
    fft_subdomains_into(&folded, dst, &stripe_twiddles);
}

fn compute_small_coset_twiddles(coset: Coset) -> TwiddleTree<SimdBackend> {
    let twiddles = slow_precompute_twiddles(coset);

    let dbl_twiddles = twiddles.iter().map(|x| x.0 * 2).collect();
    let dbl_itwiddles = twiddles.iter().map(|x| x.inverse().0 * 2).collect();
    TwiddleTree { root_coset: coset, twiddles: dbl_twiddles, itwiddles: dbl_itwiddles }
}

/// Computes the twiddles of the coset in bit-reversed order. Optimized for SIMD.
fn compute_coset_twiddles(coset: Coset, twiddles: &mut Vec<PackedM31>) {
    let log_size = coset.log_size() - 1;
    assert!(log_size >= LOG_N_LANES);

    // Compute the first `N_LANES` circle points.
    let initial_points = std::array::from_fn(|i| coset.at(bit_reverse_index(i, log_size)));
    let mut current = CirclePoint {
        x: PackedM31::from_array(initial_points.each_ref().map(|p| p.x)),
        y: PackedM31::from_array(initial_points.each_ref().map(|p| p.y)),
    };

    // Precompute the steps needed to compute the next circle points in bit reversed order.
    let mut steps = [CirclePoint::zero(); (M31_CIRCLE_LOG_ORDER - LOG_N_LANES) as usize];
    for i in 0..(log_size - LOG_N_LANES) {
        let prev_mul = bit_reverse_index((1 << i) - 1, log_size - LOG_N_LANES);
        let new_mul = bit_reverse_index(1 << i, log_size - LOG_N_LANES);
        let step = coset.step.mul(new_mul as u128) - coset.step.mul(prev_mul as u128);
        steps[i as usize] = step;
    }

    for i in 0u32..1 << (log_size - LOG_N_LANES) {
        // Extract twiddle and compute the next `N_LANES` circle points.
        let x = current.x;
        let step_index = i.trailing_ones() as usize;
        let step = CirclePoint {
            x: PackedM31::broadcast(steps[step_index].x),
            y: PackedM31::broadcast(steps[step_index].y),
        };
        current = current + step;
        twiddles.push(x);
    }
}

fn slow_eval_at_point(
    poly: &CircleCoefficients<SimdBackend>,
    point: CirclePoint<SecureField>,
) -> SecureField {
    let mut mappings = vec![point.y, point.x];
    let mut x = point.x;
    for _ in 2..poly.log_size() {
        x = CirclePoint::double_x(x);
        mappings.push(x);
    }
    mappings.reverse();

    // If the polynomial is large, the fft does a transpose in the middle.
    if poly.log_size() > CACHED_FFT_LOG_SIZE {
        let n = mappings.len();
        let n0 = (n - LOG_N_LANES as usize) / 2;
        let n1 = (n - LOG_N_LANES as usize).div_ceil(2);
        let (ab, c) = mappings.split_at_mut(n1);
        let (a, _b) = ab.split_at_mut(n0);
        // Swap content of a,c.
        a.swap_with_slice(&mut c[0..n0]);
    }
    fold(poly.coeffs.as_slice(), &mappings)
}

/// [`PolyOps::evaluate_from_prefix`] for the SIMD backend. Folding a column's evaluation on its
/// first subdomain with a point's factors (y, x, pi(x), ...) evaluates it at that point
/// (`CpuBackend::eval_at_point_by_folding`). The 32 points of an aligned block of a stripe share
/// every factor past the first five, so:
/// 1. the first five layers (circle and four line layers) are the first five layers of the
///    inverse FFT, run unfolded once per column (`ifft_lower_with_vecwise`): chunk `c` of 32
///    values then holds the 32 partial polynomials at the `c`-th point of the folded domain;
/// 2. the remaining layers fold pairs of adjacent chunks with each block's shared factor, tile
///    by tile so that the column is read once for all blocks;
/// 3. each point combines its block's 32 values with its own five factors.
/// Everything but the column's values depends only on its size and the rows read, so it is
/// planned once per size ([`PrefixPlan`]) and shared by every column of that size.
fn evaluate_from_prefix_simd(
    prefix: &BaseColumn,
    log_size: u32,
    domain: CircleDomain,
    targets: &[(usize, Vec<usize>)],
    scratch: &mut BaseColumn,
) -> Option<Vec<Vec<BaseField>>> {
    evaluate_from_prefix_mode(prefix, log_size, domain, targets, scratch, bary_tail())
}

fn evaluate_from_prefix_mode(
    prefix: &BaseColumn,
    log_size: u32,
    domain: CircleDomain,
    targets: &[(usize, Vec<usize>)],
    scratch: &mut BaseColumn,
    bary: bool,
) -> Option<Vec<Vec<BaseField>>> {
    if log_size < 10 {
        return None;
    }
    let n = log_size;
    let plan = PrefixPlan::get(n, domain, targets);
    // 1. The first five inverse layers, chunk by chunk, in the caller's (pooled) scratch.
    let n_vecs = 1usize << (n - LOG_N_LANES);
    let unfolded = &mut scratch.data[..n_vecs];
    unfolded.copy_from_slice(&prefix.data[..n_vecs]);
    // The loop body of `ifft::ifft_lower_with_vecwise` for its first five layers.
    let twiddles = plan.twiddles();
    for (index, chunk) in unfolded.chunks_exact_mut(2).enumerate() {
        let (val0, val1) = ifft::vecwise_ibutterflies(
            chunk[0],
            chunk[1],
            std::array::from_fn(|i| twiddles[0][index * 8 + i]),
            std::array::from_fn(|i| twiddles[1][index * 4 + i]),
            std::array::from_fn(|i| twiddles[2][index * 2 + i]),
        );
        let (val0, val1) =
            ifft::simd_ibutterfly(val0, val1, std::simd::u32x16::splat(twiddles[3][index]));
        chunk[0] = val0;
        chunk[1] = val1;
    }
    // 2. The remaining layers, tile by tile, for every block.
    let n_chunks = 1usize << (n - PrefixPlan::LOW);
    let tile = 1usize << plan.tile_bits;
    if bary {
        // Q722: one weighted accumulate per block, tile by tile so the column is read once.
        let mut acc = vec![[PackedBaseField::zero(); 2]; plan.blocks.len()];
        for t in 0..n_chunks >> plan.tile_bits {
            let src = &unfolded[2 * t * tile..2 * (t + 1) * tile];
            for (block, acc) in plan.blocks.iter().zip(acc.iter_mut()) {
                let [mut s0, mut s1] = *acc;
                for (j, &w) in block.weights[t * tile..(t + 1) * tile].iter().enumerate() {
                    let w = PackedBaseField::broadcast(w);
                    s0 += src[2 * j] * w;
                    s1 += src[2 * j + 1] * w;
                }
                *acc = [s0, s1];
            }
        }
        let mut out: Vec<Vec<BaseField>> =
            targets.iter().map(|(_, rows)| vec![BaseField::zero(); rows.len()]).collect();
        for (block, acc) in plan.blocks.iter().zip(acc) {
            let values: Vec<BaseField> = acc.iter().flat_map(|p| p.to_array()).collect();
            for (k, own) in block.rows.clone().zip(&block.own) {
                out[block.target][k] = fold(&values, own) * plan.scale;
            }
        }
        return Some(out);
    }
    let mut level: Vec<Vec<[PackedBaseField; 2]>> =
        plan.blocks.iter().map(|_| Vec::with_capacity(n_chunks >> plan.tile_bits)).collect();
    let mut cur = vec![[PackedBaseField::zero(); 2]; tile / 2];
    for t in 0..n_chunks >> plan.tile_bits {
        let src = &unfolded[2 * t * tile..2 * (t + 1) * tile];
        for (block, level) in plan.blocks.iter().zip(level.iter_mut()) {
            // The first layer reads the unfolded chunks in place.
            let coeffs = &block.coeffs[0][t * tile / 2..(t + 1) * tile / 2];
            for (i, (out, &c)) in cur.iter_mut().zip(coeffs).enumerate() {
                let c = PackedBaseField::broadcast(c);
                let (a0, a1, b0, b1) = (src[4 * i], src[4 * i + 1], src[4 * i + 2], src[4 * i + 3]);
                *out = [(a0 + b0) + (a0 - b0) * c, (a1 + b1) + (a1 - b1) * c];
            }
            let mut len = tile / 2;
            for layer in 1..plan.tile_bits as usize {
                let base = (t * tile) >> (layer + 1);
                let coeffs = &block.coeffs[layer][base..base + len / 2];
                for i in 0..len / 2 {
                    let c = PackedBaseField::broadcast(coeffs[i]);
                    let [a0, a1] = cur[2 * i];
                    let [b0, b1] = cur[2 * i + 1];
                    cur[i] = [(a0 + b0) + (a0 - b0) * c, (a1 + b1) + (a1 - b1) * c];
                }
                len /= 2;
            }
            level.push(cur[0]);
        }
    }
    // 3. The layers above the tiles, then each point's own factors.
    let mut out: Vec<Vec<BaseField>> =
        targets.iter().map(|(_, rows)| vec![BaseField::zero(); rows.len()]).collect();
    for (block, mut level) in plan.blocks.iter().zip(level) {
        let mut len = level.len();
        for layer in plan.tile_bits as usize..(n - PrefixPlan::LOW) as usize {
            for i in 0..len / 2 {
                let c = PackedBaseField::broadcast(block.coeffs[layer][i]);
                let [a0, a1] = level[2 * i];
                let [b0, b1] = level[2 * i + 1];
                level[i] = [(a0 + b0) + (a0 - b0) * c, (a1 + b1) + (a1 - b1) * c];
            }
            len /= 2;
        }
        let values: Vec<BaseField> = level[0].iter().flat_map(|p| p.to_array()).collect();
        for (k, own) in block.rows.clone().zip(&block.own) {
            out[block.target][k] = fold(&values, own) * plan.scale;
        }
    }
    Some(out)
}

/// The column-independent part of [`evaluate_from_prefix_simd`] for one column size and set of
/// rows: the subdomain's inverse twiddles and, per block, the shared factor times the twiddle of
/// every pair of every folded layer, and each point's own five factors.
type SubdomainTwiddles =
    std::collections::HashMap<(usize, u32), std::sync::Arc<TwiddleTree<SimdBackend>>>;
/// The twiddles of the subdomains striped trees regrow or decommit on.
static SUBDOMAIN_TWIDDLES: std::sync::LazyLock<std::sync::Mutex<SubdomainTwiddles>> =
    std::sync::LazyLock::new(Default::default);
/// The decommit plans of [`PrefixPlan::get`], by column size, domain and rows.
static PREFIX_PLANS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<u64, std::sync::Arc<PrefixPlan>>>,
> = std::sync::LazyLock::new(Default::default);

/// Drops the striped trees' cached subdomain twiddles and decommit plans: called when a proof's
/// decommitment is done, so that one leg's caches do not stay resident under the next leg.
pub fn clear_stripe_caches() {
    SUBDOMAIN_TWIDDLES.lock().unwrap().clear();
    PREFIX_PLANS.lock().unwrap().clear();
}

struct PrefixPlan {
    sub_twiddles: std::sync::Arc<TwiddleTree<SimdBackend>>,
    subdomain: CircleDomain,
    tile_bits: u32,
    blocks: Vec<PrefixBlock>,
    scale: BaseField,
}

struct PrefixBlock {
    target: usize,
    rows: std::ops::Range<usize>,
    /// `coeffs[layer][pair]`: the block's factor of folded layer `layer` (line layer 5 + layer)
    /// times that layer's inverse twiddle of pair `pair`.
    coeffs: Vec<Vec<BaseField>>,
    /// Each point's own factors, highest first (`pi^3(x), ..., x, y`), as `fold` takes them.
    own: Vec<[BaseField; 5]>,
    /// Q722: the folded layers as one weighted sum. A fold `(a + b) + (a - b) c` is
    /// `a (1 + c) + b (1 - c)`, so the top value is `sum_i weights[i] * chunk_i`, the weight of
    /// chunk `i` being the product of its `(1 +- c)` along its path. Exact: the same field
    /// operations, reassociated. Empty unless the bary tail is on.
    weights: Vec<BaseField>,
}

/// Whether the decommit evaluates the folded layers as one weighted sum per block (Q722).
fn bary_tail() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| true);
    *ON
}

/// The weight of every chunk under the fold coefficients `coeffs[layer][pair]` (layer 0 pairs
/// chunks), expanded top-down from the single top value.
fn fold_weights(coeffs: &[Vec<BaseField>]) -> Vec<BaseField> {
    let one = BaseField::from(1u32);
    let mut w = vec![one];
    for layer in coeffs.iter().rev() {
        debug_assert_eq!(layer.len(), w.len());
        w = w
            .iter()
            .zip(layer)
            .flat_map(|(&w, &c)| [w * (one + c), w * (one - c)])
            .collect();
    }
    w
}

impl PrefixPlan {
    const LOW: u32 = 5;

    fn twiddles(&self) -> Vec<&[u32]> {
        domain_line_twiddles_from_tree(self.subdomain, &self.sub_twiddles.itwiddles)
    }

    fn get(n: u32, domain: CircleDomain, targets: &[(usize, Vec<usize>)]) -> std::sync::Arc<Self> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (n, domain.log_size(), domain.half_coset.initial_index.0, targets).hash(&mut hasher);
        let key = hasher.finish();
        if let Some(plan) = PREFIX_PLANS.lock().unwrap().get(&key) {
            return plan.clone();
        }
        // Planned outside the lock: planning may run on the thread pool, whose other workers may
        // be waiting for this lock (a blocking lock held across a join deadlocks).
        let plan = std::sync::Arc::new(Self::new(n, domain, targets));
        PREFIX_PLANS.lock().unwrap().entry(key).or_insert(plan).clone()
    }

    fn new(n: u32, domain: CircleDomain, targets: &[(usize, Vec<usize>)]) -> Self {
        let low = Self::LOW;
        let subdomain = domain.split(domain.log_size() - n).0;
        let sub_twiddles = SimdBackend::subdomain_twiddles(subdomain.half_coset);
        let twiddles = domain_line_twiddles_from_tree(subdomain, &sub_twiddles.itwiddles);
        let mut blocks = vec![];
        for (target, (stripe, rows)) in targets.iter().enumerate() {
            let mut start = 0;
            for block in rows.chunk_by(|a, b| a >> low == b >> low) {
                let factors = block
                    .iter()
                    .map(|&row| {
                        let point =
                            domain.at(bit_reverse_index((stripe << n) + row, domain.log_size()));
                        let mut x = point.x;
                        let mut factors = vec![point.y];
                        for _ in 1..n {
                            factors.push(x);
                            x = CirclePoint::double_x(x);
                        }
                        factors
                    })
                    .collect::<Vec<_>>();
                let coeffs: Vec<Vec<BaseField>> = (0..(n - low) as usize)
                    .map(|layer| {
                        let alpha = factors[0][low as usize + layer];
                        twiddles[low as usize - 1 + layer]
                            .iter()
                            .map(|&dbl| alpha * BaseField::from_u32_unchecked(dbl >> 1))
                            .collect()
                    })
                    .collect();
                let weights = if bary_tail() { fold_weights(&coeffs) } else { vec![] };
                blocks.push(PrefixBlock {
                    target,
                    rows: start..start + block.len(),
                    weights,
                    coeffs,
                    own: factors.iter().map(|f| [f[4], f[3], f[2], f[1], f[0]]).collect(),
                });
                start += block.len();
            }
        }
        drop(twiddles);
        Self {
            sub_twiddles,
            subdomain,
            tile_bits: 6.min(n - low),
            blocks,
            scale: BaseField::from(1u32 << n).inverse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use crate::core::circle::CirclePoint;
    use crate::core::fields::m31::BaseField;
    use crate::core::poly::circle::CanonicCoset;
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::simd::circle::slow_eval_at_point;
    use crate::prover::backend::simd::column::BaseColumn;
    use crate::prover::backend::simd::fft::{CACHED_FFT_LOG_SIZE, MIN_FFT_LOG_SIZE};
    use crate::prover::backend::simd::m31::LOG_N_LANES;
    use crate::prover::backend::{Column, CpuBackend};
    use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
    use crate::prover::poly::{BitReversedOrder, NaturalOrder};

    #[test]
    fn test_interpolate_and_eval() {
        for log_size in MIN_FFT_LOG_SIZE..CACHED_FFT_LOG_SIZE + 4 {
            let domain = CanonicCoset::new(log_size).circle_domain();
            let evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                domain,
                (0..1 << log_size).map(BaseField::from).collect(),
            );

            let poly = evaluation.clone().interpolate();
            let evaluation2 = poly.evaluate(domain);

            assert_eq!(evaluation.values.to_cpu(), evaluation2.values.to_cpu());
        }
    }

    #[test]
    fn test_eval_extension() {
        for log_size in MIN_FFT_LOG_SIZE..CACHED_FFT_LOG_SIZE + 2 {
            let domain = CanonicCoset::new(log_size).circle_domain();
            let domain_ext = CanonicCoset::new(log_size + 2).circle_domain();
            let evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                domain,
                (0..1 << log_size).map(BaseField::from).collect(),
            );
            let poly = evaluation.clone().interpolate();

            let evaluation2 = poly.evaluate(domain_ext);

            assert_eq!(
                poly.extend(log_size + 2).coeffs.to_cpu(),
                evaluation2.interpolate().coeffs.to_cpu()
            );
        }
    }

    #[test]
    fn test_eval_at_point() {
        for log_size in MIN_FFT_LOG_SIZE + 1..CACHED_FFT_LOG_SIZE + 4 {
            let domain = CanonicCoset::new(log_size).circle_domain();
            let evaluation = CircleEvaluation::<SimdBackend, BaseField, NaturalOrder>::new(
                domain,
                (0..1 << log_size).map(BaseField::from).collect(),
            );
            let poly = evaluation.bit_reverse().interpolate();
            for i in [0, 1, 3, 1 << (log_size - 1), 1 << (log_size - 2)] {
                let p = domain.at(i);

                let eval = poly.eval_at_point(p.into_ef());

                assert_eq!(eval, BaseField::from(i).into(), "log_size={log_size}, i={i}");
            }
        }
    }

    #[test]
    fn test_simd_eval_at_point_by_folding() {
        let poly = CircleCoefficients::<SimdBackend>::new(BaseColumn::from_cpu(
            &[691, 805673, 5, 435684, 4832, 23876431, 197, 897346068].map(BaseField::from),
        ));
        let s = CanonicCoset::new(10);
        let domain = s.circle_domain();
        let eval = poly.evaluate(domain);
        let twiddles =
            SimdBackend::precompute_twiddles(CanonicCoset::new(11).circle_domain().half_coset);
        let sampled_points = [
            CirclePoint::get_point(348),
            CirclePoint::get_point(9736524),
            CirclePoint::get_point(13),
            CirclePoint::get_point(346752),
        ];
        let sampled_values =
            sampled_points.iter().map(|point| poly.eval_at_point(*point)).collect_vec();

        let sampled_folding_values = sampled_points
            .iter()
            .map(|point| eval.eval_at_point_by_folding(*point, &twiddles))
            .collect_vec();

        assert_eq!(
            sampled_folding_values, sampled_values,
            "Evaluation by folding should be equal to the polynomial evaluation"
        );
    }

    #[test]
    fn test_circle_poly_extend() {
        for log_size in MIN_FFT_LOG_SIZE..CACHED_FFT_LOG_SIZE + 2 {
            let poly = CircleCoefficients::<SimdBackend>::new(
                (0..1 << log_size).map(BaseField::from).collect(),
            );
            let eval0 = poly.evaluate(CanonicCoset::new(log_size + 2).circle_domain());

            let eval1 =
                poly.extend(log_size + 2).evaluate(CanonicCoset::new(log_size + 2).circle_domain());

            assert_eq!(eval0.values.to_cpu(), eval1.values.to_cpu());
        }
    }

    #[test]
    fn test_eval_securefield() {
        let mut rng = SmallRng::seed_from_u64(0);
        for log_size in MIN_FFT_LOG_SIZE..CACHED_FFT_LOG_SIZE + 2 {
            let domain = CanonicCoset::new(log_size).circle_domain();
            let evaluation = CircleEvaluation::<SimdBackend, BaseField, NaturalOrder>::new(
                domain,
                (0..1 << log_size).map(BaseField::from).collect(),
            );
            let poly = evaluation.bit_reverse().interpolate();
            let x = rng.random();
            let y = rng.random();
            let p = CirclePoint { x, y };

            let eval = PolyOps::eval_at_point(&poly, p);

            assert_eq!(eval, slow_eval_at_point(&poly, p), "log_size = {log_size}");
        }
    }

    #[test]
    fn test_optimized_precompute_twiddles() {
        let coset = CanonicCoset::new(10).half_coset();
        let twiddles = SimdBackend::precompute_twiddles(coset);
        let expected_twiddles = CpuBackend::precompute_twiddles(coset);

        assert_eq!(
            twiddles.twiddles,
            expected_twiddles.twiddles.iter().map(|x| x.0 * 2).collect_vec()
        );
    }
    #[test]
    fn test_circle_poly_split_at_mid_small() {
        let log_size = LOG_N_LANES;
        let poly = CircleCoefficients::<SimdBackend>::new(
            (0..1 << log_size).map(BaseField::from).collect(),
        );
        let (left, right) = poly.clone().split_at_mid();
        let random_point = CirclePoint::get_point(21903);

        assert_eq!(
            left.eval_at_point(random_point)
                + random_point.repeated_double(log_size - 2).x * right.eval_at_point(random_point),
            poly.eval_at_point(random_point)
        );
    }

    #[test]
    fn test_circle_poly_split_at_mid_medium() {
        let log_size = (CACHED_FFT_LOG_SIZE - LOG_N_LANES) / 2;
        let poly = CircleCoefficients::<SimdBackend>::new(
            (0..1 << log_size).map(BaseField::from).collect(),
        );
        let (left, right) = poly.clone().split_at_mid();
        let random_point = CirclePoint::get_point(21903);

        assert_eq!(
            left.eval_at_point(random_point)
                + random_point.repeated_double(log_size - 2).x * right.eval_at_point(random_point),
            poly.eval_at_point(random_point)
        );
    }

    #[test]
    fn test_circle_poly_split_at_mid_large() {
        let log_size = CACHED_FFT_LOG_SIZE + 1;
        let poly = CircleCoefficients::<SimdBackend>::new(
            (0..1 << log_size).map(BaseField::from).collect(),
        );
        let (left, right) = poly.clone().split_at_mid();
        let random_point = CirclePoint::get_point(21903);

        assert_eq!(
            left.eval_at_point(random_point)
                + random_point.repeated_double(log_size - 2).x * right.eval_at_point(random_point),
            poly.eval_at_point(random_point)
        );
    }

    #[test]
    fn test_simd_barycentric_evaluation() {
        let poly = CircleCoefficients::<SimdBackend>::new(BaseColumn::from_cpu(
            &[691, 805673, 5, 435684, 4832, 23876431, 197, 897346068].map(BaseField::from),
        ));
        let s = CanonicCoset::new(10);
        let domain = s.circle_domain();
        let eval = poly.evaluate(domain);
        let sampled_points = [
            CirclePoint::get_point(348),
            CirclePoint::get_point(9736524),
            CirclePoint::get_point(13),
            CirclePoint::get_point(346752),
        ];
        let sampled_values =
            sampled_points.iter().map(|point| poly.eval_at_point(*point)).collect_vec();

        let sampled_barycentric_values = sampled_points
            .iter()
            .map(|point| {
                eval.barycentric_eval_at_point(&CircleEvaluation::<
                    SimdBackend,
                    BaseField,
                    BitReversedOrder,
                >::barycentric_weights(s, *point))
            })
            .collect_vec();

        assert_eq!(
            sampled_barycentric_values, sampled_values,
            "Barycentric evaluation should be equal to the polynomial evaluation"
        );
    }

    #[test]
    fn test_simd_barycentric_weights() {
        let s = CanonicCoset::new(10);
        let sampled_points = [
            CirclePoint::get_point(348),
            CirclePoint::get_point(9736524),
            CirclePoint::get_point(13),
            CirclePoint::get_point(346752),
        ];

        let cpu_weights = sampled_points
            .iter()
            .map(|point| {
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::barycentric_weights(
                    s, *point,
                )
            })
            .collect_vec();
        let simd_weights = sampled_points
            .iter()
            .map(|point| {
                CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::barycentric_weights(
                    s, *point,
                )
            })
            .collect_vec();

        cpu_weights.iter().zip(simd_weights.iter()).for_each(|(cpu_weights, simd_weights)| {
            assert_eq!(cpu_weights.to_vec(), simd_weights.to_vec());
        });
    }

    #[test]
    fn test_simd_barycentric_weights_small_domain() {
        // Domains of at most `N_LANES` elements take a dedicated path, that pads the unused lanes
        // of the single packed element.
        for log_size in 1..=LOG_N_LANES {
            let s = CanonicCoset::new(log_size);
            let point = CirclePoint::get_point(9736524);

            let cpu_weights =
                CircleEvaluation::<CpuBackend, BaseField, BitReversedOrder>::barycentric_weights(
                    s, point,
                );
            let simd_weights =
                CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::barycentric_weights(
                    s, point,
                );

            assert_eq!(cpu_weights.to_vec(), simd_weights.to_vec(), "log_size = {log_size}");
        }
    }
}
