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
    /// Writes the values of the rows `first..first + dst.len()`, the ones [`Self::write_values`]
    /// writes there, into `dst`.
    fn write_rows(&self, first: usize, dst: &mut [BaseField]) {
        let mut values = vec![BaseField::from(0); 1 << self.log_size()];
        self.write_values(&mut values);
        dst.copy_from_slice(&values[first..first + dst.len()]);
    }
}

/// A shared [`TraceSource`].
pub type SharedTraceSource = std::sync::Arc<dyn TraceSource>;

/// The values of `source` on its trace domain, in bit-reversed order, as a column.
pub fn source_column<B: Backend>(source: &dyn TraceSource) -> Col<B, BaseField> {
    Col::<B, BaseField>::from_rows(1 << source.log_size(), &|first, dst| {
        source.write_rows(first, dst)
    })
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

    /// The values of a sourced column on its trace domain (the canonic coset of its
    /// coefficients' size), in bit-reversed order: the evaluation that
    /// [`Self::regrown_coefficients`] interpolates. `None` unless the column
    /// [is sourced](Self::is_sourced).
    pub fn source_evaluation(&self) -> Option<CircleEvaluation<B, BaseField, BitReversedOrder>> {
        if !self.is_sourced() {
            return None;
        }
        let source: &dyn TraceSource = self.source.as_deref()?;
        let domain: CircleDomain = CanonicCoset::new(source.log_size()).circle_domain();
        Some(CircleEvaluation::new(domain, source_column::<B>(source)))
    }

    /// The coefficients of a striped column that holds only stripe 0 (its evaluation on the
    /// first subdomain of the coefficients' size), interpolated from it.
    pub fn regrown_coefficients(&self) -> CircleCoefficients<B> {
        if let Some(evaluation) = self.source_evaluation() {
            // The trace values interpolate to the same polynomial as stripe 0 does.
            let twiddles = B::subdomain_twiddles(evaluation.domain.half_coset);
            return evaluation.interpolate_with_twiddles(&twiddles);
        }
        let log_blowup_factor = self.evals.domain.log_size() - self.evals.values.len().ilog2();
        let subdomain = self.evals.domain.split(log_blowup_factor).0;
        let sub_twiddles = B::subdomain_twiddles(subdomain.half_coset);
        CircleEvaluation::<B, BaseField, BitReversedOrder>::new(subdomain, self.evals.values.clone())
            .interpolate_with_twiddles(&sub_twiddles)
    }

    /// Computes (or finds cached) the subdomain twiddles that [`Self::regrown_coefficients`]
    /// interpolates with, without regrowing anything. Called once before many columns are
    /// regrown in parallel, it keeps them from each precomputing the same tree at once.
    pub fn warm_regrowth_twiddles(&self) {
        if self.is_sourced() {
            if let Some(source) = self.source.as_deref() {
                let domain: CircleDomain = CanonicCoset::new(source.log_size()).circle_domain();
                let _ = B::subdomain_twiddles(domain.half_coset);
                return;
            }
        }
        let log_blowup_factor = self.evals.domain.log_size() - self.evals.values.len().ilog2();
        let _ = B::subdomain_twiddles(self.evals.domain.split(log_blowup_factor).0.half_coset);
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

/// A component's power range in the composition's random coefficient powers.
type PowerRange = core::ops::Range<usize>;

/// In extend mode, a component whose extended columns and accumulator column take at most this
/// many bytes is light: light components run in concurrent lanes. A heavier component (the
/// block-evaluated ones among them) runs alone, one after the other, as before.
#[cfg(feature = "parallel")]
const LANE_COMPONENT_MAX_BYTES: usize = 128 << 20;
/// The most bytes the light components in flight take together: the sum of the lanes' largest.
#[cfg(feature = "parallel")]
const LANES_BUDGET_BYTES: usize = 384 << 20;
/// The most lanes; each lane's component is itself parallel over columns and rows.
#[cfg(feature = "parallel")]
const MAX_LANES: usize = 4;
/// Lanes only pay with many small components (the Cairo leg's ~55, not the circuit's few).
#[cfg(feature = "parallel")]
const LANES_MIN_COMPONENTS: usize = 16;

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
        // every column receives the same contributions in the same order (in extend mode, light
        // components run in lanes instead, see `plan_lanes`).
        let powers = B::generate_secure_powers(random_coeff, total_constraints);
        let mut end = total_constraints;
        let mut groups: Vec<(u32, Vec<(usize, PowerRange)>)> = vec![];
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
        let group_bytes = |(log_size, members): &(u32, Vec<(usize, PowerRange)>)| -> usize {
            members
                .iter()
                .map(|(index, _)| {
                    let n_columns: usize = self.components[*index]
                        .trace_log_degree_bounds()
                        .iter()
                        .map(|tree| tree.len())
                        .sum();
                    (n_columns * core::mem::size_of::<BaseField>()
                        + core::mem::size_of::<SecureField>())
                        << *log_size
                })
                .max()
                .unwrap_or(0)
        };
        groups.sort_by_key(|group| core::cmp::Reverse(group_bytes(group)));

        // Runs `members` one after the other into one accumulator of their own, which starts from
        // zero columns: the powers of the members, last member first, so that the accumulator's
        // split from the end gives each member its slice in turn.
        let run_members = |members: &[(usize, PowerRange)]| {
            let member_powers: Vec<SecureField> = members
                .iter()
                .rev()
                .flat_map(|(_, range)| powers[range.clone()].iter().copied())
                .collect();
            let mut accumulator = DomainEvaluationAccumulator::with_powers(
                member_powers,
                max_log_size,
                evaluation_mode,
            );
            for (index, _) in members {
                self.components[*index]
                    .evaluate_constraint_quotients_on_domain(trace, &mut accumulator);
            }
            accumulator.into_sub_accumulations()
        };
        let run_group = |(_, members): &(u32, Vec<(usize, PowerRange)>)| run_members(members);
        #[cfg(not(feature = "parallel"))]
        let group_results: Vec<_> = groups.iter().map(run_group).collect();
        #[cfg(feature = "parallel")]
        let group_results: Vec<_> = if extend {
            // The heavy components run alone, one after the other, as before; then the light
            // ones run in concurrent lanes, each lane into its own accumulator. M31 sums are
            // exact and every accumulated value stays canonical (below P), so summing the lanes'
            // columns below gives the same words as accumulating every component in one column.
            use rayon::prelude::*;
            match self.plan_lanes(&groups) {
                Some((heavy, lanes)) => {
                    let mut results = vec![run_members(&heavy)];
                    results.extend(
                        lanes.par_iter().map(|lane| run_members(lane)).collect::<Vec<_>>(),
                    );
                    results
                }
                None => groups.iter().map(run_group).collect(),
            }
        } else {
            use rayon::prelude::*;
            groups.par_iter().map(run_group).collect()
        };

        // Groups of different sizes write disjoint columns; lanes may share a size, and their
        // columns of that size are added (exact, canonical: the order does not change a word).
        let mut sub_accumulations: Vec<Option<SecureColumnByCoords<B>>> =
            (0..=max_log_size as usize).map(|_| None).collect();
        for group in group_results {
            for (log_size, column) in group.into_iter().enumerate() {
                if let Some(column) = column {
                    if let Some(accumulated) = &mut sub_accumulations[log_size] {
                        B::accumulate(accumulated, &column);
                    } else {
                        sub_accumulations[log_size] = Some(column);
                    }
                }
            }
        }
        DomainEvaluationAccumulator::from_sub_accumulations(sub_accumulations, evaluation_mode)
            .finalize(twiddles)
    }

    /// Splits the extend-mode composition's components into the heavy ones, in group order, and
    /// lanes of light ones (see [`LANE_COMPONENT_MAX_BYTES`]), or `None` when lanes do not apply.
    /// A component's bytes are its extended columns plus its accumulator column on its evaluation
    /// domain. The light components go largest first, each to the lane with the fewest bytes, so
    /// the first `n_lanes` open one lane each and are their lanes' largest: the bytes in flight
    /// are at most theirs together, which stays within [`LANES_BUDGET_BYTES`].
    #[cfg(feature = "parallel")]
    #[allow(clippy::type_complexity)]
    fn plan_lanes(
        &self,
        groups: &[(u32, Vec<(usize, PowerRange)>)],
    ) -> Option<(Vec<(usize, PowerRange)>, Vec<Vec<(usize, PowerRange)>>)> {
        let mut heavy = vec![];
        let mut light: Vec<(usize, usize, PowerRange)> = vec![];
        for (log_size, members) in groups {
            for (index, range) in members {
                let n_columns: usize = self.components[*index]
                    .trace_log_degree_bounds()
                    .iter()
                    .map(|tree| tree.len())
                    .sum();
                let bytes = (n_columns * core::mem::size_of::<BaseField>()
                    + core::mem::size_of::<SecureField>())
                    << log_size;
                if bytes <= LANE_COMPONENT_MAX_BYTES {
                    light.push((bytes, *index, range.clone()));
                } else {
                    heavy.push((*index, range.clone()));
                }
            }
        }
        if light.len() < LANES_MIN_COMPONENTS {
            return None;
        }
        light.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut n_lanes = MAX_LANES.min(rayon::current_num_threads()).min(light.len());
        while n_lanes > 1
            && light[..n_lanes].iter().map(|(bytes, ..)| bytes).sum::<usize>() > LANES_BUDGET_BYTES
        {
            n_lanes -= 1;
        }
        if n_lanes < 2 {
            return None;
        }
        let mut loads = vec![0usize; n_lanes];
        let mut lanes: Vec<Vec<(usize, PowerRange)>> = vec![vec![]; n_lanes];
        for (bytes, index, range) in light {
            let lane = (0..n_lanes).min_by_key(|&lane| loads[lane]).unwrap();
            loads[lane] += bytes;
            lanes[lane].push((index, range));
        }
        Some((heavy, lanes))
    }
}
