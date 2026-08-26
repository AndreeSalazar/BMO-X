//! `HandleKind` -- taxonomia de recursos del kernel.
//!
//! 7 bits = 128 tipos posibles. Tag bit 63 distingue:
//!   - 0 = recurso pasivo (textura, buffer, sampler)
//!   - 1 = canal/cola activo (queue, cmdlist, socket)

use crate::bmo_abi::primitives::bx_u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HandleKind {
    // --- Graphics (0x01..0x1F) ---------------------------------------
    Device = 0x01,
    Queue = 0x02,
    CmdList = 0x03,
    Pso = 0x04,
    RootSig = 0x05,
    Heap = 0x06,
    Buffer = 0x07,
    Texture = 0x08,
    Sampler = 0x09,
    Fence = 0x0A,
    Swapchain = 0x0B,
    QueryHeap = 0x0C,
    BlasAccelStruct = 0x0D,
    TlasAccelStruct = 0x0E,
    /// **La pantalla.** El derecho a escribir pixeles directamente: el kernel
    /// mapea el framebuffer en el espacio del proceso UNA VEZ y a partir de
    /// ahi no vuelve a tocarlo. No hay syscall por pixel porque no hay
    /// frontera que cruzar -- es el momento library-OS del diseno.
    ///
    /// Es exclusiva por construccion: un solo proceso la tiene a la vez.
    Framebuffer = 0x0F,

    // --- Audio (0x10..0x1F) ------------------------------------------
    AudioEngine = 0x10,
    AudioVoice = 0x11,
    AudioSpatializer = 0x12,
    AudioStream = 0x13,

    // --- Input (0x20..0x2F) ------------------------------------------
    InputDevice = 0x20,
    InputProfile = 0x21,

    // --- Network (0x30..0x3F) ----------------------------------------
    NetSocket = 0x30,
    NetEndpoint = 0x31,
    NetTlsCtx = 0x32,
    NetQuicConn = 0x33,

    // --- VFS / Storage (0x40..0x4F) ----------------------------------
    File = 0x40,
    Directory = 0x41,
    Mount = 0x42,
    StorageOp = 0x43,

    // --- Procesos / threading (0x50..0x5F) ---------------------------
    Process = 0x50,
    Thread = 0x51,
    Mutex = 0x52,
    Futex = 0x53,
    Port = 0x54,

    // --- IPC (0x60..0x6F) --------------------------------------------
    /// Estuario BMO Channel: pagina compartida Ring 0 <-> Ring 3.
    /// El objeto que `CHANNEL_KICK`/`WAIT` resuelven en el kernel.
    Channel = 0x60,

    // --- Aparatos crudos (0x70..0x7F) --------------------------------
    /// **Una ventana de registros de un aparato, cedida a Ring 3.**
    ///
    /// Pieza S1 del suelo de `docs/plan/PLAN_SUELO_RING3.md`. La concede
    /// `TASK_OP_APARATO_TOMAR`, y lo importante es lo que **no** hay:
    /// ninguna operacion acepta una direccion fisica. El proceso nombra un
    /// aparato de una lista cerrada y el kernel saca la fisica de su censo.
    ///
    /// [!] Aqui hay dos vecinos que este enum todavia no nombra --el kernel usa
    /// `0x70` para su endpoint y `0x71` para su reply-- y por eso este empieza
    /// en `0x74`: para no chocar con una tabla que ya existe aunque no este
    /// escrita aqui. Que las dos hayan divergido es una deuda anotada, no una
    /// licencia para ensancharla.
    MmioWindow = 0x74,
}

impl HandleKind {
    #[inline(always)]
    pub const fn code(self) -> bx_u8 {
        self as u8
    }

    /// 0 = recurso pasivo, 1 = canal/cola activo.
    #[inline(always)]
    pub const fn tag(self) -> bx_u8 {
        match self {
            HandleKind::Queue
            | HandleKind::CmdList
            | HandleKind::AudioVoice
            | HandleKind::AudioStream
            | HandleKind::NetSocket
            | HandleKind::NetQuicConn
            | HandleKind::Thread
            | HandleKind::StorageOp
            | HandleKind::Channel => 1,
            _ => 0,
        }
    }

    /// Tenta decodificar desde el byte `code` del handle. `None` si desconocido.
    pub const fn from_code(c: bx_u8) -> Option<Self> {
        match c {
            0x01 => Some(Self::Device),
            0x02 => Some(Self::Queue),
            0x03 => Some(Self::CmdList),
            0x04 => Some(Self::Pso),
            0x05 => Some(Self::RootSig),
            0x06 => Some(Self::Heap),
            0x07 => Some(Self::Buffer),
            0x08 => Some(Self::Texture),
            0x09 => Some(Self::Sampler),
            0x0A => Some(Self::Fence),
            0x0B => Some(Self::Swapchain),
            0x0C => Some(Self::QueryHeap),
            0x0D => Some(Self::BlasAccelStruct),
            0x0E => Some(Self::TlasAccelStruct),
            0x0F => Some(Self::Framebuffer),
            0x10 => Some(Self::AudioEngine),
            0x11 => Some(Self::AudioVoice),
            0x12 => Some(Self::AudioSpatializer),
            0x13 => Some(Self::AudioStream),
            0x20 => Some(Self::InputDevice),
            0x21 => Some(Self::InputProfile),
            0x30 => Some(Self::NetSocket),
            0x31 => Some(Self::NetEndpoint),
            0x32 => Some(Self::NetTlsCtx),
            0x33 => Some(Self::NetQuicConn),
            0x40 => Some(Self::File),
            0x41 => Some(Self::Directory),
            0x42 => Some(Self::Mount),
            0x43 => Some(Self::StorageOp),
            0x50 => Some(Self::Process),
            0x51 => Some(Self::Thread),
            0x52 => Some(Self::Mutex),
            0x53 => Some(Self::Futex),
            0x54 => Some(Self::Port),
            0x60 => Some(Self::Channel),
            0x74 => Some(Self::MmioWindow),
            _ => None,
        }
    }
}
