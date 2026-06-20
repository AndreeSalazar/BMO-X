//! SQ/CQ rings — colas mmap'd compartidas user/kernel.
//!
//! Modelo:
//!   - **SQE** (Submission Queue Entry): 64 B, encolada por la app.
//!   - **CQE** (Completion Queue Entry):  16 B, drenada por la app.
//!   - El kernel monitorea la SQ con MWAIT/timer + write-watch sobre el head.
//!   - Cero syscalls en hot path (solo al rellenar/drenar).
//!
//! ## Producer (Ring Producer) — lado app
//!
//! ```ignore
//! let mut prod = SqProducer::new(&SQ, 64);
//! if let Some(slot) = prod.acquire() {
//!     *slot = Sqe { op: OpCode::FileRead as u16, ..Default::default() };
//!     prod.commit();
//! }
//! ```
//!
//! ## Consumer (Kernel side) — lado kernel
//!
//! ```ignore
//! let mut cons = SqConsumer::new(&SQ);
//! while let Some(sqe) = cons.peek() {
//!     // ejecutar sqe...
//!     cons.consume();
//! }
//! ```
//!
//! ## Completion (Kernel side)
//!
//! ```ignore
//! let mut cq = CqProducer::new(&CQ);
//! cq.push(Cqe { user_data: sqe.user_data, result: 0, flags: 0 });
//! cq.commit();
//! ```

use core::sync::atomic::{AtomicU32, Ordering};
use crate::bmo_core::bmo_abi::primitives::{bx_u8, bx_u16, bx_u32, bx_u64};
use crate::bmo_core::bmo_abi::handle::BmoHandle;

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

impl OpCode {
    /// Convierte u16 → OpCode. `None` si desconocido.
    pub fn from_u16(v: bx_u16) -> Option<Self> {
        match v {
            0x0000 => Some(Self::Nop),
            0x0010 => Some(Self::FileRead),
            0x0011 => Some(Self::FileWrite),
            0x0012 => Some(Self::FileFsync),
            0x0020 => Some(Self::SocketSend),
            0x0021 => Some(Self::SocketRecv),
            0x0022 => Some(Self::SocketAccept),
            0x0023 => Some(Self::SocketConnect),
            0x0030 => Some(Self::GpuSubmitCmdList),
            0x0031 => Some(Self::GpuPresent),
            0x0040 => Some(Self::AudioSubmitPcm),
            0x0041 => Some(Self::AudioCapturePcm),
            0x0050 => Some(Self::StorageStream),
            0x0051 => Some(Self::StorageDecompress),
            0x0060 => Some(Self::Sleep),
            0x0061 => Some(Self::Timer),
            _ => None,
        }
    }
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

impl Sqe {
    /// SQE vacío, listo para llenar.
    pub const fn empty() -> Self {
        Self {
            op: OpCode::Nop as bx_u16,
            flags: 0,
            priority: 0,
            _pad0: [0; 3],
            user_data: 0,
            target: BmoHandle::INVALID,
            buffer_ptr: 0,
            buffer_len: 0,
            offset: 0,
            aux: [0; 2],
        }
    }
}

/// Completion Queue Entry — 16 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Cqe {
    pub user_data: bx_u64,          // copiado desde el SQE original
    pub result: bx_u32,             // bytes escritos / código de error
    pub flags: bx_u32,              // CqeFlags
}

impl Cqe {
    pub const fn empty() -> Self {
        Self { user_data: 0, result: 0, flags: 0 }
    }

    pub const fn err(user_data: bx_u64, code: bx_u32) -> Self {
        Self { user_data, result: code, flags: 1 } // ERROR flag
    }

    pub const fn ok(user_data: bx_u64, result: bx_u32) -> Self {
        Self { user_data, result, flags: 0 }
    }
}

/// Estado del ring (compartido user/kernel vía mmap).
#[repr(C)]
pub struct SqRing {
    /// Head: incrementado por el kernel cuando consume un SQE.
    pub head: AtomicU32,
    /// Tail: incrementado por la app cuando escribe un SQE.
    pub tail: AtomicU32,
    /// Capacidad — siempre potencia de 2.
    pub capacity: bx_u32,
    pub mask: bx_u32,               // capacity - 1
    pub entries: *mut Sqe,
}

#[repr(C)]
pub struct CqRing {
    /// Head: incrementado por la app cuando drena un CQE.
    pub head: AtomicU32,
    /// Tail: incrementado por el kernel cuando publica un CQE.
    pub tail: AtomicU32,
    pub capacity: bx_u32,
    pub mask: bx_u32,
    pub entries: *mut Cqe,
}

// SAFETY: SqRing/CqRing son punteros a memoria compartida mmap'd. La
// sincronización se hace vía los AtomicU32 internos. Los punteros `entries`
// nunca se reasignan.
unsafe impl Send for SqRing {}
unsafe impl Sync for SqRing {}
unsafe impl Send for CqRing {}
unsafe impl Sync for CqRing {}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct SqeFlags: bx_u16 {
        const LINK         = 1 << 0;
        const NO_CQE       = 1 << 1;
        const WAIT_FENCE   = 1 << 2;
        const PRIORITY     = 1 << 3;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct CqeFlags: bx_u32 {
        const ERROR        = 1 << 0;
        const CANCELLED    = 1 << 1;
        const RETRY        = 1 << 2;
        const PARTIAL      = 1 << 3;
    }
}

// ─── SQ Consumer (kernel-side) ───────────────────────────────────────

/// Lado kernel: consume SQEs de la Submission Queue.
///
/// Política: single-consumer (multi-consumer requiere locking externo).
pub struct SqConsumer<'a> {
    ring: &'a SqRing,
}

impl<'a> SqConsumer<'a> {
    pub const fn new(ring: &'a SqRing) -> Self { Self { ring } }

    /// Mira el siguiente SQE sin consumirlo. `None` si la cola está vacía.
    ///
    /// SAFETY: El caller debe llamar `consume()` después de procesar.
    pub fn peek(&self) -> Option<&'a Sqe> {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Acquire);
        if head == tail { return None; }
        let idx = (head & self.ring.mask) as usize;
        unsafe {
            let entry_ptr = self.ring.entries.add(idx);
            Some(&*entry_ptr)
        }
    }

    /// Mira + consume el siguiente SQE en un solo paso.
    pub fn next(&mut self) -> Option<Sqe> {
        let head = self.ring.head.load(Ordering::Relaxed);
        let tail = self.ring.tail.load(Ordering::Acquire);
        if head == tail { return None; }
        let idx = (head & self.ring.mask) as usize;
        let sqe = unsafe {
            let entry_ptr = self.ring.entries.add(idx);
            core::ptr::read(entry_ptr)
        };
        self.ring.head.store(head.wrapping_add(1), Ordering::Release);
        Some(sqe)
    }

    /// Marca el SQE actual como consumido (después de peek).
    pub fn consume(&mut self) {
        let head = self.ring.head.load(Ordering::Relaxed);
        self.ring.head.store(head.wrapping_add(1), Ordering::Release);
    }

    /// True si la cola está vacía.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ring.head.load(Ordering::Acquire) == self.ring.tail.load(Ordering::Acquire)
    }

    /// Cantidad de SQEs pendientes.
    #[inline]
    pub fn pending(&self) -> u32 {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }
}

// ─── SQ Producer (app-side) ──────────────────────────────────────────

/// Lado app: produce SQEs en la Submission Queue.
pub struct SqProducer<'a> {
    ring: &'a SqRing,
}

impl<'a> SqProducer<'a> {
    pub const fn new(ring: &'a SqRing) -> Self { Self { ring } }

    /// Adquiere un slot para escribir un SQE. `None` si la cola está llena.
    pub fn acquire(&self) -> Option<&'a mut Sqe> {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Relaxed);
        if tail.wrapping_sub(head) >= self.ring.capacity {
            return None;
        }
        let idx = (tail & self.ring.mask) as usize;
        unsafe {
            let entry_ptr = self.ring.entries.add(idx);
            Some(&mut *entry_ptr)
        }
    }

    /// Publica el SQE escrito (incrementa tail).
    pub fn commit(&self) {
        let tail = self.ring.tail.load(Ordering::Relaxed);
        self.ring.tail.store(tail.wrapping_add(1), Ordering::Release);
    }

    /// Cantidad de slots libres.
    #[inline]
    pub fn free_slots(&self) -> u32 {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Relaxed);
        self.ring.capacity - tail.wrapping_sub(head)
    }
}

// ─── CQ Producer (kernel-side) ───────────────────────────────────────

/// Lado kernel: publica CQEs en la Completion Queue.
pub struct CqProducer<'a> {
    ring: &'a CqRing,
}

impl<'a> CqProducer<'a> {
    pub const fn new(ring: &'a CqRing) -> Self { Self { ring } }

    /// Empuja un CQE. `false` si la cola está llena.
    pub fn push(&self, cqe: Cqe) -> bool {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Relaxed);
        if tail.wrapping_sub(head) >= self.ring.capacity {
            return false;
        }
        let idx = (tail & self.ring.mask) as usize;
        unsafe {
            let entry_ptr = self.ring.entries.add(idx);
            core::ptr::write(entry_ptr, cqe);
        }
        self.ring.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Cantidad de CQEs pendientes.
    #[inline]
    pub fn pending(&self) -> u32 {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }
}

// ─── CQ Consumer (app-side) ──────────────────────────────────────────

/// Lado app: drena CQEs de la Completion Queue.
pub struct CqConsumer<'a> {
    ring: &'a CqRing,
}

impl<'a> CqConsumer<'a> {
    pub const fn new(ring: &'a CqRing) -> Self { Self { ring } }

    /// Drena el siguiente CQE. `None` si la cola está vacía.
    pub fn pop(&mut self) -> Option<Cqe> {
        let head = self.ring.head.load(Ordering::Relaxed);
        let tail = self.ring.tail.load(Ordering::Acquire);
        if head == tail { return None; }
        let idx = (head & self.ring.mask) as usize;
        let cqe = unsafe {
            let entry_ptr = self.ring.entries.add(idx);
            core::ptr::read(entry_ptr)
        };
        self.ring.head.store(head.wrapping_add(1), Ordering::Release);
        Some(cqe)
    }

    /// Cantidad de CQEs pendientes.
    #[inline]
    pub fn pending(&self) -> u32 {
        let head = self.ring.head.load(Ordering::Acquire);
        let tail = self.ring.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }
}

// Re-exports de conveniencia (BmoMemOrder)
