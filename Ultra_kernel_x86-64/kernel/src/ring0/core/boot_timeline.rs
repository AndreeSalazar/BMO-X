//! # BOOT TIMELINE -- where the boot time actually goes
//!
//! [carril]  VERDE     cuenta a donde se fue el tiempo de arranque
//!
//! ## Why this exists
//!
//! The owner asked to make boot faster, and pointed at the kernel log: *"the
//! text part, optimise it to boot FASTER, in the background"*. Reasonable
//! guess, and the numbers say otherwise:
//!
//! ```text
//!    [  47 ms]  BMO-X operational
//!    [1164 ms]  Ring 3 entry painted
//!    [1210 ms]  first complete frame
//! ```
//!
//! **The kernel is up in 47 ms.** Forty lines of bitmap glyphs are microseconds
//! -- a full 1920x1080 clear is ~8 MB and it happens once. The missing 1.1 s is
//! somewhere else, and nobody has ever measured WHERE.
//!
//! ## The rule of the house, applied to itself
//!
//! Today alone, three panels were caught telling the state of two weeks ago,
//! and every fix started the same way: **measure, do not patch**. Optimising a
//! boot without a breakdown is exactly the mistake this project keeps writing
//! down and then almost repeating.
//!
//! So: no optimisation in this commit. A ruler.
//!
//! ## What it reports, and why it is the DELTA that matters
//!
//! A list of absolute stamps forces the reader to subtract. What answers *"what
//! do I attack first?"* is the **cost of each stage**, so that is the column
//! that is printed, sorted by the clock but readable by size.
//!
//! ```text
//!    boot timeline
//!      mm + scheduler          6 ms      6 ms
//!      pci + storage scan     18 ms     24 ms
//!      usb enumeration       842 ms    866 ms   <- the one that pays
//!      disk + partitions      31 ms    897 ms
//!      ring 3 handover       210 ms   1107 ms
//! ```
//!
//! [!] The stamps cost **one `rdtsc` each**: a register read, no serialising
//! instruction, no lock. A ruler that changes what it measures is not a ruler.
//!
//! ## Scope, said out loud
//!
//! This measures the KERNEL side, from the moment `BootContext` is validated to
//! the handover to Ring 3. What Ring 3 does with its first frame is the
//! compositor's own business and it already reports it.

use crate::ring0::task::scheduler;

/// How many stages fit. Sixteen is more than the boot has and small enough to
/// live in `.bss` without thinking about it: 16 * 24 B = 384 B.
const MAX_STAGES: usize = 16;

/// A stage name is a `&'static str` on purpose -- **no copying, no buffer**.
/// Every call site is a literal in the kernel image, so the pointer is valid
/// forever and the ruler never allocates.
struct Stage {
    name: &'static str,
    tsc: u64,
}

static mut STAGES: [Stage; MAX_STAGES] = [Stage { name: "", tsc: 0 }; MAX_STAGES];
static mut COUNT: usize = 0;
/// The zero of the clock. Set by [`start`].
static mut ORIGIN: u64 = 0;

/// Starts the clock. Called once, as early as the kernel can read the TSC.
pub fn start() {
    unsafe {
        ORIGIN = scheduler::rdtsc();
        COUNT = 0;
    }
}

/// Marks the END of a stage. The name describes what just FINISHED.
///
/// Naming the end and not the beginning is deliberate: a mark that opens a
/// stage leaves the last one unclosed, and the last one is usually the
/// interesting one.
pub fn mark(name: &'static str) {
    unsafe {
        if COUNT >= MAX_STAGES {
            // Silently dropping would make the report lie by omission. The
            // last slot is overwritten and keeps the name, so the report shows
            // the last stage instead of hiding it.
            STAGES[MAX_STAGES - 1] = Stage { name, tsc: scheduler::rdtsc() };
            return;
        }
        STAGES[COUNT] = Stage { name, tsc: scheduler::rdtsc() };
        COUNT += 1;
    }
}

/// Milliseconds between two TSC readings, or `None` if the frequency is not
/// known yet.
///
/// [!] Returns `None` instead of zero. A zero would be indistinguishable from
/// "this stage was instant", and a ruler that cannot measure has to say so --
/// otherwise the report shows a boot with no cost anywhere.
fn ms_between(a: u64, b: u64) -> Option<u64> {
    let hz = scheduler::tsc_freq();
    if hz == 0 {
        return None;
    }
    Some(b.wrapping_sub(a) * 1000 / hz)
}

/// Total milliseconds from [`start`] to the last mark.
pub fn total_ms() -> u64 {
    unsafe {
        if COUNT == 0 {
            return 0;
        }
        ms_between(ORIGIN, STAGES[COUNT - 1].tsc).unwrap_or(0)
    }
}

/// Prints the breakdown through `log`, one line per stage.
///
/// The caller passes its own printer so this module does not choose where the
/// report lands: the boot uses the kernel dashboard, a future `boot` command
/// could use the Ring 3 console, and the test bench could use neither.
pub fn report(mut log: impl FnMut(&str)) {
    unsafe {
        if COUNT == 0 {
            log("boot timeline: no stages were marked");
            return;
        }
        if scheduler::tsc_freq() == 0 {
            log("boot timeline: the TSC frequency is unknown, no times");
            return;
        }
        log("boot timeline   (cost / accumulated, ms)");
        let mut previous = ORIGIN;
        for i in 0..COUNT {
            let s = &STAGES[i];
            let cost = ms_between(previous, s.tsc).unwrap_or(0);
            let total = ms_between(ORIGIN, s.tsc).unwrap_or(0);
            let mut line = [b' '; 64];
            let mut n = 2usize;
            // The name, padded so the numbers line up in a column. Columns are
            // what makes the expensive stage findable without reading.
            for (k, b) in s.name.as_bytes().iter().enumerate() {
                if n + k < 26 {
                    line[n + k] = *b;
                }
            }
            n = 26;
            n += write_u64(&mut line[n..], cost);
            while n < 34 {
                line[n] = b' ';
                n += 1;
            }
            n += write_u64(&mut line[n..], total);
            if let Ok(text) = core::str::from_utf8(&line[..n]) {
                log(text);
            }
            previous = s.tsc;
        }
    }
}

/// Right-aligned decimal, no allocation. Returns how many bytes it wrote.
fn write_u64(dst: &mut [u8], mut v: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut n = 0;
    if v == 0 {
        digits[0] = b'0';
        n = 1;
    } else {
        while v > 0 && n < 20 {
            digits[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
    }
    let mut w = 0;
    while w < n && w < dst.len() {
        dst[w] = digits[n - 1 - w];
        w += 1;
    }
    w
}

impl Clone for Stage {
    fn clone(&self) -> Self {
        Stage { name: self.name, tsc: self.tsc }
    }
}
impl Copy for Stage {}
