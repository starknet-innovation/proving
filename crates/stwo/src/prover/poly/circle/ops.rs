#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{CircleCoefficients, CircleEvaluation};
use crate::core::ColumnVec;
use crate::core::circle::{CirclePoint, Coset};
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::poly::circle::{CanonicCoset, CircleDomain};
use crate::prover::air::component_prover::Poly;
use crate::prover::backend::{Col, ColumnOps};
use crate::prover::mempool::BaseColumnPool;
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::twiddles::{TwiddleBuffer, TwiddleTree};
use crate::prover::secure_column::SecureColumnByCoords;

/// Operations on BaseField polynomials.
pub trait PolyOps: ColumnOps<BaseField> + ColumnOps<SecureField> + Sized {
    // TODO(alont): Use a column instead of this type.
    /// The type for precomputed twiddles.
    type Twiddles: TwiddleBuffer<BitReversedOrder>;

    /// Computes a minimal [CircleCoefficients] that evaluates to the same values as this
    /// evaluation. Used by the [`CircleEvaluation::interpolate()`] function.
    fn interpolate(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        itwiddles: &TwiddleTree<Self>,
    ) -> CircleCoefficients<Self>;

    fn interpolate_columns(
        columns: Vec<CircleEvaluation<Self, BaseField, BitReversedOrder>>,
        twiddles: &TwiddleTree<Self>,
    ) -> Vec<CircleCoefficients<Self>> {
        #[cfg(feature = "parallel")]
        let iter = columns.into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let iter = columns.into_iter();

        iter.map(|eval| eval.interpolate_with_twiddles(twiddles)).collect()
    }

    /// [`Self::interpolate`] with the buffers of `pool` where the backend supports it: the
    /// coefficients may be written to a buffer of the pool, and the evaluation buffer given back.
    fn interpolate_pooled(
        eval: CircleEvaluation<Self, BaseField, BitReversedOrder>,
        itwiddles: &TwiddleTree<Self>,
        _pool: &BaseColumnPool<Self>,
    ) -> CircleCoefficients<Self> {
        Self::interpolate(eval, itwiddles)
    }

    /// [`Self::interpolate_columns`] through [`Self::interpolate_pooled`].
    fn interpolate_columns_pooled(
        columns: Vec<CircleEvaluation<Self, BaseField, BitReversedOrder>>,
        twiddles: &TwiddleTree<Self>,
        pool: &BaseColumnPool<Self>,
    ) -> Vec<CircleCoefficients<Self>> {
        #[cfg(feature = "parallel")]
        let iter = columns.into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let iter = columns.into_iter();

        iter.map(|eval| Self::interpolate_pooled(eval, twiddles, pool)).collect()
    }

    /// Evaluates the polynomial at a single point.
    /// Used by the [`CircleCoefficients::eval_at_point()`] function.
    fn eval_at_point(
        poly: &CircleCoefficients<Self>,
        point: CirclePoint<SecureField>,
    ) -> SecureField;

    /// Log size of the barycentric weights for a domain of `log_size` with `log_blowup`.
    fn barycentric_log_size(log_size: u32, _log_blowup: u32) -> u32 {
        log_size
    }

    /// Computes the weights for Barycentric Lagrange interpolation for point `p` on `coset`,
    /// writing them into the provided buffer instead of allocating a new one. The buffer's columns
    /// must have size `coset.size()`, and are fully overwritten.
    /// `p` must not be in the domain.
    /// Used by the [`CircleEvaluation::barycentric_weights_into()`] function.
    fn barycentric_weights_into(
        coset: CanonicCoset,
        p: CirclePoint<SecureField>,
        buffer: SecureColumnByCoords<Self>,
    ) -> SecureColumnByCoords<Self>;

    /// Same as [`Self::barycentric_weights_into()`], allocating the weights column.
    /// Used by the [`CircleEvaluation::barycentric_weights()`] function.
    fn barycentric_weights(
        coset: CanonicCoset,
        p: CirclePoint<SecureField>,
    ) -> SecureColumnByCoords<Self> {
        // Safety: `barycentric_weights_into()` overwrites every element of the buffer.
        let buffer = unsafe { SecureColumnByCoords::<Self>::uninitialized(coset.size()) };
        Self::barycentric_weights_into(coset, p, buffer)
    }

    /// Evaluates a polynomial at a point using the barycentric interpolation formula,
    /// given its evaluations on a circle domain and precomputed barycentric weights for the domain
    /// at the sampled point.
    /// Used by the [`CircleEvaluation::barycentric_eval_at_point()`] function.
    fn barycentric_eval_at_point(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        weights: &SecureColumnByCoords<Self>,
    ) -> SecureField;

    /// Evaluates columns on the same canonic domain at `p`, sharing interpolation work.
    /// Each polynomial has coefficient log size at most `coset.log_size() - log_blowup`.
    /// Backends may interpolate from a sufficient bit-reversed prefix of the domain; the
    /// default implementation retains the backend's existing weight-size contract.
    /// Evaluates every column of `evals`, all on the domain `weights` were computed for (see
    /// [`CircleEvaluation::barycentric_weights_into`]), at that point. A backend may read each
    /// weight once for all the columns.
    fn barycentric_eval_group(
        evals: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        weights: &SecureColumnByCoords<Self>,
    ) -> Vec<SecureField> {
        evals.iter().map(|eval| Self::barycentric_eval_at_point(eval, weights)).collect()
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
        let buffer = SecureColumnByCoords::<Self>::zeros(1 << log_size);
        let weights = Self::barycentric_weights_into(coset, p, buffer);
        evals.iter().map(|eval| Self::barycentric_eval_at_point(eval, &weights)).collect()
    }

    /// Evaluates a polynomial, represented by it's evaluations, at a point using folding.
    /// Used by the [`CircleEvaluation::eval_at_point_by_folding()`] function.
    fn eval_at_point_by_folding(
        evals: &CircleEvaluation<Self, BaseField, BitReversedOrder>,
        point: CirclePoint<SecureField>,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureField;

    /// Extends the polynomial to a larger degree bound.
    /// Used by the [`CircleCoefficients::extend()`] function.
    fn extend(poly: &CircleCoefficients<Self>, log_size: u32) -> CircleCoefficients<Self>;

    /// Evaluates the polynomial at all points in the domain.
    /// Used by the [`CircleCoefficients::evaluate()`] function.
    fn evaluate(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder>;

    /// Whether [`Self::evaluate_stripe_into`] and [`Self::copy_block`] are implemented, so that
    /// a commitment tree can be extended and hashed one subdomain stripe at a time.
    const STRIPES: bool = false;

    /// Evaluates a column of `log_size` at the rows `targets[t].1` (sorted) of stripes
    /// `targets[t].0` of `domain`, from `prefix`, whose first `2^log_size` values are its stripe 0
    /// (the evaluation on the first subdomain), without its coefficients. `None` where the
    /// backend does not support it (the caller then regrows the coefficients).
    fn evaluate_from_prefix(
        _prefix: &Col<Self, BaseField>,
        _log_size: u32,
        _domain: CircleDomain,
        _targets: &[(usize, Vec<usize>)],
        _scratch: &mut Col<Self, BaseField>,
    ) -> Option<Vec<Vec<BaseField>>> {
        None
    }

    /// Writes the `stripe`-th contiguous block of [`Self::evaluate`]'s output on `domain`, of
    /// the size of `poly`, into `dst`: the evaluation on the `stripe`-th subdomain, which the
    /// extension computes as an independent FFT.
    fn evaluate_stripe_into(
        _poly: &CircleCoefficients<Self>,
        _domain: CircleDomain,
        _twiddles: &TwiddleTree<Self>,
        _stripe: usize,
        _dst: &mut Col<Self, BaseField>,
    ) {
        unimplemented!("striped extension is not supported by this backend")
    }

    /// Writes block `block` of [`Self::evaluate`]'s output on `domain`, of `2^log_block` rows
    /// (at most the size of `poly`), into `dst`: the evaluation on that subdomain.
    fn evaluate_block_into(
        _poly: &CircleCoefficients<Self>,
        _domain: CircleDomain,
        _twiddles: &TwiddleTree<Self>,
        _log_block: u32,
        _block: usize,
        _dst: &mut Col<Self, BaseField>,
    ) {
        unimplemented!("block extension is not supported by this backend")
    }

    /// Copies `src[src_start..src_start + len]` into `dst[dst_start..dst_start + len]`.
    fn copy_block(
        src: &Col<Self, BaseField>,
        src_start: usize,
        dst: &mut Col<Self, BaseField>,
        dst_start: usize,
        len: usize,
    ) {
        use crate::prover::backend::Column;
        for i in 0..len {
            dst.set(dst_start + i, src.at(src_start + i));
        }
    }

    /// Evaluates the polynomial at all points in the domain, writing results into the provided
    /// buffer instead of allocating a new one. The buffer must have size `domain.size()`.
    fn evaluate_into(
        poly: &CircleCoefficients<Self>,
        domain: CircleDomain,
        twiddles: &TwiddleTree<Self>,
        buffer: Col<Self, BaseField>,
    ) -> CircleEvaluation<Self, BaseField, BitReversedOrder>;

    fn evaluate_polynomials(
        polynomials: ColumnVec<CircleCoefficients<Self>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<Self>,
        store_polynomials_coefficients: bool,
        pool: &BaseColumnPool<Self>,
    ) -> Vec<Poly<Self>>
    where
        Self: crate::prover::backend::Backend,
    {
        // Pre-take all buffers from the pool before the parallel section: first the buffers of
        // the exact size, so that a column does not take, and shorten, a larger idle buffer that
        // a later column of that size could have used as it is.
        let mut buffers: Vec<_> = polynomials
            .iter()
            .map(|poly_coeffs| pool.try_take(poly_coeffs.log_size() + log_blowup_factor))
            .collect();
        for (poly_coeffs, buffer) in polynomials.iter().zip(buffers.iter_mut()) {
            if buffer.is_none() {
                *buffer = Some(pool.take_or_alloc(poly_coeffs.log_size() + log_blowup_factor));
            }
        }
        let buffers: Vec<_> = buffers.into_iter().map(Option::unwrap).collect();

        #[cfg(feature = "parallel")]
        let iter = polynomials.into_par_iter().zip(buffers.into_par_iter());
        #[cfg(not(feature = "parallel"))]
        let iter = polynomials.into_iter().zip(buffers);

        iter.map(|(poly_coeffs, buffer)| {
            let domain =
                CanonicCoset::new(poly_coeffs.log_size() + log_blowup_factor).circle_domain();
            let evals = Self::evaluate_into(&poly_coeffs, domain, twiddles, buffer);
            Poly::new(store_polynomials_coefficients.then_some(poly_coeffs), evals)
        })
        .collect()
    }

    /// Precomputes twiddles for a given coset.
    fn precompute_twiddles(coset: Coset) -> TwiddleTree<Self>;

    /// The twiddles of `coset`, which a backend may keep and share between calls (striped trees
    /// regrow many columns on the same few subdomains).
    fn subdomain_twiddles(coset: Coset) -> std::sync::Arc<TwiddleTree<Self>> {
        std::sync::Arc::new(Self::precompute_twiddles(coset))
    }

    /// Given a polynomial `p`, it outputs two polynomials `p_left`, `p_right` of half the degree,
    /// which satisfy the identity
    ///
    /// `p(z) = p_left(z) + pi^{L-2}(z.x) * p_right(z)`.
    ///
    /// where `L` is the log size of the coefficient vector and `z` is a circle point.
    /// If a polynomial is given by its vector of coefficients (in terms of the FFT basis in natural
    /// order), this decomposition corresponds exactly to dividing the coefficient vector in the
    /// middle. In fact, for `n` in `[0, 2^L)`, the basis element corresponding to the n-th
    /// coefficient is
    ///
    /// `(pi^{L-2}(x))^b_{L-1} * ... * (pi(x))^b_2 * x^b_1* y^b_0`,
    ///
    /// where `b_{L-1}, ... , b_0` is the bit decomposition of n (from most to least significant
    /// bit). Therefore, splitting the coefficient vector in the middle, corresponds to separating
    /// the ones with the MSB, b_{L-1} == 1, from the ones with the MSB, b_{L-1} == 0, meaning
    /// separating the basis elements divisible by `pi^{L-2}(x)` from those that are not.
    fn split_at_mid(
        poly: CircleCoefficients<Self>,
    ) -> (CircleCoefficients<Self>, CircleCoefficients<Self>);
}
