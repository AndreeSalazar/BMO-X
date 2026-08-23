//! **The double click, and it is measured in CYCLES.**
//!
//! Two grids on this desktop open things with a double click -- the icon grid
//! (`scene::launcher`) and the ESTRATOS file grid (`scene::data`) -- and until
//! today each one carried its own copy of the rule and its own constant. Same
//! gesture, written twice, and a gesture the hand has to learn once.
//!
//! # * WHY THIS FILE EXISTS, AND IT IS NOT THE DUPLICATION
//!
//! Both copies counted **frames**:
//!
//! ```text
//!    pub const DOBLE_CLIC: u32 = 24;   // "a los ~60 por segundo, unos 400 ms"
//! ```
//!
//! That comment carried two claims, and **neither one holds**:
//!
//! 1. **The desktop does not turn at 60 per second.** The counter it compared
//!    against goes up **once per pass of the main loop**, and that loop has no
//!    pacing: it ends in `yield_screen()` and comes straight back. With nothing
//!    to paint a pass is a handful of syscalls, so the desktop spins thousands
//!    of times a second -- and twenty-four passes are then **milliseconds**, not
//!    four hundred. A human double click (~200 ms) cannot land inside a window
//!    that short: the second click always reads as a first one, the icon only
//!    ever gets selected, and **nothing launches**.
//!
//! 2. **There IS a fine clock in Ring 3**, and it was already being used three
//!    hundred lines from that constant: `main.rs` builds a thirty-second budget
//!    for `lend_screen` out of `bmo::ciclos()` and `INFO_TSC_HZ`. The stated
//!    reason for counting frames -- *"en Ring 3 no hay reloj mas fino que el
//!    segundo"* -- was not true when it was written.
//!
//! ** So the fix is not a bigger number. A number calibrated against a rhythm
//! nobody measures is a guess wearing a unit. Four hundred milliseconds is what
//! the hand knows, so four hundred milliseconds is what gets counted -- and the
//! loop can then run as fast as it likes without moving the gesture.
//!
//! The rhythm itself is worth knowing anyway, and now it is measured: see
//! `Tick::pulse` and the `escritorio` row of F7.

use bmo_userland as bmo;

/// How far apart the two clicks may be, in **milliseconds**.
///
/// Four hundred is what Windows and the Mac have used for decades, so it is
/// what a hand arriving from either one already has. It is written in
/// milliseconds and converted at the point of use on purpose: a constant in
/// cycles would be a number about this machine, and the same hand is going to
/// use the next one.
pub const MS: u64 = 400;

/// [`MS`] turned into cycles of THIS processor.
///
/// # Why this asks the kernel every time instead of caching
///
/// Because a click is a human event: a few per second at the very most, and one
/// syscall costs 969 cycles (`docs/componente/LA_PUERTA_POR_DENTRO.md`). Caching
/// it would buy nothing measurable and would cost a `static mut` in a process
/// that has managed to stay without one.
///
/// # And what happens when the machine has no clock to offer
///
/// `INFO_TSC_HZ` answers `0` when the scheduler could not calibrate the TSC. No
/// frequency means no way to turn milliseconds into cycles, so the window opens
/// all the way: the second click on the same item always opens it, however long
/// it took. **That is the honest fallback** -- a mouse without a clock can still
/// tell "twice on the same thing" apart from "once", and a machine that cannot
/// time a gesture should still be able to open a file.
fn window() -> u64 {
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    if hz == 0 {
        return u64::MAX;
    }
    // `hz / 1000` first: at 4,5 GHz that is 4,5 million, and multiplying by 400
    // stays far inside 64 bits. The other order overflows nothing either, but
    // this one is the one that reads as "cycles per millisecond".
    (hz / 1_000) * MS
}

/// **The open half of a double click.** One per grid.
///
/// It holds no geometry and no selection: which item is highlighted, and what
/// opening one means, belong to the grid. What lives here is only the part both
/// grids answer the same way -- *was this the second click?*
pub struct DoubleClick {
    /// When the click that opened the gesture happened, in cycles. `0` means
    /// **no gesture is open**.
    ///
    /// Zero works as the sentinel because the TSC has been counting since the
    /// machine powered on: by the time Ring 3 exists it is astronomically far
    /// from zero, and it never goes back.
    last: u64,
    /// Which item that click landed on. A double click has to be on the SAME
    /// thing, or crossing a grid quickly would open whatever was under the
    /// second click.
    item: usize,
}

impl DoubleClick {
    pub const fn new() -> Self {
        Self { last: 0, item: usize::MAX }
    }

    /// **A click landed on `item`.** `true` when it is the SECOND of a double.
    ///
    /// ** The second click CLOSES the gesture (`last = 0`). Without that, three
    /// clicks in a row would be two openings: the third would still fall inside
    /// the window opened by the second and open the thing again, which is not
    /// what any desktop does.
    pub fn hit(&mut self, item: usize) -> bool {
        let now = bmo::ciclos();
        let double =
            self.last != 0 && self.item == item && now.wrapping_sub(self.last) <= window();
        self.last = if double { 0 } else { now };
        self.item = item;
        double
    }

    /// Forget the open gesture. Called by whoever clicks OUTSIDE the grid: a
    /// click on the background is not the first half of anything.
    pub fn clear(&mut self) {
        self.last = 0;
    }
}
