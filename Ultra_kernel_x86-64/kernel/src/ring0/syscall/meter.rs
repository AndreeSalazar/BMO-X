//! `syscall::meter` -- how many cycles the kernel spends inside a door.
//!
//! # Why this exists
//!
//! On 2026-08-16 `c/coste.bex` measured a door from Ring 3: **2615 cycles
//! minimum**, against 20 for a plain function call. That number decided a real
//! design question (see `docs/PYTHON_MAESTRO.md` section 4b) but it could not
//! answer the next one: **where do those cycles go?**
//!
//! The named suspects were the unconditional `xsave64`/`xrstor64` with
//! `RFBM = -1` and the `iretq`, both in `entry.rs`. Adding them up honestly
//! gives a few hundred cycles, not two thousand -- so the suspicion was
//! **unconfirmed, and acting on it would have been surgery on a guess**, on the
//! exact code that produced the `#GP` in `xrstor` that cost five probes to
//! find.
//!
//! This splits the number in half without changing a single behaviour:
//!
//! ```text
//!    total (medido desde Ring 3)   2615
//!      - lo que tarda `dispatch`   <- lo que mide esto
//!      = lo que tarda el STUB      <- pushes, xsave, xrstor, iretq
//! ```
//!
//! If the stub half is small, the suspects are innocent and the cost is in
//! Rust. If it is nearly all of it, the surgery is justified and there is a
//! number behind it instead of a hypothesis.
//!
//! # It is read as a DELTA, not as an absolute
//!
//! There is no reset operation on purpose. A caller reads the pair before and
//! after its own loop and divides the differences, which measures exactly the
//! population it cares about and cannot be polluted by whatever the machine did
//! while booting. Adding a reset would also add the question *"who is allowed
//! to reset it"*, and nobody needs that answer.
//!
//! [!] Reading the pair costs two doors, and those two are counted too. Over a
//! block of thousands it is under a tenth of a percent; over a block of ten it
//! is the whole measurement.
//!
//! # What it costs, said out loud
//!
//! Two `rdtsc` reads per door, about fifty cycles, plus two stores. That is
//! ~2% of the number being measured, it inflates BOTH sides equally, and it is
//! disclosed here rather than discovered later by someone comparing against a
//! run from before this file existed.
//!
//! # Why `static mut` and not atomics
//!
//! A `lock xadd` is tens of cycles **on the very path being measured**, which
//! would be measuring the thermometer. These are plain volatile counters, the
//! same shape `plat::trap::registrar_publicacion` already uses one line away.
//!
//! [!] Consequence, stated because it will matter: with more than one core awake
//! these counters **undercount** -- two cores writing the same word lose one of
//! the two. They are a meter, not an accountant. The day APs run user code this
//! becomes per-CPU or it becomes wrong.

/// How many doors have been served since boot.
static mut DOORS: u64 = 0;

/// Accumulated TSC ticks spent inside `dispatch`, for those doors.
static mut CYCLES: u64 = 0;

/// Take the reading at the start of a door. Pairs with [`stop`].
#[inline(always)]
pub fn start() -> u64 {
    crate::ring0::task::scheduler::rdtsc()
}

/// Close the reading opened by [`start`] and fold it in.
///
/// `saturating_sub` and not `-`: the TSC is invariant on this CPU, but a value
/// that went backwards would be a fact about the machine, not a reason to
/// underflow into a number near `u64::MAX` that then poisons every average
/// computed from the sum. A lost sample is visible; a huge one is a lie.
#[inline(always)]
pub fn stop(started: u64) {
    let elapsed = crate::ring0::task::scheduler::rdtsc().saturating_sub(started);
    unsafe {
        let n = core::ptr::addr_of_mut!(DOORS);
        n.write_volatile(n.read_volatile().wrapping_add(1));
        let c = core::ptr::addr_of_mut!(CYCLES);
        c.write_volatile(c.read_volatile().wrapping_add(elapsed));
    }
}

/// Doors served since boot.
pub fn doors() -> u64 {
    unsafe { core::ptr::addr_of!(DOORS).read_volatile() }
}

/// Ticks spent inside `dispatch` for those doors.
pub fn cycles() -> u64 {
    unsafe { core::ptr::addr_of!(CYCLES).read_volatile() }
}
