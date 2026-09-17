//! **HOSTILE BYTES** -- the fourth pass of the security audit, the one that
//! reading code cannot do.
//!
//! generacion: ninguna
//! capa: puro -- deterministic byte generator for host tests; no hardware, no unsafe
//!
//! The 2026-08-24 audit said it out loud: *reading code finds what looks like a
//! bug we already know. The paths that only appear when running need millions
//! of garbage inputs per parser.* This crate is that garbage, and nothing else.
//!
//! ## The property it checks, and the one it does not
//!
//! ```text
//!    CHECKED      the parser RETURNS on every input: no panic, no index out
//!                 of bounds, no arithmetic overflow in a debug build, no
//!                 infinite loop longer than the test timeout
//!    NOT CHECKED  that the answer is RIGHT. A parser that accepts garbage as
//!                 a good frame passes here; its own adversarial tests are
//!                 the ones that catch that
//! ```
//!
//! In Ring 0 a panic IS the machine going down, so "never panics" is not a weak
//! property there: it is the difference between a bad USB descriptor being
//! rejected and a bad USB descriptor being a kill switch.
//!
//! ## Why deterministic, and not a real fuzzer
//!
//! `cargo fuzz` needs nightly sanitizers, libFuzzer and a corpus directory, and
//! none of that runs inside `bmo.ps1`. A test that only runs when someone
//! remembers to run it stops running. So: a fixed seed per target, a fixed
//! number of cases, and on failure the case number and the input in hex -- the
//! failure is reproducible with the same `cargo test`, forever.
//!
//! ## How the cases are built
//!
//! Half of the cases start from a GOOD sample the caller provides, because pure
//! random bytes die at the first magic number and never reach the code behind
//! it. The mutations are the ones that break length fields:
//!
//! ```text
//!    random       random bytes, random length
//!    truncate     a good sample cut short
//!    flip         a few bits flipped
//!    extreme      single bytes set to 00 01 7F 80 FF
//!    field        a 16/32-bit field (LE or BE) set to 0, 1, len-1, len,
//!                 len+1, 0x7FFF.., 0xFFFF..
//!    extend       a good sample with random bytes appended
//!    splice       a slice of the sample removed or duplicated
//! ```

#![forbid(unsafe_code)]

use std::panic::{self, AssertUnwindSafe};

/// The seed every target starts from unless it asks for another one.
pub const DEFAULT_SEED: u64 = 0x424D_4F2D_5821_2026;

/// xorshift64* -- small, fast, and the same numbers on every machine.
pub struct Hostile {
    state: u64,
}

impl Hostile {
    pub fn new(seed: u64) -> Self {
        // Zero is the one state xorshift never leaves.
        Hostile { state: if seed == 0 { DEFAULT_SEED } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`. `n == 0` answers 0 instead of dividing by zero.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    pub fn byte(&mut self) -> u8 {
        self.next_u64() as u8
    }

    /// One hostile input, at most `max_len` bytes.
    pub fn case(&mut self, samples: &[&[u8]], max_len: usize) -> Vec<u8> {
        let max_len = max_len.max(1);
        if samples.is_empty() || self.below(8) == 0 {
            let len = self.below(max_len + 1);
            return (0..len).map(|_| self.byte()).collect();
        }
        let mut v = samples[self.below(samples.len())].to_vec();
        match self.below(6) {
            0 => {
                let len = self.below(v.len() + 1);
                v.truncate(len);
            }
            1 => {
                for _ in 0..1 + self.below(8) {
                    if !v.is_empty() {
                        let i = self.below(v.len());
                        v[i] ^= 1 << self.below(8);
                    }
                }
            }
            2 => {
                const EXTREME: [u8; 5] = [0x00, 0x01, 0x7F, 0x80, 0xFF];
                for _ in 0..1 + self.below(4) {
                    if !v.is_empty() {
                        let i = self.below(v.len());
                        v[i] = EXTREME[self.below(EXTREME.len())];
                    }
                }
            }
            3 => self.field(&mut v),
            4 => {
                for _ in 0..1 + self.below(64) {
                    let b = self.byte();
                    v.push(b);
                }
            }
            _ => {
                if !v.is_empty() {
                    let a = self.below(v.len());
                    let b = a + self.below(v.len() - a + 1);
                    if self.below(2) == 0 {
                        v.drain(a..b);
                    } else {
                        let chunk = v[a..b].to_vec();
                        let at = self.below(v.len() + 1);
                        v.splice(at..at, chunk);
                    }
                }
            }
        }
        v.truncate(max_len);
        v
    }

    /// Overwrites a 16- or 32-bit field with a value that breaks length math.
    fn field(&mut self, v: &mut [u8]) {
        let width = if self.below(2) == 0 { 2 } else { 4 };
        if v.len() < width {
            return;
        }
        let at = self.below(v.len() - width + 1);
        let len = v.len() as u64;
        let pick: [u64; 9] = [
            0,
            1,
            len.wrapping_sub(1),
            len,
            len + 1,
            0x7F,
            0xFF,
            if width == 2 { 0x7FFF } else { 0x7FFF_FFFF },
            if width == 2 { 0xFFFF } else { 0xFFFF_FFFF },
        ];
        let value = pick[self.below(pick.len())];
        let big_endian = self.below(2) == 0;
        for k in 0..width {
            let shift = if big_endian { 8 * (width - 1 - k) } else { 8 * k };
            v[at + k] = (value >> shift) as u8;
        }
    }
}

/// Feeds `cases` hostile inputs to `target`. Panics -- naming the target, the
/// case number, the seed and the input in hex -- if the target panics on any
/// of them.
///
/// `target` receives the input and must only parse it. Whatever it returns is
/// ignored on purpose: see the crate header for what this does NOT check.
pub fn attack<F>(name: &str, seed: u64, cases: u32, samples: &[&[u8]], max_len: usize, mut target: F)
where
    F: FnMut(&[u8]),
{
    let mut h = Hostile::new(seed);
    for n in 0..cases {
        let input = h.case(samples, max_len);
        let result = panic::catch_unwind(AssertUnwindSafe(|| target(&input)));
        if result.is_err() {
            panic!(
                "HOSTILE: `{}` panicked on case {} of {} (seed {:#x}), input {} bytes: {}",
                name,
                n,
                cases,
                seed,
                input.len(),
                hex(&input, 256)
            );
        }
    }
}

/// The first `limit` bytes in hex, so a failure can be pasted into a test.
pub fn hex(bytes: &[u8], limit: usize) -> String {
    let mut s = String::with_capacity(bytes.len().min(limit) * 3);
    for b in bytes.iter().take(limit) {
        s.push_str(&format!("{:02x} ", b));
    }
    if bytes.len() > limit {
        s.push_str("...");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_cases() {
        let sample: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8];
        let mut a = Hostile::new(7);
        let mut b = Hostile::new(7);
        for _ in 0..500 {
            assert_eq!(a.case(&[sample], 64), b.case(&[sample], 64));
        }
    }

    #[test]
    fn no_case_is_longer_than_asked() {
        let sample = [0xAAu8; 300];
        let mut h = Hostile::new(DEFAULT_SEED);
        for _ in 0..2000 {
            assert!(h.case(&[&sample], 100).len() <= 100);
        }
    }

    #[test]
    fn a_panicking_target_is_named_with_its_case() {
        let r = panic::catch_unwind(|| {
            attack("toy", 1, 1000, &[], 16, |b| {
                if b.len() > 3 && b[0] == b[1] {
                    panic!("found it");
                }
            })
        });
        let msg = r.expect_err("the toy target must be caught");
        let text = msg.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(text.contains("`toy` panicked on case"), "{}", text);
    }

    #[test]
    fn the_mutations_reach_length_fields() {
        // A 4-byte sample must at some point become 0xFFFFFFFF in either order.
        let sample = [0u8; 4];
        let mut h = Hostile::new(3);
        let seen = (0..20_000).any(|_| h.case(&[&sample], 4) == [0xFF; 4]);
        assert!(seen);
    }
}
