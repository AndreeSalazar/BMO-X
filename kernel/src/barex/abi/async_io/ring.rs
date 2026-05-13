//! SQ/CQ rings — colas mmap'd compartidas user/kernel.
//!
//! Modelo:
//!   - **SQE** (Submission Queue Entry): 64 B, encolada por la app.
//!   - **CQE** (Completion Queue Entry):  16 B, drenada por la app.
//!   - El kernel monitorea la SQ con MWAIT/timer + write-watch sobre el head.
//!   - Cero syscalls en hot path (solo al rellenar/drenar).

use crate::barex::abi::primitives::{bx_u8, bx_u16, bx_u32, bx_u64};
use crate::barex::abi::handle::BmoHandle;

/// Op codes universales para cualquier subsistema async.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Nop                 = 0x0000,

    // ─── VFS ─────────────────────────────────────────────────────────
    FileRead            = 0x0010,
    FileWrite           = 0x0011,
    FileFsync           = 0x0012,

    // ─── Network ─────────────────────────────────────────────────────
    SocketSend          = 0x0020,
    SocketRecv          = 0x0021,
    SocketAccept        = 0x0022,
    SocketConnect       = 0x0023,

    // ─── Graphics ────────────────────────────────────────────────────
    GpuSubmitCmdList    = 0x0030,
    GpuPresent          = 0x0031,

    // ─── Audio ───────────────────────────────────────────────────────
    AudioSubmitPcm      = 0x0040,
    AudioCapturePcm     = 0x0041,

    // ─── DirectStorage ───────────────────────────────────────────────
    StorageStream       = 0x0050,
    StorageDecompress   = 0x0051,

    // ─── Time ────────────────────────────────────────────────────────
    Sleep               = 0x0060,
    Timer               = 0x0061,
}

/// Submission Queue Entry — exactamente 64 bytes (cabe en una cache line).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Sqe {
    pub op: bx_u16,                 // OpCode
    pub flags: bx_u16,              // SqeFlags bitfield
    pub priority: bx_u8,            // 0..=255
    pub _pad0: [bx_u8; 3],

    pub user_data: bx_u64,          // se devuelve sin tocar en el CQE.user_data
    pub target: BmoHandle,          // recurso afectado (file, socket, queue...)
    pub buffer_ptr: bx_u64,         // payload — formato depende de OpCode
    pub buffer_len: bx_u64,
    pub offset: bx_u64,             // file offset / RTP timestamp / etc.
    pub aux: [bx_u64; 2],           // params extra
}

/// Completion Queue Entry — 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cqe {
    pub user_data: bx_u64,          // copiado desde el SQE original
    pub result: bx_u32,             // bytes escritos / código de error
    pub flags: bx_u32,              // CqeFlags
}

/// Estado del ring (compartido user/kernel vía mmap).
#[repr(C)]
pub struct SqRing {
    /// Head: incrementado por el kernel cuando consume un SQE.
    pub head: core::sync::atomic::AtomicU32,
    /// Tail: incrementado por la app cuando escribe un SQE.
    pub tail: core::sync::atomic::AtomicU32,
    /// Capacidad — siempre potencia de 2.
    pub capacity: bx_u32,
    pub mask: bx_u32,               // capacity - 1
    pub entries: *mut Sqe,
}

#[repr(C)]
pub struct CqRing {
    /// Head: incrementado por la app cuando drena un CQE.
    pub head: core::sync::atomic::AtomicU32,
    /// Tail: incrementado por el kernel cuando publica un CQE.
    pub tail: core::sync::atomic::AtomicU32,
    pub capacity: bx_u32,
    pub mask: bx_u32,
    pub entries: *mut Cqe,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SqeFlags: bx_u16 {
        /// Encadenar al siguiente SQE — se ejecuta solo si este OK.
        const LINK         = 1 << 0;
        /// No notificar el CQE (fire-and-forget).
        const NO_CQE       = 1 << 1;
        /// Esperar fence antes de iniciar.
        const WAIT_FENCE   = 1 << 2;
        /// Marcar como prioritario (puede saltar la cola).
        const PRIORITY     = 1 << 3;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct CqeFlags: bx_u32 {
        /// Hubo un error — `result` contiene el código.
        const ERROR        = 1 << 0;
        /// Operación cancelada.
        const CANCELLED    = 1 << 1;
        /// Reintento sugerido.
        const RETRY        = 1 << 2;
        /// Operación parcial — `result` indica bytes hechos.
        const PARTIAL      = 1 << 3;
    }
}
