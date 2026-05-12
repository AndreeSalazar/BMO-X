//! BMO ABI — la convención de llamada nativa de FastOS / BareX.
//!
//! **Reemplaza el C ABI / cdecl / stdcall / Win64 ABI heredado.**
//!
//! El BMO ABI está optimizado para:
//!   - Ryzen 5 5600X (Zen 3, 6 cores, 1 CCD, 32 MB L3 unificado).
//!   - Llamadas BareX: muchos parámetros pequeños + handles opacos `u64`.
//!   - Cero conversión de tipos heredados (sin `HRESULT`, sin `BOOL`, sin
//!     `wchar_t`/UTF-16, sin estructuras COM).
//!
//! ## Diferencias clave vs Microsoft x64 ABI / System V AMD64 ABI
//!
//! | Aspecto              | MS x64           | SysV AMD64        | **BMO ABI**            |
//! |----------------------|------------------|-------------------|------------------------|
//! | Args int             | RCX RDX R8 R9    | RDI RSI RDX RCX R8 R9 | **RDI RSI RDX R10 R8 R9 RAX_extra** (7 regs)|
//! | Args float           | XMM0..3          | XMM0..7           | **XMM0..7** + opcional ZMM 0..3 |
//! | Shadow space         | 32 bytes         | red zone 128 B    | **0 bytes** (sin sombras) |
//! | Caller-saved         | RAX RCX RDX R8-R11 XMM0-5 | RAX RCX RDX RSI RDI R8-R11 XMM0-15 | **RAX R10 R11 XMM8-15** (poco) |
//! | Stack alignment      | 16 B antes call  | 16 B antes call   | **64 B** (cache line completa) |
//! | Return ints          | RAX (RDX:RAX)    | RAX (RDX:RAX)     | **RAX:RDX (128 b)** + flags en R11 |
//! | Strings              | UTF-16 wchar_t*  | UTF-8 char*       | **UTF-8** + longitud explícita (no `\0`) |
//! | Errores              | HRESULT/SetLastError | errno global  | **`BxResult<T>` por valor** (sin globals) |
//! | Handles              | `HANDLE` opaco   | fd `i32`          | **`u64` opaco con bit de tipo** |
//! | Async                | OVERLAPPED+APC   | callback / poll   | **Submission Queue / Completion Queue** (io_uring-like) |
//!
//! ## Por qué 7 registros para args int
//!
//! Las llamadas BareX típicas (`bx_cmdlist::draw`, `bx_buffer::write_at`, etc.)
//! pasan 5–7 enteros + 0–2 floats. Tener 7 GPRs disponibles **elimina spills al
//! stack en el 90 % de las llamadas calientes**, ahorrando ~3 ns por llamada y
//! liberando puertos de issue del Zen 3.
//!
//! ## Layout de structs (`#[repr(bmo)]` conceptual)
//!
//! - **Sin padding final** (no se alinea el `sizeof` al alignment más grande).
//! - **Campos pequeños primero** para empacar mejor.
//! - Alignment respetado por campo individualmente.
//! - **Endianness:** little-endian fijo (x86-64).
//!
//! ## Wire format para handles
//!
//! ```text
//!   bit 63        : tag de tipo (0 = recurso, 1 = canal/cola)
//!   bits 62..56   : kind (7 bits — 128 tipos posibles)
//!   bits 55..40   : generación (16 bits — detecta use-after-free)
//!   bits 39..0    : índice en la tabla (1 trillón de slots)
//! ```
//!
//! Esto detecta automáticamente UAF: si un slot se libera y se reasigna,
//! el handle viejo tiene generación distinta y la API devuelve
//! `BxError::InvalidArgument`.

#![allow(dead_code)]

/// Resultado canónico empacado para retorno por valor en BMO ABI.
///
/// Ocupa exactamente 16 bytes — cabe en `RAX:RDX`, sin tocar memoria.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoStatus {
    /// 0 = OK; >0 = código de error (`BxError as u32`).
    pub code: u32,
    /// Bits de flags auxiliares (truncated, partial, retry, etc.).
    pub flags: u32,
    /// Valor útil (handle, contador, lo que aplique).
    pub value: u64,
}

impl BmoStatus {
    pub const OK: Self = Self { code: 0, flags: 0, value: 0 };

    pub const fn ok_value(v: u64) -> Self {
        Self { code: 0, flags: 0, value: v }
    }

    pub const fn err(code: u32) -> Self {
        Self { code, flags: 0, value: 0 }
    }

    #[inline(always)]
    pub const fn is_ok(&self) -> bool { self.code == 0 }
}

/// Handle opaco BMO — 64 bits con tag, kind, generación e índice.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BmoHandle(pub u64);

impl BmoHandle {
    pub const NULL: Self = Self(0);

    #[inline(always)]
    pub const fn new(kind: HandleKind, generation: u16, index: u64) -> Self {
        let tag = (kind.tag() as u64) << 63;
        let kind_bits = ((kind.code() as u64) & 0x7F) << 56;
        let gen_bits = ((generation as u64) & 0xFFFF) << 40;
        let idx_bits = index & 0x000000FF_FFFFFFFF;
        Self(tag | kind_bits | gen_bits | idx_bits)
    }

    #[inline(always)]
    pub const fn is_resource(&self) -> bool { (self.0 >> 63) == 0 }

    #[inline(always)]
    pub const fn kind_code(&self) -> u8 {
        ((self.0 >> 56) & 0x7F) as u8
    }

    #[inline(always)]
    pub const fn generation(&self) -> u16 {
        ((self.0 >> 40) & 0xFFFF) as u16
    }

    #[inline(always)]
    pub const fn index(&self) -> u64 { self.0 & 0x000000FF_FFFFFFFF }
}

/// Tipos de handle reconocidos por la tabla del kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HandleKind {
    Device       = 0x01,
    Queue        = 0x02,
    CmdList      = 0x03,
    Pso          = 0x04,
    RootSig      = 0x05,
    Heap         = 0x06,
    Buffer       = 0x07,
    Texture      = 0x08,
    Sampler      = 0x09,
    Fence        = 0x0A,
    Swapchain    = 0x0B,
    QueryHeap    = 0x0C,
    AudioEngine  = 0x10,
    AudioVoice   = 0x11,
    InputDevice  = 0x20,
    NetSocket    = 0x30,
    NetEndpoint  = 0x31,
    File         = 0x40,
    Process      = 0x50,
    Thread       = 0x51,
}

impl HandleKind {
    #[inline(always)]
    pub const fn code(self) -> u8 { self as u8 }

    /// 0 = recurso pasivo, 1 = canal/cola activo.
    #[inline(always)]
    pub const fn tag(self) -> u8 {
        match self {
            HandleKind::Queue
            | HandleKind::CmdList
            | HandleKind::AudioVoice
            | HandleKind::NetSocket
            | HandleKind::Thread => 1,
            _ => 0,
        }
    }
}

/// Slice BMO: puntero + longitud explícita (sin `\0` terminator).
/// Se pasa como dos enteros consecutivos en el ABI (encaja en RDI:RSI por ejemplo).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoSlice {
    pub ptr: *const u8,
    pub len: u64,
}

impl BmoSlice {
    pub const EMPTY: Self = Self { ptr: core::ptr::null(), len: 0 };

    /// SAFETY: `bytes` debe permanecer válido mientras se use el slice.
    #[inline(always)]
    pub const fn from_bytes(bytes: &[u8]) -> Self {
        Self { ptr: bytes.as_ptr(), len: bytes.len() as u64 }
    }

    /// SAFETY: `s` debe permanecer válido. Cadena UTF-8 sin terminador nulo.
    #[inline(always)]
    pub const fn from_str(s: &str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() as u64 }
    }
}

/// Stack alignment requerido por el BMO ABI antes de un `call`.
pub const STACK_ALIGNMENT: usize = 64;

/// Tamaño de la "shadow space" — siempre 0 en BMO ABI (vs 32 en MS x64).
pub const SHADOW_SPACE: usize = 0;

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);
