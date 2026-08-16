//! `dynobj` -- the shape of a DYNAMICALLY TYPED value, as a contract.
//!
//! This is the first piece of `docs/PYTHON_MAESTRO.md`, and it is deliberately
//! not called `python`.
//!
//! # Why this is not "the Python module"
//!
//! It is the same call already decided for packed decimal: `bmo_lower::packed`
//! holds BCD because *packing is a REPRESENTATION, not language semantics* --
//! COBOL's `COMP-3`, Ada Annex F `Decimal` and PL/I `FIXED DECIMAL` all want the
//! same nibbles. An object header is the same kind of thing:
//!
//! ```text
//!    what lives here      the header, the immortal bit, the slot numbers
//!    what does NOT        what `a + b` MEANS when a is str and b is int
//! ```
//!
//! The second one is Python semantics and stays in `toolchain/lang/python`.
//!
//! # Why it is not inside `runtime/`
//!
//! `runtime/` already exists here with `TypeRegistry`, `VTableStore` and
//! `LangBridge` -- but it is a cross-language *interface* registry, a different
//! job, and its only callers today are its own tests.
//!
//! ** And it could not serve this purpose anyway: `VTableEntry` is
//! `Option<extern "C" fn()>`, a raw function pointer. See the next section for
//! why a raw pointer cannot appear in an object that gets lent.
//!
//! # ** THE CONSTRAINT THAT SHAPES EVERYTHING HERE
//!
//! The point of this contract is that the immutable part of a program -- code,
//! types, constant pool -- is **lent between processes** with `MEM_OP_OFRECER`
//! instead of copied. Two facts about BMO make that demand a specific shape:
//!
//! 1. **`loan::take` maps at an address decided by the SLOT.** The same lent
//!    page lands at a *different* virtual address in each process. Therefore a
//!    shared object **cannot hold a pointer to another shared object**. It holds
//!    an INDEX. This is the same "offsets, not pointers" rule already written
//!    for the bytecode section, and here it is not a preference: a pointer is
//!    simply wrong.
//!
//! 2. **A lent page can be READ-ONLY.** So a reference count in it must not be
//!    written -- and "not written" is stronger than "wasted work": the store
//!    would fault. That is why [`header::may_write`] exists as its own question,
//!    separate from [`header::retain`].
//!
//! ** Both of these are decisions that CANNOT be retrofitted. CPython learned
//! the second one the hard way: immortal objects (PEP 683) arrived in 3.12,
//! years after the header was fixed, and cost a release cycle -- because until
//! then every `fork()`ed child dirtied every shared page just by *reading*
//! objects. Deciding it on day one is the entire reason this module is written
//! before a single line of interpreter.
//!
//! # What is here and what is not
//!
//! The rule agreed for this work is *the CONTRACT may be complete, the
//! IMPLEMENTATION is the seed* -- the same way `SectionKind::Resources = 0x0B`
//! sat declared and empty from the day BEF was designed until something needed
//! it.
//!
//! ```text
//!    header    the sixteen bytes every dynamic object starts with
//!    slots     the numbered operations of a type, like TASK_OP_*
//! ```
//!
//! Not here yet, and each has its reason:
//!
//! - **The type table entry** (the BEF `Tipos` section format). Next increment;
//!   it needs the slot numbers below to be settled first.
//! - **The allocator.** It is Ring 3 code over `KIND_MEMORIA`, not a contract.
//! - **Anything that runs.** This crate is tested on the host and holds no
//!   behaviour: it is bytes and numbers.

pub mod header;
pub mod slots;

pub use header::{DynHeader, DynVarHeader};
