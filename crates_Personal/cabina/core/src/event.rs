use core::fmt;

// ─── Constants ────────────────────────────────────────────────────────────

pub const MODULE_MAX: usize = 32;
pub const MSG_MAX: usize = 128;

// ─── Severity ─────────────────────────────────────────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info = 0,
    Trace = 1,
    Warning = 2,
    Fault = 3,
    Panic = 4,
}

impl Severity {
    pub const COUNT: usize = 5;

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Info),
            1 => Some(Self::Trace),
            2 => Some(Self::Warning),
            3 => Some(Self::Fault),
            4 => Some(Self::Panic),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Trace => "TRACE",
            Self::Warning => "WARN",
            Self::Fault => "FAULT",
            Self::Panic => "PANIC",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ─── SeverityMask ─────────────────────────────────────────────────────────

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeverityMask(u8);

impl SeverityMask {
    pub const ALL: Self = Self(0x1F);
    pub const NONE: Self = Self(0);

    pub fn includes(self, s: Severity) -> bool {
        (self.0 & (1 << s as u8)) != 0
    }

    pub fn add(&mut self, s: Severity) {
        self.0 |= 1 << s as u8;
    }

    pub fn remove(&mut self, s: Severity) {
        self.0 &= !(1 << s as u8);
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn from_bits(bits: u8) -> Self {
        Self(bits & 0x1F)
    }
}

// ─── Layer ────────────────────────────────────────────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Ring0 = 0,
    BmoCore = 1,
    BmoGpu = 2,
    Ring3 = 3,
    Lang = 4,
    Fs = 5,
    Net = 6,
    Sec = 7,
}

impl Layer {
    pub const COUNT: usize = 8;

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Ring0),
            1 => Some(Self::BmoCore),
            2 => Some(Self::BmoGpu),
            3 => Some(Self::Ring3),
            4 => Some(Self::Lang),
            5 => Some(Self::Fs),
            6 => Some(Self::Net),
            7 => Some(Self::Sec),
            _ => None,
        }
    }

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

    /// Infiere la capa a partir del nombre del módulo.
    pub fn from_module(name: &str) -> Self {
        let n = name;
        if n.starts_with("ring0") || n.starts_with("cpu") || n.starts_with("mem")
            || n.starts_with("kbc") || n.starts_with("dev") || n == "boot" || n == "acpi"
        {
            Self::Ring0
        } else if n.starts_with("ring3") {
            Self::Ring3
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
            Self::BmoCore
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ─── LayerMask ────────────────────────────────────────────────────────────

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerMask(u8);

impl LayerMask {
    pub const ALL: Self = Self(0xFF);
    pub const NONE: Self = Self(0);

    pub fn includes(self, l: Layer) -> bool {
        (self.0 & (1 << l as u8)) != 0
    }

    pub fn add(&mut self, l: Layer) {
        self.0 |= 1 << l as u8;
    }

    pub fn remove(&mut self, l: Layer) {
        self.0 &= !(1 << l as u8);
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

// ─── Entity ───────────────────────────────────────────────────────────────

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Module = 0,
    Process = 1,
    Thread = 2,
    Syscall = 3,
    File = 4,
    GpuQueue = 5,
    Window = 6,
    Device = 7,
}

impl Entity {
    pub const COUNT: usize = 8;

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Module),
            1 => Some(Self::Process),
            2 => Some(Self::Thread),
            3 => Some(Self::Syscall),
            4 => Some(Self::File),
            5 => Some(Self::GpuQueue),
            6 => Some(Self::Window),
            7 => Some(Self::Device),
            _ => None,
        }
    }

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

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ─── EntityMask ───────────────────────────────────────────────────────────

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityMask(u8);

impl EntityMask {
    pub const ALL: Self = Self(0xFF);
    pub const NONE: Self = Self(0);

    pub fn includes(self, e: Entity) -> bool {
        (self.0 & (1 << e as u8)) != 0
    }

    pub fn add(&mut self, e: Entity) {
        self.0 |= 1 << e as u8;
    }

    pub fn remove(&mut self, e: Entity) {
        self.0 &= !(1 << e as u8);
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

// ─── Event ────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    pub seq: u64,
    pub tick_ns: u64,
    pub severity: Severity,
    pub layer: Layer,
    pub entity: Entity,
    pub _pad: u8,
    pub module: [u8; MODULE_MAX],
    pub entity_id: u32,
    pub msg: [u8; MSG_MAX],
    pub value: u64,
}

impl Event {
    pub fn new(
        severity: Severity,
        layer: Layer,
        entity: Entity,
        module: &str,
        entity_id: u32,
        msg: &str,
        value: u64,
    ) -> Self {
        let mut ev = Self {
            seq: 0,
            tick_ns: 0,
            severity,
            layer,
            entity,
            _pad: 0,
            module: [0; MODULE_MAX],
            entity_id,
            msg: [0; MSG_MAX],
            value,
        };
        str_to_fixed(module, &mut ev.module);
        str_to_fixed(msg, &mut ev.msg);
        ev
    }

    pub fn module_str(&self) -> &str {
        fixed_to_str(&self.module)
    }

    pub fn msg_str(&self) -> &str {
        fixed_to_str(&self.msg)
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("seq", &self.seq)
            .field("tick_ns", &self.tick_ns)
            .field("severity", &self.severity)
            .field("layer", &self.layer)
            .field("entity", &self.entity)
            .field("module", &self.module_str())
            .field("entity_id", &self.entity_id)
            .field("msg", &self.msg_str())
            .field("value", &self.value)
            .finish()
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}|{}] {}: {} (0x{:X})[{}#{}]",
            self.layer,
            self.severity,
            self.module_str(),
            self.msg_str(),
            self.value,
            self.entity,
            self.entity_id,
        )
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq
    }
}

impl Eq for Event {}

// ─── Fixed-size string helpers ───────────────────────────────────────────

pub fn str_to_fixed<const N: usize>(s: &str, buf: &mut [u8; N]) {
    let len = s.len().min(N - 1);
    buf[..len].copy_from_slice(&s.as_bytes()[..len]);
    buf[len] = 0;
}

pub fn fixed_to_str<const N: usize>(buf: &[u8; N]) -> &str {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(N);
    core::str::from_utf8(&buf[..end]).unwrap_or("")
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_roundtrip() {
        for s in [Severity::Info, Severity::Trace, Severity::Warning, Severity::Fault, Severity::Panic] {
            assert_eq!(Severity::from_u8(s as u8), Some(s));
            assert_eq!(s.name().len() > 0, true);
        }
        assert_eq!(Severity::from_u8(255), None);
    }

    #[test]
    fn severity_mask_works() {
        let mut mask = SeverityMask::ALL;
        assert!(mask.includes(Severity::Info));
        assert!(mask.includes(Severity::Panic));
        mask.remove(Severity::Info);
        assert!(!mask.includes(Severity::Info));
        assert!(mask.includes(Severity::Trace));
        assert_eq!(mask.bits(), 0x1E);
        let mask2 = SeverityMask::from_bits(0x1E);
        assert_eq!(mask, mask2);
    }

    #[test]
    fn layer_entity_roundtrip() {
        assert_eq!(Layer::from_u8(0), Some(Layer::Ring0));
        assert_eq!(Layer::from_u8(7), Some(Layer::Sec));
        assert_eq!(Layer::from_u8(8), None);
        assert_eq!(Entity::from_u8(0), Some(Entity::Module));
        assert_eq!(Entity::from_u8(7), Some(Entity::Device));
        assert_eq!(Entity::from_u8(8), None);
    }

    #[test]
    fn layer_mask_works() {
        let mut mask = LayerMask::ALL;
        assert!(mask.includes(Layer::Ring0));
        assert!(mask.includes(Layer::Net));
        mask.remove(Layer::Ring0);
        assert!(!mask.includes(Layer::Ring0));
        let mask2 = LayerMask::from_bits(mask.bits());
        assert_eq!(mask, mask2);
    }

    #[test]
    fn event_creation_and_strings() {
        let ev = Event::new(
            Severity::Info,
            Layer::Ring0,
            Entity::Module,
            "test_mod",
            42,
            "hello world",
            0xABCD,
        );
        assert_eq!(ev.module_str(), "test_mod");
        assert_eq!(ev.msg_str(), "hello world");
        assert_eq!(ev.value, 0xABCD);
        assert_eq!(ev.entity_id, 42);
        assert_eq!(ev.severity, Severity::Info);
        assert_eq!(ev.layer, Layer::Ring0);
    }

    #[test]
    fn event_truncation() {
        let long_msg = "a".repeat(MSG_MAX + 50);
        let ev = Event::new(
            Severity::Warning,
            Layer::Ring0,
            Entity::Module,
            "", 0, &long_msg, 0,
        );
        let msg = ev.msg_str();
        assert!(msg.len() <= MSG_MAX - 1);
        assert!(msg.chars().all(|c| c == 'a'));
    }

    #[test]
    fn event_display_format() {
        let ev = Event::new(
            Severity::Fault,
            Layer::BmoCore,
            Entity::Syscall,
            "sys", 7, "segfault", 0xDEAD,
        );
        let mut buf = [0u8; 256];
        let mut len = 0usize;
        {
            use core::fmt::Write;
            struct BufWriter<'a>(&'a mut [u8], &'a mut usize);
            impl Write for BufWriter<'_> {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    let n = s.len().min(self.0.len().saturating_sub(*self.1));
                    self.0[*self.1..*self.1 + n].copy_from_slice(&s.as_bytes()[..n]);
                    *self.1 += n;
                    Ok(())
                }
            }
            write!(BufWriter(&mut buf, &mut len), "{}", ev).unwrap();
        }
        let s = core::str::from_utf8(&buf[..len]).unwrap();
        assert!(s.contains("FAULT"));
        assert!(s.contains("bmo_core"));
        assert!(s.contains("segfault"));
        assert!(s.contains("0xDEAD"));
        assert!(s.contains("syscall#7"));
    }

    #[test]
    fn event_eq_by_seq() {
        let a = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "a", 0, "x", 0);
        let b = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "b", 0, "y", 0);
        // seq is 0 for both → they compare equal
        assert_eq!(a, b);
    }

    #[test]
    fn fixed_to_str_empty() {
        let buf = [0u8; 32];
        assert_eq!(fixed_to_str(&buf), "");
    }

    #[test]
    fn str_to_fixed_full() {
        let mut buf = [0u8; 8];
        str_to_fixed("hello", &mut buf);
        assert_eq!(&buf[..6], b"hello\0");
    }

    #[test]
    fn event_copy_works() {
        let a = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "m", 1, "msg", 99);
        let b = a;
        assert_eq!(a.module_str(), b.module_str());
        assert_eq!(a.msg_str(), b.msg_str());
        assert_eq!(a.value, b.value);
    }

    #[test]
    fn event_repr_c() {
        use core::mem;
        assert_eq!(mem::align_of::<Event>(), 8);
    }
}
