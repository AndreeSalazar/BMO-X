//! Function signature — typed parameter list and return type for function pointers,
//! syscalls, VTable methods, and LangBridge calls.

use super::convention::{CallingConvention, ScalarKind};
use crate::bmo_abi::primitives::{bx_u32, bx_u64};

/// Maximum number of parameters in a function signature.
pub const MAX_PARAMS: usize = 16;

/// Describes a single parameter in a function signature.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ParamDescriptor {
    /// FNV-1a 64-bit hash of the parameter name (0 if unnamed).
    pub name_hash: bx_u64,
    /// Index into TypeRegistry for the parameter's type.
    pub type_id: bx_u32,
    /// If the type is a scalar, its kind (for register assignment).
    pub scalar_kind: ScalarKind,
    /// Reserved.
    pub _pad: u8,
}

const _: () = assert!(core::mem::size_of::<ParamDescriptor>() == 16);

impl ParamDescriptor {
    pub const fn new(name_hash: bx_u64, type_id: bx_u32, scalar_kind: ScalarKind) -> Self {
        Self {
            name_hash,
            type_id,
            scalar_kind,
            _pad: 0,
        }
    }

    pub const fn unnamed(type_id: bx_u32, scalar_kind: ScalarKind) -> Self {
        Self::new(0, type_id, scalar_kind)
    }
}

/// A complete function signature: parameters + return type + calling convention.
///
/// This is the typed equivalent of `extern "C" fn(...)` — it carries enough
/// metadata for:
/// - Code generation (register assignment per calling convention)
/// - LangBridge marshaling (type-safe FFI argument conversion)
/// - VTable validation (ensuring interface methods have correct signatures)
/// - Syscall dispatch (validating argument counts and types)
///
/// BEF representation: stored in `.type_map` section as a TypeMeta with
/// `kind = 3 (fn)`, followed by `param_count` ParamDescriptors and one
/// return type ID.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FunctionSignature {
    /// Calling convention for this function.
    pub convention: CallingConvention,
    /// Number of parameters.
    pub param_count: bx_u32,
    /// TypeRegistry index for the return type (0 = void).
    pub return_type_id: bx_u32,
    /// Reserved for flags (variadic, noexcept, pure, etc.).
    pub flags: bx_u32,
}

const _: () = assert!(core::mem::size_of::<FunctionSignature>() == 16);

impl FunctionSignature {
    pub const fn new(convention: CallingConvention) -> Self {
        Self {
            convention,
            param_count: 0,
            return_type_id: 0,
            flags: 0,
        }
    }

    pub const fn with_return(mut self, return_type_id: bx_u32) -> Self {
        self.return_type_id = return_type_id;
        self
    }

    /// How many GPR registers this signature consumes for arguments.
    pub fn gpr_usage(&self, params: &[ParamDescriptor]) -> bx_u32 {
        let available = self.convention.gpr_arg_count() as usize;
        let mut used = 0u32;
        for p in params.iter().take(available) {
            if p.scalar_kind as u8 != ScalarKind::Void as u8 {
                used += 1;
            }
        }
        used.min(available as u32) as bx_u32
    }

    /// True if all arguments fit in registers (no stack spill).
    pub fn is_register_call(&self, params: &[ParamDescriptor]) -> bool {
        self.gpr_usage(params) <= self.convention.gpr_arg_count() as u32
    }
}

/// Flags for function signatures.
pub mod func_flags {
    use crate::bmo_abi::primitives::bx_u32;

    /// Function accepts variable arguments (e.g., printf-style).
    pub const VARIADIC: bx_u32 = 1 << 0;
    /// Function cannot throw/exceptions (enables tail-call optimization).
    pub const NOEXCEPT: bx_u32 = 1 << 1;
    /// Function is pure (no side effects, no memory writes).
    pub const PURE: bx_u32 = 1 << 2;
    /// Function does not return (e.g., exit, abort, longjmp).
    pub const NORETURN: bx_u32 = 1 << 3;
    /// Function is a syscall stub (3-byte: syscall; ret).
    pub const SYSCALL_STUB: bx_u32 = 1 << 4;
}
