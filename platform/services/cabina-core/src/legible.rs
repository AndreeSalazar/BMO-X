//! **Turning a number back into the fact it already was.**
//!
//! # Why this lives here and not in the renderer
//!
//! Because here it can be RUN. The kernel crate cannot be tested -- `cargo test`
//! links `std` and the kernel is `no_std` -- so anything that lives there is
//! checked by reading it, which is how this project has already been burned:
//! nine floating-point tests in the C frontend are green and **none of them
//! executes**, because the emulator has no `xmm`.
//!
//! Formatting is exactly the kind of code that is wrong quietly. An off-by-one in
//! a nibble loop, a MAC printed backwards, a scale that rounds the wrong way --
//! none of those fail. They print something plausible, and a plausible wrong
//! number is worse than no number, because it gets believed and acted on.
//!
//! So the decisions live here with tests, and the renderer keeps only the part
//! that cannot be tested anywhere: putting bytes on a screen.
//!
//! # The rule these all follow
//!
//! **Readable AND checkable in the same string.** A size prints its scale and its
//! exact number, because the scale is what a human reads and the exact number is
//! what gets compared against a file size. Either alone is half a fact.

/// A fixed byte sink, so this module can be `no_std` and allocation-free while
/// still being ordinary testable code.
///
/// Writes past the end are DROPPED, not wrapped and not panicking: a diagnostic
/// that panics takes down the thing it was diagnosing.
pub struct Escritor<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl<'a> Escritor<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Escritor { buf, n: 0 }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.n]).unwrap_or("")
    }

    pub fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() {
            if self.n < self.buf.len() {
                self.buf[self.n] = c;
                self.n += 1;
            }
        }
    }

    pub fn dec(&mut self, mut v: u64) {
        if v == 0 {
            self.txt("0");
            return;
        }
        let mut tmp = [0u8; 20];
        let mut i = 0;
        while v > 0 {
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            if self.n < self.buf.len() {
                self.buf[self.n] = tmp[i];
                self.n += 1;
            }
        }
    }

    pub fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.n < self.buf.len() {
                self.buf[self.n] = H[((v >> (i * 4)) & 0xF) as usize];
                self.n += 1;
            }
        }
    }

    /// Hex with no leading zeros. The historical presentation of an event value,
    /// kept because `Fmt::Raw` has to keep printing exactly as it did.
    pub fn hex_min(&mut self, v: u64) {
        if v == 0 {
            self.txt("0");
            return;
        }
        let mut digits = 1;
        let mut t = v >> 4;
        while t > 0 {
            t >>= 4;
            digits += 1;
        }
        self.hex(v, digits);
    }
}

/// A size, **with its scale and its exact number**.
///
/// Both, always. `4.0 MiB` is readable and cannot be checked against 4196020;
/// `4196020` can be checked and says nothing at a glance. The pair is one fact.
pub fn size(w: &mut Escritor, v: u64) {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    let unidad = if v >= GIB {
        GIB
    } else if v >= MIB {
        MIB
    } else if v >= KIB {
        KIB
    } else {
        w.dec(v);
        w.txt(" B");
        return;
    };
    w.dec(v / unidad);
    w.txt(".");
    // One decimal, truncated and not rounded. Rounding up would let 4194303
    // bytes print as `4.0 MiB` while being one byte short of it, and "just under
    // a boundary" is precisely the interesting case in a buffer bug.
    w.dec((v % unidad) * 10 / unidad);
    w.txt(match unidad {
        GIB => " GiB (",
        MIB => " MiB (",
        _ => " KiB (",
    });
    w.dec(v);
    w.txt(")");
}

/// Bytes of a page. Not imported from the kernel on purpose: this crate must not
/// depend on it, and 4096 is not going to move under us.
const PAGE: u64 = 4096;

/// An address, **with its offset inside the page when that offset is the point**.
///
/// # Why the offset, and it is not decoration
///
/// The split-relocation bug of 2026-08-11 WAS the offset. The line said
/// `=1074388988`; the fact was `0x4009DFFC`, which is offset 4092 of its page, so
/// eight bytes written from there cross into the next one. Decimal hid it, hex
/// showed it to whoever already suspected it, and this prints it.
///
/// Only near the edges: an address in the middle of a page has nothing to say,
/// and a line that always says something is a line that stops being read.
pub fn address(w: &mut Escritor, v: u64) {
    w.txt("0x");
    w.hex(v, 8);
    let off = v % PAGE;
    if off == 0 {
        w.txt(" +0/4096");
    } else if off >= PAGE - 8 {
        w.txt(" +");
        w.dec(off);
        w.txt("/4096");
    }
}

/// Six bytes packed with byte 0 at the top, printed `2C:F0:5D:D9:3C:E3`.
///
/// The order is the whole content of this function. A MAC printed backwards is
/// perfectly plausible -- six bytes that are neither zeros nor ones -- so it
/// would be compared against what Windows says, would not match, and the blame
/// would land on the BAR, which would be fine.
pub fn mac(w: &mut Escritor, v: u64) {
    for i in (0..6).rev() {
        if i != 5 {
            w.txt(":");
        }
        w.hex((v >> (i * 8)) & 0xFF, 2);
    }
}

/// A bitfield in binary, grouped in nibbles.
///
/// Which bits are set is the entire content of a bitfield, and hex hides exactly
/// that: `PHYstatus=0x0B` is three separate facts -- link up, 100 Mbps, full
/// duplex -- written so that all three have to be decoded by hand every time.
pub fn bits(w: &mut Escritor, v: u64) {
    w.txt("0b");
    if v == 0 {
        w.txt("0");
        return;
    }
    let mut top = 63;
    while top > 0 && (v >> top) & 1 == 0 {
        top -= 1;
    }
    // Up to a whole nibble so the groups line up between one reading and the next.
    let top = top | 3;
    for i in (0..=top).rev() {
        if i != top && (i + 1) % 4 == 0 {
            w.txt("_");
        }
        w.txt(if (v >> i) & 1 == 1 { "1" } else { "0" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes with a buffer of `cap` bytes and compares. No `String`: this crate
    /// is `no_std` in the test build too, and that is the point -- the code under
    /// test is the same code the kernel links.
    #[track_caller]
    fn ver(cap: usize, f: impl FnOnce(&mut Escritor), esperado: &str) {
        let mut b = [0u8; 128];
        let mut w = Escritor::new(&mut b[..cap.min(128)]);
        f(&mut w);
        assert_eq!(w.as_str(), esperado);
    }

    /// ** THE SIZE CARRIES BOTH HALVES, AND THAT IS THE WHOLE DESIGN.
    ///
    /// The scale is what a human reads; the exact number is what gets compared
    /// against a file size. The WAD is the case that motivated this: the line has
    /// to be checkable against 4196020 without a calculator.
    #[test]
    fn a_size_is_readable_and_checkable_at_once() {
        ver(64, |w| size(w, 4_196_020), "4.0 MiB (4196020)");
        ver(64, |w| size(w, 2048), "2.0 KiB (2048)");
        ver(64, |w| size(w, 815_496), "796.3 KiB (815496)");
        // Under a KiB there is no scale worth printing and no second half.
        ver(64, |w| size(w, 60), "60 B");
        ver(64, |w| size(w, 0), "0 B");
    }

    /// ** THE DECIMAL TRUNCATES, IT DOES NOT ROUND.
    ///
    /// One byte short of 4 MiB has to look short. Rounding up would print `4.0
    /// MiB` for a buffer that is one byte too small -- and "just under a
    /// boundary" is exactly the case a buffer bug lives in.
    #[test]
    fn just_under_a_boundary_still_looks_under() {
        ver(64, |w| size(w, 4 * 1024 * 1024 - 1), "3.9 MiB (4194303)");
        ver(64, |w| size(w, 4 * 1024 * 1024), "4.0 MiB (4194304)");
    }

    /// ** THE OFFSET INSIDE THE PAGE IS THE BUG, PRINTED.
    ///
    /// This is the real address from the DOOM failure of 2026-08-11. The line
    /// used to say `1074388988` and the day it cost was spent discovering that
    /// it means offset 4092 -- eight bytes from there cross the page.
    #[test]
    fn an_address_near_the_page_edge_says_so() {
        ver(64, |w| address(w, 0x4009_DFFC), "0x4009DFFC +4092/4096");
        ver(64, |w| address(w, 0x4009_D000), "0x4009D000 +0/4096");
        // And in the middle of a page it stays quiet: a line that always says
        // something is a line nobody reads.
        ver(64, |w| address(w, 0x4009_D800), "0x4009D800");
    }

    /// ** A MAC PRINTED BACKWARDS IS PLAUSIBLE, WHICH IS WHY THIS TEST EXISTS.
    ///
    /// Six bytes that are neither zeros nor ones look like a real address. The
    /// mismatch against Windows would be blamed on the BAR, which would be
    /// correct at the time.
    #[test]
    fn a_mac_reads_in_the_order_it_is_written() {
        ver(64, |w| mac(w, 0x2CF0_5DD9_3CE3), "2C:F0:5D:D9:3C:E3");
        ver(64, |w| mac(w, 0xFFFF_FFFF_FFFF), "FF:FF:FF:FF:FF:FF");
        ver(64, |w| mac(w, 0), "00:00:00:00:00:00");
    }

    /// ** HEX HIDES THE ONLY THING A BITFIELD SAYS.
    ///
    /// `0x0B` of PHYstatus is three facts at once: link up, 100 Mbps, full
    /// duplex. In binary the three are visible without a manual.
    #[test]
    fn a_bitfield_shows_which_bits_are_set() {
        ver(64, |w| bits(w, 0x0B), "0b1011");
        // ** Whole nibbles, including the leading zeros of the top group.
        //
        // The first version of this test expected `0b1_0011` and the code was
        // right: groups that line up between one reading and the next are worth
        // four characters, because a bitfield gets compared against the PREVIOUS
        // reading of the same register more often than it gets read alone.
        ver(64, |w| bits(w, 0x13), "0b0001_0011");
        ver(64, |w| bits(w, 0), "0b0");
    }

    /// ** A DIAGNOSTIC THAT OVERRUNS ITS BUFFER TAKES DOWN WHAT IT WAS
    /// ** DIAGNOSING.
    ///
    /// CABINA runs inside the fault handler. Truncating is the only acceptable
    /// behaviour here -- not wrapping, and above all not panicking.
    #[test]
    fn writing_past_the_end_truncates_and_does_not_panic() {
        ver(5, |w| size(w, 4_196_020), "4.0 M");
        ver(4, |w| mac(w, 0x2CF0_5DD9_3CE3), "2C:F");
        ver(2, |w| bits(w, 0xFF), "0b");
    }

    /// `Fmt::Raw` has to keep printing exactly as it did before any of this
    /// existed, because two hundred call sites still use it.
    #[test]
    fn raw_hex_is_unchanged() {
        ver(64, |w| w.hex_min(0x800), "800");
        ver(64, |w| w.hex_min(0), "0");
        ver(64, |w| w.hex_min(0x4009DFFC), "4009DFFC");
    }
}
