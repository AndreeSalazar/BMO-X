//! `dynobj::slots` -- the numbered operations of a type.
//!
//! A dynamic type is a table of function slots, and a slot number is the whole
//! dispatch mechanism: `a + b` is *read slot 0x10 of a's type and call it*.
//!
//! ** This is the shape Eddi asked for -- `OP_ADD = 0x10` -- put where it costs
//! nothing. The number indexes a vtable in user memory, so the dispatch is a
//! `call [rax+N*8]`, not a `syscall`. The measurement on 2026-08-16 is why:
//! a door costs ~2570 cycles and a call ~20, and `a + b` is the innermost
//! operation of the language. See `docs/maestro/PYTHON_MAESTRO.md` section 4b.
//!
//! It is the same table-shaped growth as `TASK_OP_*` and `sem-asm`: adding an
//! operation is a row, not a redesign.
//!
//! # Why the numbers are grouped, and why there are gaps
//!
//! The families are spaced so a new operation lands next to its relatives
//! instead of at the end. A table where `OP_SUB` is 0x11 and `OP_MOD` is 0x37
//! because that is where there was room is a table nobody can read.
//!
//! ```text
//!    0x00..0x0F   life        create, destroy, trace
//!    0x10..0x2F   arithmetic  the operators
//!    0x30..0x3F   comparison
//!    0x40..0x4F   attributes
//!    0x50..0x5F   containers  length, index, iteration
//!    0x60..0x6F   call and conversion
//! ```

/// How many slots a type table reserves. Gaps included: a type is a fixed-size
/// array, so a gap costs eight bytes and a renumbering costs every binary ever
/// produced.
pub const SLOT_COUNT: usize = 0x70;

// -- 0x00..0x0F  life ------------------------------------------------------

/// Free this object. The only slot every type must fill.
pub const OP_DEALLOC: u32 = 0x00;

/// Walk the references this object holds, for the cycle collector.
///
/// Only types with [`super::header::FLAG_TRACKED`] fill it. Reference counting
/// alone cannot free `a.b = a`, and something has to trace.
pub const OP_TRAVERSE: u32 = 0x01;

/// Drop the references this object holds, without freeing the object itself.
/// The cycle collector needs the two steps separate: it breaks a cycle first
/// and lets the counts do the freeing.
pub const OP_CLEAR: u32 = 0x02;

// -- 0x10..0x2F  arithmetic ------------------------------------------------

pub const OP_ADD: u32 = 0x10;
pub const OP_SUB: u32 = 0x11;
pub const OP_MUL: u32 = 0x12;
/// True division: in Python 3 this is the one that returns a float.
pub const OP_DIV: u32 = 0x13;
/// Floor division, `//`. A separate slot and not a flag on `OP_DIV`, because
/// for integers they are genuinely different operations and folding them would
/// put a branch in the hottest path of the interpreter.
pub const OP_FLOORDIV: u32 = 0x14;
pub const OP_MOD: u32 = 0x15;
pub const OP_POW: u32 = 0x16;
pub const OP_NEG: u32 = 0x17;
pub const OP_ABS: u32 = 0x18;

pub const OP_AND: u32 = 0x20;
pub const OP_OR: u32 = 0x21;
pub const OP_XOR: u32 = 0x22;
pub const OP_NOT: u32 = 0x23;
pub const OP_SHL: u32 = 0x24;
pub const OP_SHR: u32 = 0x25;

// -- 0x30..0x3F  comparison ------------------------------------------------

/// One slot for all six comparisons, with the operator as an argument.
///
/// This is CPython's `tp_richcompare` and the reason is worth keeping: six
/// slots would let a type answer `<` and not `>=`, and a type that is
/// inconsistent with itself is a bug that only shows up when something sorts.
pub const OP_COMPARE: u32 = 0x30;
/// The hash. Must agree with equality or every dict built from this type is
/// quietly wrong -- the oldest trap in this shape of code.
pub const OP_HASH: u32 = 0x31;
/// Truthiness: what `if x:` asks.
pub const OP_BOOL: u32 = 0x32;

// -- 0x40..0x4F  attributes ------------------------------------------------

pub const OP_GETATTR: u32 = 0x40;
pub const OP_SETATTR: u32 = 0x41;
pub const OP_DELATTR: u32 = 0x42;

// -- 0x50..0x5F  containers ------------------------------------------------

/// How many elements. Not how many bytes -- see `DynVarHeader::count`.
pub const OP_LEN: u32 = 0x50;
pub const OP_GETITEM: u32 = 0x51;
pub const OP_SETITEM: u32 = 0x52;
pub const OP_DELITEM: u32 = 0x53;
pub const OP_CONTAINS: u32 = 0x54;
/// Make an iterator over this object.
pub const OP_ITER: u32 = 0x55;
/// Advance an iterator. Separate from [`OP_ITER`] because an iterator is itself
/// an object: `iter(x)` and `next(it)` are asked of different things.
pub const OP_NEXT: u32 = 0x56;

// -- 0x60..0x6F  call and conversion ---------------------------------------

pub const OP_CALL: u32 = 0x60;
/// The text a human should see, `repr`.
pub const OP_REPR: u32 = 0x61;
/// The text a program should print, `str`. Distinct from [`OP_REPR`] on
/// purpose: `repr("a")` is `'a'` with the quotes and `str("a")` is not, and a
/// language that confuses them cannot round-trip its own output.
pub const OP_STR: u32 = 0x62;
pub const OP_INT: u32 = 0x63;
pub const OP_FLOAT: u32 = 0x64;

/// The name of a slot, or `None` if the number is not assigned.
///
/// It exists for the same reason `syscalls::surface::name()` does: a numbered
/// contract with no names is unreadable in a dump, and an unassigned number
/// must answer `None` rather than something plausible -- a name on a reserved
/// number makes a trace of a broken program look correct.
pub const fn name(slot: u32) -> Option<&'static str> {
    match slot {
        OP_DEALLOC => Some("dealloc"),
        OP_TRAVERSE => Some("traverse"),
        OP_CLEAR => Some("clear"),
        OP_ADD => Some("add"),
        OP_SUB => Some("sub"),
        OP_MUL => Some("mul"),
        OP_DIV => Some("div"),
        OP_FLOORDIV => Some("floordiv"),
        OP_MOD => Some("mod"),
        OP_POW => Some("pow"),
        OP_NEG => Some("neg"),
        OP_ABS => Some("abs"),
        OP_AND => Some("and"),
        OP_OR => Some("or"),
        OP_XOR => Some("xor"),
        OP_NOT => Some("not"),
        OP_SHL => Some("shl"),
        OP_SHR => Some("shr"),
        OP_COMPARE => Some("compare"),
        OP_HASH => Some("hash"),
        OP_BOOL => Some("bool"),
        OP_GETATTR => Some("getattr"),
        OP_SETATTR => Some("setattr"),
        OP_DELATTR => Some("delattr"),
        OP_LEN => Some("len"),
        OP_GETITEM => Some("getitem"),
        OP_SETITEM => Some("setitem"),
        OP_DELITEM => Some("delitem"),
        OP_CONTAINS => Some("contains"),
        OP_ITER => Some("iter"),
        OP_NEXT => Some("next"),
        OP_CALL => Some("call"),
        OP_REPR => Some("repr"),
        OP_STR => Some("str"),
        OP_INT => Some("int"),
        OP_FLOAT => Some("float"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every assigned slot, so the tests below cannot be fooled by a constant
    /// that exists and was forgotten here.
    const ASSIGNED: &[u32] = &[
        OP_DEALLOC, OP_TRAVERSE, OP_CLEAR,
        OP_ADD, OP_SUB, OP_MUL, OP_DIV, OP_FLOORDIV, OP_MOD, OP_POW, OP_NEG, OP_ABS,
        OP_AND, OP_OR, OP_XOR, OP_NOT, OP_SHL, OP_SHR,
        OP_COMPARE, OP_HASH, OP_BOOL,
        OP_GETATTR, OP_SETATTR, OP_DELATTR,
        OP_LEN, OP_GETITEM, OP_SETITEM, OP_DELITEM, OP_CONTAINS, OP_ITER, OP_NEXT,
        OP_CALL, OP_REPR, OP_STR, OP_INT, OP_FLOAT,
    ];

    /// ** NO TWO SLOTS SHARE A NUMBER.
    ///
    /// This is not hygiene, it is a bug that already happened in this codebase:
    /// `INFO_CPU_HZ_REAL` was written at `0x1E`, which was already `INFO_FUGAS`.
    /// **Two constants with the same number do not fail to compile: they give a
    /// panel showing somebody else's data.** Here it would be worse -- calling
    /// the wrong slot means calling the wrong function with the wrong
    /// arguments.
    #[test]
    fn no_two_slots_share_a_number() {
        for (i, a) in ASSIGNED.iter().enumerate() {
            for b in ASSIGNED.iter().skip(i + 1) {
                assert_ne!(a, b, "slot {a:#x} is assigned twice");
            }
        }
    }

    /// Every assigned slot has a name, and every name belongs to an assigned
    /// slot. The second half is what stops a name outliving its constant.
    #[test]
    fn every_assigned_slot_has_a_name() {
        for s in ASSIGNED {
            assert!(name(*s).is_some(), "slot {s:#x} has no name");
        }
        let mut named = 0;
        let mut n = 0;
        while n < SLOT_COUNT as u32 {
            if name(n).is_some() {
                named += 1;
            }
            n += 1;
        }
        assert_eq!(named, ASSIGNED.len(), "a name exists for an unassigned slot");
    }

    /// A reserved number answers `None`. Giving it a name would make a trace of
    /// a broken program read as correct -- the same reason syscall number 1
    /// stays nameless after `CHANNEL_KICK` was retired.
    #[test]
    fn an_unassigned_number_has_no_name() {
        assert_eq!(name(0x0F), None);
        assert_eq!(name(0x2F), None);
        assert_eq!(name(0x6F), None);
        assert_eq!(name(0xFFFF), None);
    }

    /// Everything fits in the table. A slot past the end would index off it.
    #[test]
    fn every_slot_fits_in_the_table() {
        for s in ASSIGNED {
            assert!((*s as usize) < SLOT_COUNT, "slot {s:#x} is past SLOT_COUNT");
        }
    }

    /// The families stay in their ranges. If this fails somebody put an
    /// operation where its relatives are not, and the next person to add one
    /// will follow the wrong example.
    #[test]
    fn each_family_stays_in_its_range() {
        assert!(OP_DEALLOC < 0x10 && OP_TRAVERSE < 0x10 && OP_CLEAR < 0x10);
        for s in [OP_ADD, OP_SUB, OP_MUL, OP_DIV, OP_FLOORDIV, OP_MOD, OP_POW,
                  OP_NEG, OP_ABS, OP_AND, OP_OR, OP_XOR, OP_NOT, OP_SHL, OP_SHR] {
            assert!((0x10..0x30).contains(&s), "{s:#x} is not arithmetic");
        }
        for s in [OP_COMPARE, OP_HASH, OP_BOOL] {
            assert!((0x30..0x40).contains(&s));
        }
        for s in [OP_GETATTR, OP_SETATTR, OP_DELATTR] {
            assert!((0x40..0x50).contains(&s));
        }
        for s in [OP_LEN, OP_GETITEM, OP_SETITEM, OP_DELITEM, OP_CONTAINS,
                  OP_ITER, OP_NEXT] {
            assert!((0x50..0x60).contains(&s));
        }
        for s in [OP_CALL, OP_REPR, OP_STR, OP_INT, OP_FLOAT] {
            assert!((0x60..0x70).contains(&s));
        }
    }
}
