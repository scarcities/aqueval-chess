//! Vector kernels. The release build targets plain x86-64, so AVX2 is picked at run time:
//! the `#[target_feature]` functions are only called after `has_avx2()` said yes.

use super::{Accumulator, HIDDEN, QA};

pub fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        // QUANT_NO_AVX2 forces the scalar kernels, so tests can cover CPUs without AVX2.
        std::is_x86_feature_detected!("avx2") && std::env::var_os("QUANT_NO_AVX2").is_none()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Σ clamp(acc, 0, QA)² · w, the SCReLU output layer for one perspective.
#[inline]
pub fn screlu_dot_scalar(acc: &Accumulator, w: &Accumulator) -> i64 {
    let mut sum = 0i64;
    for (&a, &w) in acc.vals.iter().zip(w.vals.iter()) {
        let c = (a as i32).clamp(0, QA);
        // c² · w fits in i32: 255² · 32767 < 2³¹.
        sum += (c * c * w as i32) as i64;
    }
    sum
}

/// AVX2 version of `screlu_dot_scalar`. Requires every |w| ≤ 128 so that c · w fits in i16.
///
/// # Safety
/// The CPU must support AVX2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn screlu_dot_avx2(acc: &Accumulator, w: &Accumulator) -> i64 {
    use std::arch::x86_64::*;
    let zero = _mm256_setzero_si256();
    let qa = _mm256_set1_epi16(QA as i16);
    let mut sum = _mm256_setzero_si256();
    for i in (0..HIDDEN).step_by(16) {
        // Accumulator is 64-byte aligned and HIDDEN is a multiple of 16: aligned loads.
        let a = _mm256_load_si256(acc.vals.as_ptr().add(i) as *const __m256i);
        let wv = _mm256_load_si256(w.vals.as_ptr().add(i) as *const __m256i);
        let c = _mm256_min_epi16(_mm256_max_epi16(a, zero), qa);
        // (c · w) · c: the first product stays in i16, madd widens the second to i32 pairs.
        let cw = _mm256_mullo_epi16(c, wv);
        sum = _mm256_add_epi32(sum, _mm256_madd_epi16(cw, c));
    }
    let lo = _mm256_cvtepi32_epi64(_mm256_castsi256_si128(sum));
    let hi = _mm256_cvtepi32_epi64(_mm256_extracti128_si256::<1>(sum));
    let s = _mm256_add_epi64(lo, hi);
    let mut lanes = [0i64; 4];
    _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, s);
    lanes.iter().sum()
}

/// dst = src + Σ adds − Σ subs, element-wise with i16 wraparound.
#[inline(always)]
fn update_body<const A: usize, const S: usize>(
    dst: &mut Accumulator,
    src: &Accumulator,
    adds: [&Accumulator; A],
    subs: [&Accumulator; S],
) {
    for i in 0..HIDDEN {
        let mut v = src.vals[i];
        for a in adds {
            v = v.wrapping_add(a.vals[i]);
        }
        for s in subs {
            v = v.wrapping_sub(s.vals[i]);
        }
        dst.vals[i] = v;
    }
}

/// The same loop compiled with AVX2 enabled, so LLVM vectorises it 16 lanes wide.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn update_avx2<const A: usize, const S: usize>(
    dst: &mut Accumulator,
    src: &Accumulator,
    adds: [&Accumulator; A],
    subs: [&Accumulator; S],
) {
    update_body(dst, src, adds, subs)
}

#[inline(always)]
pub fn update<const A: usize, const S: usize>(
    avx2: bool,
    dst: &mut Accumulator,
    src: &Accumulator,
    adds: [&Accumulator; A],
    subs: [&Accumulator; S],
) {
    #[cfg(target_arch = "x86_64")]
    if avx2 {
        // SAFETY: `avx2` is only true when `has_avx2()` returned true.
        return unsafe { update_avx2(dst, src, adds, subs) };
    }
    let _ = avx2;
    update_body(dst, src, adds, subs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::zobrist::SplitMix64;

    fn random_acc(rng: &mut SplitMix64, range: i64) -> Accumulator {
        let mut acc = Accumulator::ZERO;
        for v in acc.vals.iter_mut() {
            *v = ((rng.next_u64() % (2 * range as u64 + 1)) as i64 - range) as i16;
        }
        acc
    }

    #[test]
    fn avx2_kernels_match_scalar() {
        if !has_avx2() {
            return;
        }
        let mut rng = SplitMix64::new(11);
        for _ in 0..50 {
            let acc = random_acc(&mut rng, 600);
            let w = random_acc(&mut rng, 128);
            #[cfg(target_arch = "x86_64")]
            assert_eq!(screlu_dot_scalar(&acc, &w), unsafe { screlu_dot_avx2(&acc, &w) });

            let (a1, a2, s1, s2) = (random_acc(&mut rng, 300), random_acc(&mut rng, 300), random_acc(&mut rng, 300), random_acc(&mut rng, 300));
            let (mut x, mut y) = (Accumulator::ZERO, Accumulator::ZERO);
            update(false, &mut x, &acc, [&a1, &a2], [&s1, &s2]);
            update(true, &mut y, &acc, [&a1, &a2], [&s1, &s2]);
            assert_eq!(x, y);
        }
    }
}
