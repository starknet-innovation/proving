//! Sums of products of very packed base field values with base field constants, reduced once.

use std::simd::{u32x16, u64x8};

use super::m31::{MODULUS, PackedM31};
use super::very_packed_m31::VeryPackedM31;
use crate::core::fields::m31::{M31, P};

/// The products added between two folds of a [`VeryPackedM31Dot`]: a folded lane is below 2^34
/// and each product of two values in `[0, P]` below `(2^31 - 1)^2`, so four of them still fit.
const PRODUCTS_PER_FOLD: usize = 4;

/// `init + ∑ values[i] * coeffs[i]` lane by lane, as exact 64-bit sums: each product is one
/// widening multiply and one 64-bit add instead of a reduced field product and a reduced field
/// sum. The lanes are folded (`hi * 2^31 + lo == hi + lo` modulo `P`) every few products and
/// reduced into `[0, P)` once, by [`Self::reduce`]: the same field element as the reduced sum.
///
/// On AVX2 the lanes are kept as the even and the odd `u32` lanes of each 256-bit half (one
/// `vpmuludq` per four products), elsewhere as the lower and upper eight lanes of each packed
/// value, zero-extended.
#[derive(Copy, Clone, Debug)]
pub struct VeryPackedM31Dot([u64x8; 4]);

impl VeryPackedM31Dot {
    /// The sum of no products: `init`, every lane in `[0, P]`.
    #[inline(always)]
    pub fn new(init: VeryPackedM31) -> Self {
        let mut acc = [u64x8::splat(0); 4];
        for (p, packed) in init.0.iter().enumerate() {
            let [a, b] = split(packed.into_simd());
            acc[2 * p] = a;
            acc[2 * p + 1] = b;
        }
        Self(acc)
    }

    /// Adds `∑ value * coeff` over `terms`.
    #[inline(always)]
    pub fn add_products(&mut self, terms: impl IntoIterator<Item = (VeryPackedM31, M31)>) {
        let mut n = 0;
        for (value, coeff) in terms {
            if n == PRODUCTS_PER_FOLD {
                self.fold();
                n = 0;
            }
            self.add_product(value, coeff);
            n += 1;
        }
        if n != 0 {
            self.fold();
        }
    }

    /// The sum, every lane reduced into `[0, P)`.
    #[inline(always)]
    pub fn reduce(self) -> VeryPackedM31 {
        let fold = |x: u64x8| (x & u64x8::splat(P as u64)) + (x >> 31);
        VeryPackedM31::from_fn(|p| {
            // Two folds take a lane below `2^31 + 8`; one conditional subtraction reduces it.
            let lanes = join(fold(fold(self.0[2 * p])), fold(fold(self.0[2 * p + 1])));
            use std::simd::cmp::SimdOrd;
            // SAFETY: every lane is in `[0, P)`.
            unsafe { PackedM31::from_simd_unchecked(lanes.simd_min(lanes - MODULUS)) }
        })
    }

    /// Keeps every lane below 2^34 (the same value modulo `P`).
    #[inline(always)]
    fn fold(&mut self) {
        for x in &mut self.0 {
            *x = (*x & u64x8::splat(P as u64)) + (*x >> 31);
        }
    }

    #[inline(always)]
    fn add_product(&mut self, value: VeryPackedM31, coeff: M31) {
        for (p, packed) in value.0.iter().enumerate() {
            let [a, b] = mul_lanes(packed.into_simd(), coeff.0);
            self.0[2 * p] += a;
            self.0[2 * p + 1] += b;
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))] {
        use std::arch::x86_64::{__m256i, _mm256_mul_epu32};
        use std::mem::transmute;

        /// The even and the odd lanes of each half of `v`, zero-extended:
        /// `[even(h0), even(h1)], [odd(h0), odd(h1)]`.
        #[inline(always)]
        fn split(v: u32x16) -> [u64x8; 2] {
            let [h0, h1]: [std::simd::u64x4; 2] = unsafe { transmute::<u32x16, _>(v) };
            let mask = std::simd::u64x4::splat(0xFFFF_FFFF);
            [join4(h0 & mask, h1 & mask), join4(h0 >> 32, h1 >> 32)]
        }

        /// The inverse of [`split`], each lane below 2^32.
        #[inline(always)]
        fn join(even: u64x8, odd: u64x8) -> u32x16 {
            let packed = even | (odd << 32);
            unsafe { transmute::<u64x8, u32x16>(packed) }
        }

        #[inline(always)]
        fn join4(a: std::simd::u64x4, b: std::simd::u64x4) -> u64x8 {
            unsafe { transmute::<[std::simd::u64x4; 2], u64x8>([a, b]) }
        }

        /// `split(v) * coeff`, exact: one `vpmuludq` per four lanes.
        #[inline(always)]
        fn mul_lanes(v: u32x16, coeff: u32) -> [u64x8; 2] {
            let [h0, h1]: [__m256i; 2] = unsafe { transmute::<u32x16, _>(v) };
            let c: __m256i = unsafe { transmute::<std::simd::u32x8, _>(std::simd::u32x8::splat(coeff)) };
            let mul = |a: __m256i| -> std::simd::u64x4 { unsafe { transmute(_mm256_mul_epu32(a, c)) } };
            let odd = |a: __m256i| -> __m256i {
                unsafe { transmute(transmute::<__m256i, std::simd::u64x4>(a) >> 32) }
            };
            [join4(mul(h0), mul(h1)), join4(mul(odd(h0)), mul(odd(h1)))]
        }
    } else {
        use std::simd::num::SimdUint;

        /// The lower and the upper eight lanes of `v`, zero-extended.
        #[inline(always)]
        fn split(v: u32x16) -> [u64x8; 2] {
            let lo = std::simd::simd_swizzle!(v, [0, 1, 2, 3, 4, 5, 6, 7]);
            let hi = std::simd::simd_swizzle!(v, [8, 9, 10, 11, 12, 13, 14, 15]);
            [lo.cast::<u64>(), hi.cast::<u64>()]
        }

        /// The inverse of [`split`], each lane below 2^32.
        #[inline(always)]
        fn join(lo: u64x8, hi: u64x8) -> u32x16 {
            let (lo, hi) = (lo.cast::<u32>(), hi.cast::<u32>());
            std::simd::simd_swizzle!(lo, hi, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
        }

        /// `split(v) * coeff`, exact (widening multiplies).
        #[inline(always)]
        fn mul_lanes(v: u32x16, coeff: u32) -> [u64x8; 2] {
            let [lo, hi] = split(v);
            let c = u64x8::splat(coeff as u64);
            [lo * c, hi * c]
        }
    }
}

mod dot_patch_tests;
