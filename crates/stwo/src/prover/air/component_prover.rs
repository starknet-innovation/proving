use dashmap::DashMap;
use itertools::Itertools;

use crate::core::ColumnVec;
use crate::core::air::{Component, Components};
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::TreeVec;
use crate::core::poly::circle::CircleDomain;
use crate::prover::CirclePoint;
use crate::prover::air::accumulation::{DomainEvaluationAccumulator, EvaluationMode};
use crate::prover::backend::Backend;
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
}

/// A struct for representing a polynomial corresponding to a trace column.
/// A polynomial is defined by it's evaluations on a circle domain of size at least it's degree,
/// and optionally its coefficients in the FFT basis.
pub struct Poly<B: Backend> {
    pub coeffs: Option<CircleCoefficients<B>>,
    pub evals: CircleEvaluation<B, BaseField, BitReversedOrder>,
}

impl<B: Backend> Poly<B> {
    pub const fn new(
        coeffs: Option<CircleCoefficients<B>>,
        evals: CircleEvaluation<B, BaseField, BitReversedOrder>,
    ) -> Self {
        Self { coeffs, evals }
    }

    pub fn eval_at_point(
        &self,
        point: CirclePoint<SecureField>,
        weights_hash_map: Option<&WeightsHashMap<B>>,
    ) -> SecureField {
        if let Some(coeffs) = &self.coeffs {
            coeffs.eval_at_point(point)
        } else {
            self.evals.barycentric_eval_at_point(
                &weights_hash_map
                    .unwrap()
                    .get(&(self.evals.domain.log_size(), point))
                    .expect("weights should exist for all sampled points"),
            )
        }
    }

    pub fn get_evaluation_on_domain(
        &self,
        domain: CircleDomain,
        twiddles: &TwiddleTree<B>,
    ) -> CircleEvaluation<B, BaseField, BitReversedOrder> {
        if let Some(coeffs) = &self.coeffs {
            coeffs.evaluate_with_twiddles(domain, twiddles)
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
        let evaluation_mode = EvaluationMode::infer(&components, log_blowup_factor);
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
        let group_results: Vec<_> = {
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
