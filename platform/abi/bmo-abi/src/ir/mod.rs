//! BMO Intermediate Representation (IR) -- the unified AST that all language
//! frontends emit and all backends consume.
//!
//! # Architecture
//!
//! ```text
//!  C frontend    COBOL frontend    C++ frontend    Rust frontend
//!      |               |               |               |
//!      v               v               v               v
//!  +---------------------------------------------------------+
//!  |                    bmo_abi::ir                          |
//!  |                                                         |
//!  |  IrModule -> IrType -> IrFunction -> IrBlock -> IrStmt      |
//!  |                                                         |
//!  |  -> IrExpr (const, local, binop, call, load, store)      |
//!  +------------------------+--------------------------------+
//!                           |
//!                           v
//!  +---------------------------------------------------------+
//!  |              x86-64 codegen  /  ARM64 codegen           |
//!  +------------------------+--------------------------------+
//!                           |
//!                           v
//!                     BEF binary
//! ```
//!
//! # Design constraints
//!
//! - **no_std**: works in kernel and userland without libstd
//! - **No recursion in types**: all types are Copy or have fixed-size arrays
//! - **SSA-lite**: each local is assigned once per function (not strictly enforced)
//! - **Fixed capacity**: IrModule, IrFunction, IrBlock have fixed maximum sizes

use crate::bmo_abi::types::convention::{CallingConvention, ScalarKind};

/// Maximum functions per module.
pub const MAX_FUNCTIONS: usize = 64;

/// Maximum basic blocks per function.
pub const MAX_BLOCKS: usize = 32;

/// Maximum locals per function.
pub const MAX_LOCALS: usize = 128;

/// Maximum arguments per function.
pub const MAX_ARGS: usize = 32;

/// Maximum statements per basic block.
pub const MAX_STMTS: usize = 256;

/// Maximum syscall definitions per module.
pub const MAX_SYSCALLS: usize = 128;

/// Maximum imported library modules.
pub const MAX_IMPORTS: usize = 64;

/// Maximum globals per module.
pub const MAX_GLOBALS: usize = 64;

/// A syscall definition loaded from Semantic_ASM.
#[derive(Debug, Clone, Copy)]
pub struct IrSyscallDef {
    pub name: u16,
    pub nr: u32,
    pub arg_count: u8,
}

/// An imported library module.
#[derive(Debug, Clone, Copy)]
pub struct IrImport {
    pub module_path: u16,
    pub bef_offset: u32,
    pub bef_len: u32,
}

// -- IrType ----------------------------------------------------------

/// Unified type representation.
///
/// Maps directly to `runtime::types::TypeMeta` during codegen.
/// Language frontends produce these; the codegen registers them in TypeRegistry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    /// void (no value).
    Void,
    /// 8-bit signed integer.
    I8,
    /// 16-bit signed integer.
    I16,
    /// 32-bit signed integer.
    I32,
    /// 64-bit signed integer.
    I64,
    /// 8-bit unsigned integer.
    U8,
    /// 16-bit unsigned integer.
    U16,
    /// 32-bit unsigned integer.
    U32,
    /// 64-bit unsigned integer.
    U64,
    /// 32-bit IEEE-754 float.
    F32,
    /// 64-bit IEEE-754 float.
    F64,
    /// Boolean (1 byte, 0 or 1).
    Bool,
    /// Opaque pointer (target type unknown or void*).
    Pointer,
    /// Sized pointer to a known type (index into module's type table).
    PointerTo(u16),
    /// Fixed-size array of a known type.
    Array { elem: u16, len: u32 },
    /// Struct (index into module's struct table).
    Struct(u16),
    /// Function pointer (index into module's type table for the signature).
    Function(u16),
}

impl IrType {
    /// Size in bytes. Returns 0 for Void, 8 for Pointer, and computed sizes.
    pub fn size(&self) -> u32 {
        match self {
            Self::Void => 0,
            Self::I8 | Self::U8 | Self::Bool => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64
            | Self::U64
            | Self::F64
            | Self::Pointer
            | Self::PointerTo(_)
            | Self::Function(_) => 8,
            Self::Array { elem: _, len } => {
                // Approximate -- actual size depends on elem type
                len * 8
            }
            Self::Struct(_) => 0, // Unknown at IR level -- resolved during codegen
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Pointer => "ptr",
            Self::PointerTo(_) => "ptr",
            Self::Array { .. } => "array",
            Self::Struct(_) => "struct",
            Self::Function(_) => "fn",
        }
    }

    pub const fn to_scalar_kind(&self) -> Option<ScalarKind> {
        match self {
            Self::Void => Some(ScalarKind::Void),
            Self::I8 => Some(ScalarKind::I8),
            Self::I16 => Some(ScalarKind::I16),
            Self::I32 => Some(ScalarKind::I32),
            Self::I64 => Some(ScalarKind::I64),
            Self::U8 => Some(ScalarKind::U8),
            Self::U16 => Some(ScalarKind::U16),
            Self::U32 => Some(ScalarKind::U32),
            Self::U64 => Some(ScalarKind::U64),
            Self::F32 => Some(ScalarKind::F32),
            Self::F64 => Some(ScalarKind::F64),
            Self::Bool => Some(ScalarKind::Bool),
            Self::Pointer | Self::PointerTo(_) | Self::Function(_) => Some(ScalarKind::Pointer),
            Self::Array { .. } | Self::Struct(_) => None,
        }
    }
}

// -- IrExpr ----------------------------------------------------------

/// An expression -- produces a value.
#[derive(Debug, Clone, Copy)]
pub enum IrExpr {
    /// Signed 64-bit integer literal.
    ConstI64(i64),
    /// Unsigned 64-bit integer literal.
    ConstU64(u64),
    /// Floating-point literal.
    ConstF64(f64),
    /// String literal (index into module's string table).
    ConstStr(u16),
    /// Zero-initialized value of a given type.
    ConstZero(IrType),
    /// Read a local variable (by index).
    Local(u16),
    /// Read a global variable (by index).
    Global(u16),
    /// Read a function argument (by index).
    Arg(u16),
    /// Binary operation: lhs op rhs.
    Binary { op: IrBinOp, lhs: u16, rhs: u16 },
    /// Unary operation: op expr.
    Unary { op: IrUnOp, expr: u16 },
    /// Load from memory: *(base + offset).
    Load { base: u16, offset: i32, ty: IrType },
    /// Address-of: &local or &global.
    AddrOf(u16),
    /// Call a function: fn(args...).
    Call {
        func: u16,
        args: u16,
        arg_count: u16,
    },
    /// Syscall invocation.
    Syscall { nr: u32, args: u16, arg_count: u16 },
    /// Virtual call through vtable: *(obj->vtable[offset])(args).
    VCall {
        this: u16,
        vtable_offset: u32,
        sig_type_id: u16,
        args: u16,
        arg_count: u16,
    },
    /// Type cast expression.
    Cast { expr: u16, to: IrType },
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LtU,
    LeU,
    GtU,
    GeU,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
    Not,
    BitNot,
}

// -- IrStmt ----------------------------------------------------------

/// A statement -- produces no value, has a side effect.
#[derive(Debug, Clone, Copy)]
pub enum IrStmt {
    /// Assign: local[idx] = expr.
    Assign(u16, IrExpr),
    /// Store to memory: *(base + offset) = value.
    Store { base: u16, offset: i32, value: u16 },
    /// Evaluate expression, discard result.
    Expr(IrExpr),
    /// Conditional branch: if cond then true_block else false_block.
    Branch {
        cond: u16,
        then_block: u16,
        else_block: u16,
    },
    /// Unconditional jump to another block.
    Jump(u16),
    /// Return from function (with optional value).
    Return(Option<u16>),
    /// Define a local variable.
    DefLocal { idx: u16, ty: IrType },
}

// -- IrBlock ---------------------------------------------------------

/// A basic block: a linear sequence of statements ending with a terminator.
#[derive(Debug, Clone)]
pub struct IrBlock {
    pub label: u16,
    pub stmts: [IrStmt; MAX_STMTS],
    pub stmt_count: u16,
}

impl IrBlock {
    pub fn new(label: u16) -> Self {
        Self {
            label,
            stmts: [IrStmt::Return(None); MAX_STMTS],
            stmt_count: 0,
        }
    }

    pub fn push(&mut self, stmt: IrStmt) -> bool {
        if self.stmt_count as usize >= MAX_STMTS {
            return false;
        }
        self.stmts[self.stmt_count as usize] = stmt;
        self.stmt_count += 1;
        true
    }
}

// -- IrFunction ------------------------------------------------------

/// A function definition.
#[derive(Debug, Clone)]
pub struct IrFunction {
    /// Function name (index into module string table).
    pub name: u16,
    /// Calling convention for this function.
    pub convention: CallingConvention,
    /// Return type (index into module type table).
    pub return_type: u16,
    /// Argument types (indices into module type table).
    pub args: [u16; MAX_ARGS],
    pub arg_count: u16,
    /// Local variables.
    pub locals: [(u16, IrType); MAX_LOCALS],
    pub local_count: u16,
    /// Basic blocks.
    pub blocks: [IrBlock; MAX_BLOCKS],
    pub block_count: u16,
    /// Whether this function is public (exported).
    pub public: bool,
}

impl IrFunction {
    pub fn new(name: u16) -> Self {
        Self {
            name,
            convention: CallingConvention::BmoX86_64,
            return_type: 0,
            args: [0u16; MAX_ARGS],
            arg_count: 0,
            locals: [(0, IrType::Void); MAX_LOCALS],
            local_count: 0,
            blocks: core::array::from_fn(|i| IrBlock::new(i as u16)),
            block_count: 0,
            public: false,
        }
    }

    pub fn add_arg(&mut self, type_id: u16) -> bool {
        if self.arg_count as usize >= MAX_ARGS {
            return false;
        }
        self.args[self.arg_count as usize] = type_id;
        self.arg_count += 1;
        true
    }

    pub fn add_local(&mut self, idx: u16, ty: IrType) -> bool {
        if self.local_count as usize >= MAX_LOCALS {
            return false;
        }
        self.locals[self.local_count as usize] = (idx, ty);
        self.local_count += 1;
        true
    }

    pub fn add_block(&mut self, block: IrBlock) -> Option<u16> {
        if self.block_count as usize >= MAX_BLOCKS {
            return None;
        }
        let idx = self.block_count;
        self.blocks[idx as usize] = block;
        self.block_count += 1;
        Some(idx)
    }

    pub fn block_mut(&mut self, idx: u16) -> Option<&mut IrBlock> {
        self.blocks
            .get_mut(idx as usize)
            .filter(|_| idx < self.block_count)
    }
}

// -- IrGlobal --------------------------------------------------------

/// A global variable declaration.
#[derive(Debug, Clone, Copy)]
pub struct IrGlobal {
    pub name: u16,
    pub ty: u16,
    pub init: Option<IrExpr>,
    pub read_only: bool,
}

// -- IrModule --------------------------------------------------------

/// A complete compilation unit -- the output of a language frontend.
#[derive(Debug, Clone)]
pub struct IrModule {
    /// Module name (index into string table).
    pub name: u16,
    /// Type table: all types referenced by this module.
    pub types: [IrType; 64],
    pub type_count: u16,
    /// String table: all string literals and identifiers.
    pub strings: [u16; 256], // length-prefixed offsets into string_data
    pub string_count: u16,
    /// Raw string data.
    pub string_data: [u8; 4096],
    pub string_data_len: u16,
    /// Function definitions.
    pub functions: [IrFunction; MAX_FUNCTIONS],
    pub function_count: u16,
    /// Global variable declarations.
    pub globals: [IrGlobal; MAX_GLOBALS],
    pub global_count: u16,
    /// Syscall definitions loaded from Semantic_ASM.
    pub syscalls: [IrSyscallDef; MAX_SYSCALLS],
    pub syscall_count: u16,
    /// Imported library modules (stdlib, etc.).
    pub imports: [IrImport; MAX_IMPORTS],
    pub import_count: u16,
}

impl IrModule {
    pub fn new(name: u16) -> Self {
        Self {
            name,
            types: [IrType::Void; 64],
            type_count: 0,
            strings: [0u16; 256],
            string_count: 0,
            string_data: [0u8; 4096],
            string_data_len: 0,
            functions: core::array::from_fn(|_| IrFunction::new(0)),
            function_count: 0,
            globals: core::array::from_fn(|_| IrGlobal {
                name: 0,
                ty: 0,
                init: None,
                read_only: false,
            }),
            global_count: 0,
            syscalls: [IrSyscallDef {
                name: 0,
                nr: 0,
                arg_count: 0,
            }; MAX_SYSCALLS],
            syscall_count: 0,
            imports: [IrImport {
                module_path: 0,
                bef_offset: 0,
                bef_len: 0,
            }; MAX_IMPORTS],
            import_count: 0,
        }
    }

    /// Add a type and return its index.
    pub fn add_type(&mut self, ty: IrType) -> Option<u16> {
        if self.type_count as usize >= 64 {
            return None;
        }
        let idx = self.type_count;
        self.types[idx as usize] = ty;
        self.type_count += 1;
        Some(idx)
    }

    /// Add a string and return its index.
    pub fn add_string(&mut self, s: &str) -> Option<u16> {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if self.string_count as usize >= 256 {
            return None;
        }
        if self.string_data_len as usize + 2 + len > 4096 {
            return None;
        }
        let idx = self.string_count;
        // Store length prefix (u16 LE)
        self.string_data[self.string_data_len as usize] = (len & 0xFF) as u8;
        self.string_data[self.string_data_len as usize + 1] = ((len >> 8) & 0xFF) as u8;
        self.string_data_len += 2;
        // Store string bytes
        let start = self.string_data_len as usize;
        self.string_data[start..start + len].copy_from_slice(bytes);
        self.string_data_len += len as u16;
        // Store offset
        self.strings[idx as usize] = idx; // offset = start - 2 (before length prefix)
        self.string_count += 1;
        Some(idx)
    }

    /// Add a function and return its index.
    pub fn add_function(&mut self, func: IrFunction) -> Option<u16> {
        if self.function_count as usize >= MAX_FUNCTIONS {
            return None;
        }
        let idx = self.function_count;
        self.functions[idx as usize] = func;
        self.function_count += 1;
        Some(idx)
    }

    /// Add a global and return its index.
    pub fn add_global(&mut self, global: IrGlobal) -> Option<u16> {
        if self.global_count as usize >= MAX_GLOBALS {
            return None;
        }
        let idx = self.global_count;
        self.globals[idx as usize] = global;
        self.global_count += 1;
        Some(idx)
    }

    /// Add a syscall definition and return its index.
    pub fn add_syscall(&mut self, name: u16, nr: u32, arg_count: u8) -> Option<u16> {
        if self.syscall_count as usize >= MAX_SYSCALLS {
            return None;
        }
        let idx = self.syscall_count;
        self.syscalls[idx as usize] = IrSyscallDef {
            name,
            nr,
            arg_count,
        };
        self.syscall_count += 1;
        Some(idx)
    }

    /// Add an imported library module.
    pub fn add_import(&mut self, module_path: u16, bef_offset: u32, bef_len: u32) -> Option<u16> {
        if self.import_count as usize >= MAX_IMPORTS {
            return None;
        }
        let idx = self.import_count;
        self.imports[idx as usize] = IrImport {
            module_path,
            bef_offset,
            bef_len,
        };
        self.import_count += 1;
        Some(idx)
    }

    /// Load all syscall definitions from the embedded registry.
    ///
    /// * Devuelve `Err` en vez de tragarse el fallo, y esto es una trampa que
    /// todavia no habia saltado: `add_string` devuelve `None` cuando la tabla
    /// de cadenas se llena (256 entradas o 4 KiB), y el `unwrap_or(0)` que
    /// habia aqui registraba entonces el syscall **con el nombre de la cadena
    /// 0** -- o sea, con el nombre de otro. Un syscall mal nombrado en el IR no
    /// falla al compilar: falla al mirar el binario y no entender que llama.
    ///
    /// Hoy no lo llama nadie, y por eso mismo se arregla ahora: es gratis
    /// cambiar la firma antes de que exista el primer llamante, y el `Result`
    /// obliga al que llegue a decidir que hacer en vez de heredar el silencio.
    #[must_use = "si la tabla de cadenas se lleno, los syscalls quedan mal nombrados"]
    pub fn load_embedded_syscalls(&mut self) -> Result<(), &'static str> {
        let defs = crate::bmo_abi::asm::defs::syscalls();
        for d in &defs {
            let name_idx = self
                .add_string(&d.name)
                .ok_or("la tabla de cadenas del IR se lleno cargando los syscalls")?;
            self.add_syscall(name_idx, d.nr, d.arg_count);
        }
        Ok(())
    }
}
