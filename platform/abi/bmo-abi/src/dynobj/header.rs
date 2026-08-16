//! `dynobj::header` -- the sixteen bytes every dynamic object starts with.
//!
//! ```text
//!    +0   refs        u64   reference count; bit 63 = IMMORTAL
//!    +8   type_index  u32   index into the type table, NOT a pointer
//!    +12  flags       u32
//!                           = 16 bytes, aligned to 8
//! ```
//!
//! Sixteen bytes is the same size CPython's `PyObject` costs, which is worth
//! saying plainly: **this contract does not make dynamic objects cheaper.** A
//! three-element list is around two hundred bytes in any language shaped like
//! this, and twelve in C. That is the price of the language, not of the system,
//! and no kernel removes it.
//!
//! What the layout does buy is the two things BMO can do and CPython cannot:
//! the object can live in a page lent to another process, and it can live in a
//! page that is never written.

use crate::types::disposicion::Disposicion;

/// Bit 63 of the reference count. Set = this object is IMMORTAL.
///
/// ## Why the high bit and not a sentinel value
///
/// CPython uses a magic count (`_Py_IMMORTAL_REFCNT`) and compares against it.
/// A sign test is the same one instruction, and it buys two things a sentinel
/// does not:
///
/// - **Any value with the bit set is immortal**, so the constant pool writer
///   does not have to agree with the runtime on one exact number. One less
///   thing two sides can disagree about, which is the failure mode this
///   codebase pays for most often.
/// - **A count that goes "negative" without the bit is corruption**, and it is
///   visible in a dump instead of looking like a plausible large number. That
///   is the giant-needle rule: a broken value must not look like good data.
pub const IMMORTAL: u64 = 1 << 63;

/// The count stored in an immortal object.
///
/// It is `IMMORTAL` alone and not `IMMORTAL | 1`: the low bits carry no meaning
/// once the object can never die, and leaving them zero makes an immortal
/// header trivially recognisable by eye in a hex dump.
pub const IMMORTAL_REFS: u64 = IMMORTAL;

/// This object lives in memory taken with `PRESTADO_OP_*` -- someone else's.
///
/// It is not the same question as [`IMMORTAL`] and mixing them would be a bug:
/// an object can be immortal and private (an interned string this process made)
/// or lent and mortal is **forbidden**, which is exactly what the debug check
/// in [`is_consistent`] exists to catch.
pub const FLAG_LENT: u32 = 1 << 0;

/// The cycle collector walks this object.
///
/// Only containers need it. Reference counting alone cannot free `a.b = a`, so
/// something has to trace -- but tracing an integer is pure waste, and this bit
/// is what says which is which.
pub const FLAG_TRACKED: u32 = 1 << 1;

/// The header every dynamic object starts with.
///
/// `repr(C)` is load-bearing: the interpreter is C (see `PYTHON_MAESTRO.md`
/// section 4b), the constant pool writer is Rust, and the emulator reads the
/// same bytes. Three readers of one format is the reason it is written down.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynHeader {
    /// Reference count. Bit 63 set means immortal -- see [`IMMORTAL`].
    pub refs: u64,
    /// **Index into the type table, never a pointer.**
    ///
    /// A pointer would be wrong, not merely slower: a lent page lands at a
    /// different virtual address in every process, so a pointer written by the
    /// producer means nothing to the consumer. See the module header.
    pub type_index: u32,
    /// [`FLAG_LENT`], [`FLAG_TRACKED`], rest reserved and must be zero.
    pub flags: u32,
}

/// A dynamic object whose size is known only at runtime: list, str, tuple, int.
///
/// Kept as a separate type rather than an optional field, because whether an
/// object has a count is a property of its TYPE and never changes. A single
/// struct with a sometimes-meaningless field is how a reader ends up trusting
/// a number that was never set.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynVarHeader {
    pub base: DynHeader,
    /// How many elements, NOT how many bytes. The element width comes from the
    /// type, and confusing the two is the oldest bug in this shape of code.
    pub count: u64,
}

impl DynHeader {
    /// A normal, private, mortal object with one reference.
    pub const fn new(type_index: u32) -> Self {
        Self { refs: 1, type_index, flags: 0 }
    }

    /// An object that can never die: an interned string, a small integer, an
    /// entry of the constant pool.
    pub const fn immortal(type_index: u32) -> Self {
        Self { refs: IMMORTAL_REFS, type_index, flags: 0 }
    }

    /// An immortal object living in a page lent by another process.
    pub const fn lent(type_index: u32) -> Self {
        Self { refs: IMMORTAL_REFS, type_index, flags: FLAG_LENT }
    }

    pub const fn is_immortal(&self) -> bool {
        is_immortal(self.refs)
    }

    pub const fn is_lent(&self) -> bool {
        self.flags & FLAG_LENT != 0
    }

    pub const fn is_tracked(&self) -> bool {
        self.flags & FLAG_TRACKED != 0
    }

    /// **Whether the count may be STORED to at all.**
    ///
    /// This is not an optimisation and it is not the same question as "would
    /// the value change". A lent page can be mapped read-only, so the store
    /// itself faults. Every retain and release must ask this FIRST.
    pub const fn may_write(&self) -> bool {
        may_write(self.refs)
    }

    /// An invariant that must hold for every header ever built.
    ///
    /// **A lent object that is not immortal is forbidden** -- it would be an
    /// object in someone else's page whose count this process is expected to
    /// change, which is the exact bug the whole lending design exists to avoid.
    /// It cannot be expressed in the type system without making the struct
    /// non-`repr(C)`, so it is expressed as a question with a test behind it.
    pub const fn is_consistent(&self) -> bool {
        if self.is_lent() && !self.is_immortal() {
            return false;
        }
        // Reserved bits are reserved: a set bit here means a producer wrote a
        // flag this reader does not know, and guessing is worse than refusing.
        self.flags & !(FLAG_LENT | FLAG_TRACKED) == 0
    }
}

/// Is this count immortal? One sign test.
pub const fn is_immortal(refs: u64) -> bool {
    refs & IMMORTAL != 0
}

/// May a store to this count be performed? See [`DynHeader::may_write`].
pub const fn may_write(refs: u64) -> bool {
    !is_immortal(refs)
}

/// The count after taking one more reference.
///
/// An immortal count comes back unchanged -- but note that the CALLER must
/// still check [`may_write`] before storing it. Returning the same value is
/// not enough: writing the same bytes to a read-only page still faults.
pub const fn retain(refs: u64) -> u64 {
    if is_immortal(refs) {
        return refs;
    }
    refs + 1
}

/// The count after dropping one reference. Same caveat as [`retain`].
///
/// Saturating at zero on purpose: a release below zero is a bug in the caller,
/// and wrapping to `u64::MAX` would set bit 63 -- turning a double-free into a
/// silently immortal object, which is a leak that never reports itself. It
/// would be the `unwrap_or(0)` failure in a new costume.
pub const fn release(refs: u64) -> u64 {
    if is_immortal(refs) {
        return refs;
    }
    if refs == 0 {
        return 0;
    }
    refs - 1
}

/// Does dropping this reference destroy the object?
pub const fn is_last(refs: u64) -> bool {
    !is_immortal(refs) && refs == 1
}

/// The size of [`DynHeader`], computed by the ABI's own layout rule.
///
/// It is derived and not written as `16`, and that is the point: if the shared
/// aggregate-layout rule in `types::disposicion` ever changes, this moves with
/// it and the test below fails loudly instead of two sides quietly disagreeing.
pub fn header_size() -> u32 {
    let mut d = Disposicion::nueva();
    d.coloca(8, 8); // refs
    d.coloca(4, 4); // type_index
    d.coloca(4, 4); // flags
    d.total()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    /// ** THE LAYOUT IS THE CONTRACT.
    ///
    /// Three readers will parse these bytes: the interpreter in BMO C, the
    /// constant-pool writer in Rust, and the emulator. Nothing in the build can
    /// check that they agree, so the numbers are pinned here by hand.
    #[test]
    fn the_header_is_sixteen_bytes_aligned_to_eight() {
        assert_eq!(size_of::<DynHeader>(), 16);
        assert_eq!(align_of::<DynHeader>(), 8);
        assert_eq!(size_of::<DynVarHeader>(), 24);
        assert_eq!(align_of::<DynVarHeader>(), 8);
    }

    /// And the same size, reached the OTHER way: through the shared layout
    /// rule that C and C++ already use. Two roads to one number.
    #[test]
    fn the_abi_layout_rule_agrees_with_the_struct() {
        assert_eq!(header_size() as usize, size_of::<DynHeader>());
    }

    /// ** THE TEST THAT JUSTIFIES THE WHOLE MODULE.
    ///
    /// An immortal object's count must never move, no matter how many times it
    /// is retained or released. If this ever fails, the constant pool cannot be
    /// lent -- every read would dirty a shared page and the entire design in
    /// `PYTHON_MAESTRO.md` section 4b collapses.
    #[test]
    fn an_immortal_count_never_moves() {
        let mut r = IMMORTAL_REFS;
        for _ in 0..1000 {
            r = retain(r);
            r = release(r);
        }
        assert_eq!(r, IMMORTAL_REFS);
        assert!(is_immortal(r));
        assert!(!is_last(r), "an immortal object is never the last reference");
    }

    /// And the mirror: a normal count does move. Without this, a header that
    /// froze everything would pass the test above.
    #[test]
    fn a_normal_count_does_move() {
        let mut r = 1u64;
        r = retain(r);
        assert_eq!(r, 2);
        assert!(!is_last(r));
        r = release(r);
        assert_eq!(r, 1);
        assert!(is_last(r), "one reference left: the next release frees it");
    }

    /// ** The store is a SEPARATE question from the value.
    ///
    /// A lent page can be read-only, so writing the same bytes back still
    /// faults. `retain` returning an unchanged value is not permission.
    #[test]
    fn immortal_forbids_the_store_not_just_the_change() {
        let obj = DynHeader::immortal(7);
        assert!(!obj.may_write());
        assert_eq!(retain(obj.refs), obj.refs, "value unchanged...");
        assert!(!may_write(obj.refs), "...and the store is still forbidden");

        let normal = DynHeader::new(7);
        assert!(normal.may_write());
    }

    /// A double release must not wrap. `0 - 1` would be `u64::MAX`, which has
    /// bit 63 set -- so a double-free would turn the object IMMORTAL and leak
    /// it forever without a single complaint.
    #[test]
    fn releasing_past_zero_does_not_forge_an_immortal() {
        let r = release(release(release(1)));
        assert_eq!(r, 0);
        assert!(!is_immortal(r), "a double free must never mint an immortal");
    }

    /// Lent implies immortal. The forbidden combination is an object in someone
    /// else's page whose count we are expected to move.
    #[test]
    fn lent_and_mortal_is_not_a_legal_header() {
        assert!(DynHeader::lent(3).is_consistent());
        assert!(DynHeader::new(3).is_consistent());
        assert!(DynHeader::immortal(3).is_consistent());

        let bad = DynHeader { refs: 1, type_index: 3, flags: FLAG_LENT };
        assert!(!bad.is_consistent(), "lent + mortal has to be refused");
    }

    /// A flag this reader does not know is refused, not ignored. A producer
    /// from a newer toolchain must fail loudly, not be half-understood.
    #[test]
    fn an_unknown_flag_is_refused() {
        let future = DynHeader { refs: 1, type_index: 0, flags: 1 << 31 };
        assert!(!future.is_consistent());
    }

    /// The type reference is an INDEX, and this pins its width. If it ever
    /// became pointer-sized, a lent object would stop being shareable and
    /// nothing else in the build would notice.
    #[test]
    fn the_type_reference_is_an_index_not_a_pointer() {
        assert_eq!(size_of::<u32>(), 4);
        let h = DynHeader::new(0xABCD);
        assert_eq!(h.type_index, 0xABCD);
        // 16 bytes total with an 8-byte count leaves exactly 8 for the type and
        // the flags together: a pointer-sized type index would not fit beside
        // the flags without growing the header.
        assert_eq!(size_of::<DynHeader>(), 8 + 4 + 4);
    }
}
