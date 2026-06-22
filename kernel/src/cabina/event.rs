//! `cabina::event` — Eventos + caja negra circular con tags de capa.
//!
//! Cada evento lleva:
//! - **Capa (Layer)**: Ring 0, BMO Core, BMO GPU, Ring 3, Lang, FS, Net, Sec
//! - **Entidad (Entity)**: Process, Thread, Syscall, File, GPUQueue, Window, Module
//!
//! Esto permite que los filtros inteligentes (`cabina::query`) filtren
//! por capa o por entidad, además de por severidad y texto.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicBool, Ordering};
use core::fmt;
use alloc::string::String;

use crate::cabina::BLACKBOX_CAP;

/// Capa del sistema que emitió el evento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Layer {
    /// Ring 0: hardware, CPU, IRQ, memoria, drivers.
    Ring0    = 0,
    /// BMO Core: windowing, FS, desktop, ui.
    BmoCore  = 1,
    /// BMO GPU: RDNA4, compute, shaders.
    BmoGpu   = 2,
    /// Ring 3: userland apps.
    Ring3    = 3,
    /// Lang: AOT, linker, parser.
    Lang     = 4,
    /// FS: filesystem, mounts, drivers.
    Fs       = 5,
    /// Net: TCP/UDP, sockets.
    Net      = 6,
    /// Sec: capabilities, sandbox.
    Sec      = 7,
}

impl Layer {
    pub const ALL: [Layer; 8] = [
        Layer::Ring0, Layer::BmoCore, Layer::BmoGpu, Layer::Ring3,
        Layer::Lang, Layer::Fs, Layer::Net, Layer::Sec,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ring0 => "ring0",
            Self::BmoCore => "bmo_core",
            Self::BmoGpu => "bmo_gpu",
            Self::Ring3 => "ring3",
            Self::Lang => "lang",
            Self::Fs => "fs",
            Self::Net => "net",
            Self::Sec => "sec",
        }
    }

    /// Color para esta capa (HUD).
    pub fn color(self) -> u32 {
        match self {
            Self::Ring0 => 0xFFFF4444,    // rojo (crítico)
            Self::BmoCore => 0xFFFFAA00,  // naranja
            Self::BmoGpu => 0xFF00FFFF,   // cyan
            Self::Ring3 => 0xFF44FF44,    // verde
            Self::Lang => 0xFFAAFF00,     // verde-amarillo
            Self::Fs => 0xFFAA00FF,       // magenta
            Self::Net => 0xFF0088FF,      // azul
            Self::Sec => 0xFFFF0088,      // rosa
        }
    }

    /// Infiere la capa a partir del nombre del módulo.
    pub fn from_module(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.starts_with("ring0") || n.starts_with("cpu") || n.starts_with("mem") || n.starts_with("kbc") || n.starts_with("dev") || n == "boot" || n == "acpi" {
            Self::Ring0
        } else if n.starts_with("bmo_gpu") || n.starts_with("gpu") || n == "rdna4" {
            Self::BmoGpu
        } else if n.starts_with("lang") || n == "aot" || n == "linker" || n == "frontend" || n == "backend" {
            Self::Lang
        } else if n.starts_with("fs") || n == "vfat" || n == "exfat" || n == "ramdisk" {
            Self::Fs
        } else if n.starts_with("net") || n == "tcp" || n == "udp" || n == "socket" {
            Self::Net
        } else if n.starts_with("sec") || n == "cap" || n == "sandbox" {
            Self::Sec
        } else {
            // Por defecto: BMO Core.
            Self::BmoCore
        }
    }
}

/// Entidad (qué originó el evento).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Entity {
    /// Módulo genérico.
    Module   = 0,
    /// Proceso (PID).
    Process  = 1,
    /// Thread (TID).
    Thread   = 2,
    /// Syscall (BMO ABI nr).
    Syscall  = 3,
    /// Archivo (path).
    File     = 4,
    /// Cola de GPU (gfx/compute/sdma).
    GpuQueue = 5,
    /// Ventana.
    Window   = 6,
    /// Dispositivo (PCI bus:dev.fn).
    Device   = 7,
}

impl Entity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Process => "process",
            Self::Thread => "thread",
            Self::Syscall => "syscall",
            Self::File => "file",
            Self::GpuQueue => "gpu_queue",
            Self::Window => "window",
            Self::Device => "device",
        }
    }
}

/// Severidad de un evento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Severity {
    /// Información de boot/operación normal.
    Info    = 0,
    /// Mensaje de trace (debugging fino).
    Trace   = 1,
    /// Advertencia (no fatal).
    Warning = 2,
    /// Fault (error recuperable, e.g. #GP en syscall).
    Fault   = 3,
    /// Panic (no recuperable, e.g. triple fault incipiente).
    Panic   = 4,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Trace => "TRACE",
            Self::Warning => "WARN",
            Self::Fault => "FAULT",
            Self::Panic => "PANIC",
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Info => 0xFFCCCCCC,
            Self::Trace => 0xFF888888,
            Self::Warning => 0xFFFFFF00,
            Self::Fault => 0xFFFF8800,
            Self::Panic => 0xFFFF0000,
        }
    }
}

/// Un evento de la cabina.
#[derive(Clone, Debug)]
pub struct Event {
    pub seq: u64,
    pub tick_ns: u64,
    pub severity: Severity,
    pub layer: Layer,
    pub entity: Entity,
    pub module: String,
    pub entity_id: u32,    // PID, TID, syscall nr, file inode, etc.
    pub msg: String,
    pub value: u64,
}

impl Event {
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            tick_ns: 0,
            severity: Severity::Info,
            layer: Layer::BmoCore,
            entity: Entity::Module,
            module: String::new(),
            entity_id: 0,
            msg: String::new(),
            value: 0,
        }
    }
    pub fn new(severity: Severity, module: &str, msg: &str) -> Self {
        let mut e = Self::empty();
        e.severity = severity;
        e.module = String::from(module);
        e.layer = Layer::from_module(module);
        e.msg = String::from(msg);
        e
    }
    pub fn with_layer(mut self, layer: Layer) -> Self { self.layer = layer; self }
    pub fn with_entity(mut self, entity: Entity, id: u32) -> Self {
        self.entity = entity;
        self.entity_id = id;
        self
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val = if self.value != 0 { alloc::format!(" (0x{:x})", self.value) } else { String::new() };
        let eid = if self.entity_id != 0 {
            alloc::format!("[{}#{}]", self.entity.name(), self.entity_id)
        } else { String::new() };
        write!(f, "[{}|{}] {}: {}{}{}",
               self.layer.name(),
               self.severity.name(),
               self.module,
               self.msg,
               val,
               eid)
    }
}

// ─── Buffer circular ──────────────────────────────────────────────

pub struct Blackbox {
    events: [Event; BLACKBOX_CAP],
    head: AtomicU32,
    count: AtomicU32,
    seq: AtomicU64,
    initialized: AtomicBool,
}

static mut BLACKBOX: Blackbox = Blackbox {
    events: [const { Event::empty() }; BLACKBOX_CAP],
    head: AtomicU32::new(0),
    count: AtomicU32::new(0),
    seq: AtomicU64::new(0),
    initialized: AtomicBool::new(false),
};

pub mod buffer {
    use super::*;
    use crate::cabina::telemetry;

    pub fn init() {
        unsafe {
            BLACKBOX.initialized.store(true, Ordering::SeqCst);
            BLACKBOX.head.store(0, Ordering::SeqCst);
            BLACKBOX.count.store(0, Ordering::SeqCst);
            BLACKBOX.seq.store(0, Ordering::SeqCst);
        }
    }

    pub fn push(ev: &Event) {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return; }
            let pos = BLACKBOX.head.fetch_add(1, Ordering::Relaxed) as usize;
            let pos = pos % BLACKBOX_CAP;
            let mut stored = ev.clone();
            stored.seq = BLACKBOX.seq.fetch_add(1, Ordering::Relaxed);
            stored.tick_ns = {
                let tsc = crate::cpu::rdtsc();
                let freq = crate::cpu::tsc_per_sec();
                if freq == 0 { 0 } else { tsc.wrapping_mul(1_000_000_000) / freq }
            };
            // Si el evento es un fault/panic, incrementar telemetry.
            match stored.severity {
                Severity::Fault => { telemetry::cpu::inc_gp(); }
                Severity::Panic => { telemetry::cpu::inc_df(); }
                _ => {}
            }
            BLACKBOX.events[pos] = stored;
            let c = BLACKBOX.count.load(Ordering::Relaxed);
            if c < BLACKBOX_CAP as u32 {
                BLACKBOX.count.store(c + 1, Ordering::Relaxed);
            }
        }
    }

    pub fn latest() -> Option<Event> {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return None; }
            let count = BLACKBOX.count.load(Ordering::Relaxed);
            if count == 0 { return None; }
            let pos = (BLACKBOX.head.load(Ordering::Relaxed) as usize).wrapping_sub(1) % BLACKBOX_CAP;
            Some(BLACKBOX.events[pos].clone())
        }
    }

    pub fn last(n: usize) -> alloc::vec::Vec<Event> {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return alloc::vec::Vec::new(); }
            let count = BLACKBOX.count.load(Ordering::Relaxed) as usize;
            let n = n.min(count);
            let mut out = alloc::vec::Vec::with_capacity(n);
            let mut head = BLACKBOX.head.load(Ordering::Relaxed) as usize;
            for _ in 0..n {
                if head == 0 { head = BLACKBOX_CAP; }
                head -= 1;
                out.push(BLACKBOX.events[head].clone());
            }
            out
        }
    }

    pub fn iter() -> alloc::vec::Vec<Event> { last(BLACKBOX_CAP) }
    pub fn count() -> usize {
        unsafe { BLACKBOX.count.load(Ordering::Relaxed) as usize }
    }
}
