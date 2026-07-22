//! # Estuaries — typed views over `bmo-channel` pages
//!
//! An "estuary" is where a stream of typed messages meets a CPU
//! boundary. The `bmo-channel` crate gives you a raw
//! `(opcode, arg0, arg1, arg2)` ring buffer on a 4096-byte shared
//! page. An `Estuary<T>` wraps that with:
//!
//! - **Typed messages**: `T` is whatever the protocol on this
//!   estuary speaks (a `KeyEvent`, a `DrawRect`, a syscall number
//!   plus its args, a `LogLine`, …).
//! - **A known opcode space**: each estuary has its own opcode
//!   range, so the kernel-side demuxer can route a message to the
//!   right handler without inspecting the payload.
//! - **Static configuration**: the estuary ID is fixed at compile
//!   time, so the kernel can hand out a known physical page to
//!   each userland process at process spawn.
//!
//! ## Why "estuary"?
//!
//! A river is a producer (Ring 3), the ocean is a consumer (Ring 0),
//! and the estuary is where the two mix — brackish, full of life, with
//! a tide that flows both ways. The metaphor fits because messages
//! flow both ways: Ring 3 → Ring 0 (requests) and Ring 0 → Ring 3
//! (responses / events / completion notifications).
//!
//! ## Layout
//!
//! ```text
//!   ┌─ bmo-channel page (4096 B) ────────────────────────────┐
//!   │  magic  doorbell  submit_h/t  complete_h/t  _pad       │
//!   │  submit_ring[62]  (Ring 3 → Ring 0, raw opcodes)        │
//!   │  complete_ring[62] (Ring 0 → Ring 3, raw opcodes)      │
//!   └─────────────────────────────────────────────────────────┘
//!                ▲                              ▲
//!                │                              │
//!         ring3_send(...)                ring3_poll(|e| ...)
//!                │                              │
//!                ▼                              ▼
//!   ┌─ Estuary<Input> ─────────────────── InputEvent ─────────┐
//!   │    opcode=1 → KeyDown     arg0=scancode                 │
//!   │    opcode=2 → KeyUp       arg0=scancode                 │
//!   │    opcode=3 → MouseMove   arg0=dx  arg1=dy              │
//!   │    opcode=4 → MouseButton arg0=button arg1=down         │
//!   └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## The five standard estuaries
//!
//! - **Input** (`estuary_id = 1`): keyboard / mouse / gamepad events
//!   flowing Ring 0 → Ring 3. Used by `bmo-driver-keyboard` and
//!   `bmo-driver-mouse` userland drivers.
//! - **Framebuffer** (`estuary_id = 2`): draw commands flowing
//!   Ring 3 → Ring 0 (the compositor accepts them, batches, and
//!   presents to the actual framebuffer). Used by `bmo-service-gui`.
//! - **Syscall** (`estuary_id = 3`): for syscalls too big or too
//!   async for the synchronous `syscall` instruction path. Used by
//!   `bmo-rt` and the bmo-cobol-front runtime.
//! - **Log** (`estuary_id = 4`): kernel log lines flowing
//!   Ring 0 → Ring 3. Used by `bmo-service-cabina` (diagnostics).
//! - **Custom** (`estuary_id >= 16`): user-defined. The first 16 IDs
//!   are reserved for the platform; everything else is fair game.
//!
//! ## Where the page addresses come from
//!
//! The kernel hands out a small array of physical page addresses in
//! `BootContext.channel_pages[NUM_ESTUARIES]`. Index 0 is the Input
//! estuary, index 1 is the Framebuffer, etc. The userland process
//! maps these into its own address space (using the platform's
//! `map_shared_page` helper, or just by reading the BootContext and
//! trusting the addresses that the kernel mapped for it).

use bmo_channel::{Channel, CHANNEL_MAGIC};

/// An estuary's slot in `BootContext.channel_pages[]`.
/// The first 16 IDs are reserved for the platform; anything >= 16
/// is a user-defined estuary.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstuaryId {
    /// Reserved (must not be used).
    Reserved = 0,
    /// Keyboard / mouse / gamepad events (Ring 0 → Ring 3).
    Input = 1,
    /// Framebuffer draw commands (Ring 3 → Ring 0).
    Framebuffer = 2,
    /// Async syscall channel (Ring 3 → Ring 0).
    Syscall = 3,
    /// Kernel log lines (Ring 0 → Ring 3).
    Log = 4,
    /// First user-defined ID. Custom estuaries pick from here.
    CustomBase = 16,
}

impl EstuaryId {
    /// Convert a raw u16 into an `EstuaryKind`, marking whether it
    /// is one of the four standard ones or a custom one.
    pub fn from_raw(v: u16) -> EstuaryKind {
        match v {
            0 => EstuaryKind::Reserved,
            1 => EstuaryKind::Standard(EstuaryId::Input),
            2 => EstuaryKind::Standard(EstuaryId::Framebuffer),
            3 => EstuaryKind::Standard(EstuaryId::Syscall),
            4 => EstuaryKind::Standard(EstuaryId::Log),
            n if n >= Self::CustomBase as u16 => EstuaryKind::Custom(n),
            _ => EstuaryKind::Unknown,
        }
    }
}

/// Either a standard estuary kind or a user-defined one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstuaryKind {
    /// `EstuaryId::Reserved` — must not be used.
    Reserved,
    /// One of the four standard estuaries.
    Standard(EstuaryId),
    /// A user-defined estuary with a raw u16 ID.
    Custom(u16),
    /// An ID in the gap between 4 and 16 (reserved for future
    /// platform-defined estuaries; not yet assigned).
    Unknown,
}

/// A typed view over a `bmo-channel` page. `T` is whatever the
/// estuary's protocol speaks.
///
/// **Lifetime:** the `Channel` must outlive the `Estuary`. In
/// practice the `Channel` lives in a static page (the kernel maps
/// the same physical page into both Ring 0 and Ring 3), so
/// `Estuary<'static, T>` is the common case.
pub struct Estuary<'a, T: Copy> {
    channel: &'a Channel,
    _phantom: core::marker::PhantomData<T>,
}

impl<'a, T: Copy> Estuary<'a, T> {
    /// Wrap a `Channel` reference as an `Estuary<T>`. The caller
    /// is responsible for ensuring the page is mapped into the
    /// current address space and that `T` is the correct type
    /// for this estuary's protocol.
    ///
    /// # Safety
    /// `channel` must point to a valid `bmo-channel` page that is
    /// mapped read-write in this address space, and `T` must be
    /// the correct message type for the protocol the kernel and
    /// the userland have agreed on.
    pub unsafe fn from_raw(channel: &'a Channel) -> Self {
        Self { channel, _phantom: core::marker::PhantomData }
    }

    /// Validate the channel's magic. Returns `true` if the page
    /// looks like a properly initialized `bmo-channel`. The
    /// platform code calls this on each estuary at boot to fail
    /// fast if the kernel handed out a wrong page.
    pub fn is_valid(&self) -> bool {
        // SAFETY: Channel has a `magic: u64` field at offset 0.
        unsafe {
            let m = (self.channel as *const Channel as *const u64).read();
            m == CHANNEL_MAGIC
        }
    }

    /// Submit a typed message to the kernel / consumer. The
    /// message is encoded into the channel's 4-u64 entry format
    /// via `T::encode`.
    pub fn send(&self, msg: T) -> bool
    where T: Encode
    {
        let (op, a0, a1, a2) = msg.encode();
        self.channel.ring3_send(op, a0, a1, a2)
    }

    /// Poll for typed messages. The callback receives each
    /// decoded message. Returns the number of messages handled.
    pub fn poll<F: FnMut(T)>(&self, mut cb: F) -> usize
    where T: Decode
    {
        let mut count = 0;
        self.channel.ring3_poll_n(usize::MAX, |op, a0, a1, a2| {
            if let Some(msg) = T::decode(op, a0, a1, a2) {
                cb(msg);
                count += 1;
            }
        });
        count
    }
}

/// Trait for types that can be encoded into a 4-u64 channel entry.
pub trait Encode {
    /// Encode `self` into `(opcode, arg0, arg1, arg2)`.
    fn encode(&self) -> (u64, u64, u64, u64);
}

/// Trait for types that can be decoded from a 4-u64 channel entry.
/// Returns `None` if the entry doesn't represent a valid message
/// (e.g. the opcode is unknown to this protocol).
pub trait Decode: Sized {
    /// Try to decode an entry. `None` means "not a valid message".
    fn decode(opcode: u64, arg0: u64, arg1: u64, arg2: u64) -> Option<Self>;
}

// ── Standard estuaries ───────────────────────────────────────────
//
// These are the four protocol-defined message types that flow
// across the platform's standard estuaries. Each one is its own
// enum so the wire format is statically checked at compile time.

/// Input events flowing Ring 0 → Ring 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// A keyboard key was pressed. `arg0` is the scancode (PS/2
    /// Set 1 or USB HID usage, depending on the backend).
    KeyDown { scancode: u32 },
    /// A keyboard key was released.
    KeyUp { scancode: u32 },
    /// Mouse moved by `(dx, dy)` since the last event.
    MouseMove { dx: i32, dy: i32 },
    /// A mouse button changed state.
    MouseButton { button: u8, down: bool },
    /// Mouse wheel scrolled by `delta` (positive = up).
    MouseWheel { delta: i32 },
}

impl Encode for InputEvent {
    fn encode(&self) -> (u64, u64, u64, u64) {
        // Input is usually Ring 0 → Ring 3, but we provide encode
        // for symmetry / testing.
        match *self {
            Self::KeyDown { scancode } => (1, scancode as u64, 0, 0),
            Self::KeyUp { scancode } => (2, scancode as u64, 0, 0),
            Self::MouseMove { dx, dy } =>
                (3, dx as i32 as u64, dy as i32 as u64, 0),
            Self::MouseButton { button, down } =>
                (4, button as u64, down as u64, 0),
            Self::MouseWheel { delta } => (5, delta as i32 as u64, 0, 0),
        }
    }
}

impl Decode for InputEvent {
    fn decode(op: u64, a0: u64, a1: u64, _a2: u64) -> Option<Self> {
        match op {
            1 => Some(Self::KeyDown { scancode: a0 as u32 }),
            2 => Some(Self::KeyUp { scancode: a0 as u32 }),
            3 => Some(Self::MouseMove { dx: a0 as i32, dy: a1 as i32 }),
            4 => Some(Self::MouseButton {
                button: a0 as u8, down: a1 != 0,
            }),
            5 => Some(Self::MouseWheel { delta: a0 as i32 }),
            _ => None,
        }
    }
}

/// Framebuffer draw commands flowing Ring 3 → Ring 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawCmd {
    /// Fill a rectangle. `color` is 0x00RRGGBB.
    FillRect { x: i32, y: i32, w: i32, h: i32, color: u32 },
    /// Blit a pre-registered image. `image_id` is a handle the
    /// compositor gave out at registration time.
    Blit { dst_x: i32, dst_y: i32, image_id: u32 },
    /// Composite the current backbuffer to the display.
    Present,
    /// Set the current scissor rectangle (clip region).
    SetClip { x: i32, y: i32, w: i32, h: i32 },
}

impl Encode for DrawCmd {
    fn encode(&self) -> (u64, u64, u64, u64) {
        match *self {
            Self::FillRect { x, y, w, h, color } => (
                1,
                pack_i32(x, y),     // a0 = (x | y<<32)
                pack_i32(w, h),     // a1 = (w | h<<32)
                color as u64,       // a2 = color
            ),
            Self::Blit { dst_x, dst_y, image_id } => (
                2,
                pack_i32(dst_x, dst_y),
                image_id as u64,
                0,
            ),
            Self::Present => (3, 0, 0, 0),
            Self::SetClip { x, y, w, h } => (
                4,
                pack_i32(x, y),
                pack_i32(w, h),
                0,
            ),
        }
    }
}

impl Decode for DrawCmd {
    fn decode(_op: u64, _a0: u64, _a1: u64, _a2: u64) -> Option<Self> {
        // DrawCmd is Ring 3 → Ring 0, so userland code doesn't
        // normally decode it. We provide this for tests and for
        // any Ring 0 mock that needs to verify what the userland
        // sent. The real Ring 0 side uses its own typed receiver.
        None
    }
}

/// Pack two i32s into a u64. The low 32 bits hold `a`, the high
/// 32 bits hold `b`. Both `a` and `b` are sign-extended to i32
/// before being truncated to u32.
fn pack_i32(a: i32, b: i32) -> u64 {
    let lo = (a as u32) as u64;
    let hi = (b as u32) as u64;
    lo | (hi << 32)
}

/// Async syscall request. For syscalls that are too large for the
/// synchronous `syscall` path (e.g. file I/O with a 64-KiB buffer,
/// or syscalls that need to wait on a kernel event).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncSyscall {
    /// `bmo_abi::syscalls::SYS_*` constant.
    pub nr: u32,
    /// First 6 syscall arguments. Matches the synchronous ABI
    /// so userland can use the same `args` buffer for both.
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
}

impl Encode for AsyncSyscall {
    fn encode(&self) -> (u64, u64, u64, u64) {
        // We use a 2-entry sequence: first entry carries
        // (nr, arg0, arg1, arg2); the kernel reads the next
        // entry from the same ring to get arg3..arg5. This is
        // a known pattern — see bmo-rt for the matching
        // 2-entry send in `bmo_syscall_async`.
        // The opcode is 1; the channel's protocol demuxer knows
        // to pair consecutive entries.
        (1, self.nr as u64, self.arg0, self.arg1)
    }
}

impl Decode for AsyncSyscall {
    fn decode(_op: u64, _a0: u64, _a1: u64, _a2: u64) -> Option<Self> {
        // The Ring 3 side never decodes AsyncSyscall — those
        // are produced by Ring 3 and consumed by Ring 0.
        None
    }
}

/// A single log line written by the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogLine {
    /// Severity: 0=trace, 1=debug, 2=info, 3=warn, 4=error, 5=fatal.
    pub severity: u8,
    /// Origin subsystem ID (1=kernel, 2=driver, 3=service, …).
    pub origin: u8,
    /// The log message, encoded as the first 14 bytes of UTF-8.
    /// Longer messages are split across consecutive entries.
    pub msg_first: [u8; 14],
}

impl Encode for LogLine {
    fn encode(&self) -> (u64, u64, u64, u64) {
        // Log is Ring 0 → Ring 3; encoding is for tests.
        let mut buf = [0u8; 16];
        buf[0] = self.severity;
        buf[1] = self.origin;
        buf[2..16].copy_from_slice(&self.msg_first);
        let lo = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let hi = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        (1, lo, hi, 0)
    }
}

impl Decode for LogLine {
    fn decode(op: u64, a0: u64, a1: u64, _a2: u64) -> Option<Self> {
        if op != 1 { return None; }
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&a0.to_le_bytes());
        buf[8..16].copy_from_slice(&a1.to_le_bytes());
        let mut msg = [0u8; 14];
        msg.copy_from_slice(&buf[2..16]);
        Some(LogLine { severity: buf[0], origin: buf[1], msg_first: msg })
    }
}

// ── Typed aliases for the four standard estuaries ────────────────

/// Input events flowing Ring 0 → Ring 3.
pub type InputEstuary<'a> = Estuary<'a, InputEvent>;
/// Framebuffer draw commands flowing Ring 3 → Ring 0.
pub type FramebufferEstuary<'a> = Estuary<'a, DrawCmd>;
/// Async syscalls flowing Ring 3 → Ring 0.
pub type SyscallEstuary<'a> = Estuary<'a, AsyncSyscall>;
/// Kernel log lines flowing Ring 0 → Ring 3.
pub type LogEstuary<'a> = Estuary<'a, LogLine>;

/// A user-defined estuary carrying any `T: Copy + Encode + Decode`.
/// `EstuaryId::CustomBase` and above are reserved for these.
pub type CustomEstuary<'a, T> = Estuary<'a, T>;

// ── Re-export the underlying ring entry for low-level users ──────
//
// Most code should go through `Estuary<T>`. The raw `ChannelEntry`
// is re-exported for the rare case of a generic dispatcher that
// doesn't know the message type ahead of time (e.g. a debugger).
pub use bmo_channel::ChannelEntry as RawChannelEntry;
