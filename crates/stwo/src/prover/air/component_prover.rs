use dashmap::DashMap;
use itertools::Itertools;

use crate::core::ColumnVec;
use crate::core::air::{Component, Components};
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::TreeVec;
use crate::core::poly::circle::{CanonicCoset, CircleDomain};
use crate::prover::CirclePoint;
use crate::prover::air::accumulation::{DomainEvaluationAccumulator, EvaluationMode};
use crate::prover::backend::{Backend, Col, Column};
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation, SecureCirclePoly};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::secure_column::SecureColumnByCoords;

/// Type alias for the weights hash map used in barycentric eval_at_point.
pub type WeightsHashMap<B> = DashMap<(u32, CirclePoint<SecureField>), SecureColumnByCoords<B>>;

pub trait ComponentProver<B: Backend>: Component + Sync {
    /// Evaluates the constraint quotients of the component on the evaluation domain.
    /// Accumulates quotients in `evaluation_accumulator`.
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, B>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<B>,
    );
}

/// The set of polynomials that make up the trace.
pub struct Trace<'a, B: Backend> {
    /// Polynomials for each column.
    pub polys: TreeVec<ColumnVec<&'a Poly<B>>>,
    /// The commitment scheme's twiddles, which cover every canonic domain up to the largest
    /// committed one: composition extends with them instead of precomputing per component.
    pub twiddles: Option<&'a TwiddleTree<B>>,
}

/// The values of a committed column on its trace domain (the canonic coset of the coefficients'
/// size), in bit-reversed order, reproduced on demand from the data the column was generated
/// from. They determine the column's polynomial exactly, as stripe 0 of its extension does, so a
/// striped column that has a source keeps no stripe at all.
pub trait TraceSource: Send + Sync {
    /// The log size of the trace domain.
    fn log_size(&self) -> u32;
    /// Writes the column's values on its trace domain, in bit-reversed order, into `dst`, of
    /// length `2^log_size`.
    fn write_values(&self, dst: &mut [BaseField]);
}

/// A shared [`TraceSource`].
pub type SharedTraceSource = std::sync::Arc<dyn TraceSource>;

/// The values of `source` on its trace domain, in bit-reversed order, as a column.
pub fn source_column<B: Backend>(source: &dyn TraceSource) -> Col<B, BaseField> {
    Col::<B, BaseField>::from_fill(1 << source.log_size(), &|dst| source.write_values(dst))
}

/// A struct for representing a polynomial corresponding to a trace column.
/// A polynomial is defined by it's evaluations on a circle domain of size at least it's degree,
/// and optionally its coefficients in the FFT basis.
///
/// A column with a [`TraceSource`] stores no evaluation values: `evals` keeps only its domain.
pub struct Poly<B: Backend> {
    pub coeffs: Option<CircleCoefficients<B>>,
    pub evals: CircleEvaluation<B, BaseField, BitReversedOrder>,
    pub source: Option<SharedTraceSource>,
}

impl<B: Backend> Poly<B> {
    pub const fn new(
        coeffs: Option<CircleCoefficients<B>>,
        evals: CircleEvaluation<B, BaseField, BitReversedOrder>,
    ) -> Self {
        Self { coeffs, evals, source: None }
    }

    /// A column that stores no evaluation values and reproduces them from `source`.
    pub fn from_source(domain: CircleDomain, source: SharedTraceSource) -> Self {
        // The evaluation keeps only the domain; it is built on a placeholder of matching size.
        let mut evals = CircleEvaluation::new(
            CanonicCoset::new(1).circle_domain(),
            Col::<B, BaseField>::zeros(2),
        );
        evals.values = Col::<B, BaseField>::zeros(0);
        evals.domain = domain;
        Self { coeffs: None, evals, source: Some(source) }
    }

    /// Whether the column keeps neither its coefficients nor any of its evaluation values, and
    /// reproduces them from its [`TraceSource`].
    pub fn is_sourced(&self) -> bool {
        self.coeffs.is_none() && self.source.is_some() && self.evals.values.is_empty()
    }

    /// Stripe 0 of the column's extension (its evaluation on the first subdomain of the
    /// coefficients' size): borrowed where it is stored, regrown where the column has a source.
    pub fn stripe0(
        &self,
        twiddles: &TwiddleTree<B>,
    ) -> std::borrow::Cow<'_, CircleEvaluation<B, BaseField, BitReversedOrder>> {
        if !self.is_sourced() {
            return std::borrow::Cow::Borrowed(&self.evals);
        }
        let coeffs = self.regrown_coefficients();
        let mut values = Col::<B, BaseField>::zeros(1 << coeffs.log_size());
        B::evaluate_stripe_into(&coeffs, self.evals.domain, twiddles, 0, &mut values);
        let mut evals = CircleEvaluation::new(
            CanonicCoset::new(coeffs.log_size()).circle_domain(),
            values,
        );
        evals.domain = self.evals.domain;
        std::borrow::Cow::Owned(evals)
    }

    pub fn eval_at_point(
        &self,
        point: CirclePoint<SecureField>,
        weights_hash_map: Option<&WeightsHashMap<B>>,
    ) -> SecureField {
        if let Some(coeffs) = &self.coeffs {
            coeffs.eval_at_point(point)
        } else if weights_hash_map.is_none() && self.evals.values.len() < self.evals.domain.size() {
            // A striped column holds only stripe 0, which determines the polynomial: interpolate
            // it for the sample.
            self.regrown_coefficients().eval_at_point(point)
        } else {
            self.evals.barycentric_eval_at_point(
                &weights_hash_map
                    .unwrap()
                    .get(&(self.evals.domain.log_size(), point))
                    .expect("weights should exist for all sampled points"),
            )
        }
    }

    /// The coefficients of a striped column that holds only stripe 0 (its evaluation on the
    /// first subdomain of the coefficients' size), interpolated from it.
    pub fn regrown_coefficients(&self) -> CircleCoefficients<B> {
        if self.is_sourced() {
            // The trace values interpolate to the same polynomial as stripe 0 does.
            let source = self.source.as_ref().unwrap();
            let domain = CanonicCoset::new(source.log_size()).circle_domain();
            let twiddles = B::subdomain_twiddles(domain.half_coset);
            return CircleEvaluation::<B, BaseField, BitReversedOrder>::new(
                domain,
                source_column::<B>(source.as_ref()),
            )
            .interpolate_with_twiddles(&twiddles);
        }
        let log_blowup_factor = self.evals.domain.log_size() - self.evals.values.len().ilog2();
        let subdomain = self.evals.domain.split(log_blowup_factor).0;
        let sub_twiddles = B::subdomain_twiddles(subdomain.half_coset);
        CircleEvaluation::<B, BaseField, BitReversedOrder>::new(subdomain, self.evals.values.clone())
            .interpolate_with_twiddles(&sub_twiddles)
    }

    /// [`Self::regrown_coefficients`], interpolating stripe 0 in its own buffer, which the column
    /// gives up.
    pub fn take_regrown_coefficients(&mut self) -> CircleCoefficients<B> {
        if self.is_sourced() {
            return self.regrown_coefficients();
        }
        let log_blowup_factor = self.evals.domain.log_size() - self.evals.values.len().ilog2();
        let subdomain = self.evals.domain.split(log_blowup_factor).0;
        let sub_twiddles = B::subdomain_twiddles(subdomain.half_coset);
        let values = std::mem::replace(&mut self.evals.values, Col::<B, BaseField>::zeros(0));
        CircleEvaluation::<B, BaseField, BitReversedOrder>::new(subdomain, values)
            .interpolate_with_twiddles(&sub_twiddles)
    }

    pub fn get_evaluation_on_domain(
        &self,
        domain: CircleDomain,
        twiddles: &TwiddleTree<B>,
    ) -> CircleEvaluation<B, BaseField, BitReversedOrder> {
        if let Some(coeffs) = &self.coeffs {
            coeffs.evaluate_with_twiddles(domain, twiddles)
        } else if self.evals.values.len() < self.evals.domain.size() {
            // A striped column holds only stripe 0, its evaluation on the first subdomain of the
            // coefficients' size, which determines the polynomial.
            self.regrown_coefficients().evaluate_with_twiddles(domain, twiddles)
        } else {
            panic!("The polynomial's coefficients are not stored");
        }
    }
}

pub struct ComponentProvers<'a, B: Backend> {
    pub components: Vec<&'a dyn ComponentProver<B>>,
    pub n_preprocessed_columns: usize,
}

impl<B: Backend> ComponentProvers<'_, B> {
    pub fn components(&self) -> Components<'_> {
        Components {
            components: self.components.iter().map(|c| *c as &dyn Component).collect_vec(),
            n_preprocessed_columns: self.n_preprocessed_columns,
        }
    }
    pub fn compute_composition_polynomial(
        &self,
        random_coeff: SecureField,
        trace: &Trace<'_, B>,
        twiddles: &TwiddleTree<B>,
        log_blowup_factor: u32,
    ) -> SecureCirclePoly<B> {
        let total_constraints: usize = self.components.iter().map(|c| c.n_constraints()).sum();
        let components: Vec<&dyn Component> =
            self.components.iter().map(|c| *c as &dyn Component).collect();
        // A striped commitment keeps only stripe 0 of a column: extend each component's columns
        // on demand instead of borrowing the committed 2^(n+1) prefix, one size group at a time
        // so that only one component's extension is resident.
        let extend = trace
            .polys
            .iter()
            .flatten()
            .any(|poly| poly.evals.values.len() < poly.evals.domain.size());
        let evaluation_mode = if extend {
            EvaluationMode::ExtendToEvalDomain
        } else {
            EvaluationMode::infer(&components, log_blowup_factor)
        };
        let max_log_size = self.components().composition_log_degree_bound();

        // Each component accumulates into the column of its evaluation domain's size, with the
        // slice of the random coefficient powers that the accumulator hands out from the end in
        // component order. Components of different sizes write disjoint columns, so they run
        // concurrently; components of the same size run one after the other, as before, so
        // every column receives the same contributions in the same order.
        let powers = B::generate_secure_powers(random_coeff, total_constraints);
        let mut end = total_constraints;
        let mut groups: Vec<(u32, Vec<(usize, core::ops::Range<usize>)>)> = vec![];
        for (index, component) in self.components.iter().enumerate() {
            let n = component.n_constraints();
            let start = end - n;
            let log_size = match evaluation_mode {
                EvaluationMode::SubDomain { log_expansion } => {
                    component.max_constraint_log_degree_bound() - log_expansion
                }
                EvaluationMode::ExtendToEvalDomain => component.max_constraint_log_degree_bound(),
            };
            match groups.iter_mut().find(|(size, _)| *size == log_size) {
                Some((_, members)) => members.push((index, start..end)),
                None => groups.push((log_size, vec![(index, start..end)])),
            }
            end = start;
        }

        let run_group = |(_, members): &(u32, Vec<(usize, core::ops::Range<usize>)>)| {
            // The powers of the group's members, last member first, so that the accumulator's
            // split from the end gives each member its slice in turn.
            let group_powers: Vec<SecureField> = members
                .iter()
                .rev()
                .flat_map(|(_, range)| powers[range.clone()].iter().copied())
                .collect();
            let mut accumulator = DomainEvaluationAccumulator::with_powers(
                group_powers,
                max_log_size,
                evaluation_mode,
            );
            for (index, _) in members {
                self.components[*index]
                    .evaluate_constraint_quotients_on_domain(trace, &mut accumulator);
            }
            accumulator.into_sub_accumulations()
        };
        #[cfg(not(feature = "parallel"))]
        let group_results: Vec<_> = groups.iter().map(run_group).collect();
        #[cfg(feature = "parallel")]
        let group_results: Vec<_> = if extend {
            groups.iter().map(run_group).collect()
        } else {
            use rayon::prelude::*;
            groups.par_iter().map(run_group).collect()
        };

        let mut sub_accumulations: Vec<Option<SecureColumnByCoords<B>>> =
            (0..=max_log_size as usize).map(|_| None).collect();
        for group in group_results {
            for (log_size, column) in group.into_iter().enumerate() {
                if let Some(column) = column {
                    assert!(
                        sub_accumulations[log_size].replace(column).is_none(),
                        "two groups accumulated the same size"
                    );
                }
            }
        }
        DomainEvaluationAccumulator::from_sub_accumulations(sub_accumulations, evaluation_mode)
            .finalize(twiddles)
    }
}
