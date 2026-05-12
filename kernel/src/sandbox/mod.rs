//! Sandbox de aplicaciones BEF.
//!
//! Spec: `FastOS_App_Sandbox.md` + `FastOS_Security_Model.md`.
//! Cada proceso BEF recibe un set de capabilities declarado en su
//! manifest TOML. El kernel rechaza cualquier syscall que pida acceso
//! a recursos no concedidos.

#![allow(dead_code)]

bitflags::bitflags! {
    /// Capabilities atómicas que un proceso puede solicitar.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capability: u64 {
        const NONE              = 0;

        // VFS
        const FS_READ           = 1 << 0;
        const FS_WRITE          = 1 << 1;
        const FS_EXEC           = 1 << 2;

        // Network
        const NET_OUTBOUND      = 1 << 8;
        const NET_INBOUND       = 1 << 9;
        const NET_RAW           = 1 << 10;

        // Graphics
        const GFX_BAREX         = 1 << 16;
        const GFX_EXCLUSIVE     = 1 << 17;
        const GFX_KERNEL_BYPASS = 1 << 18;

        // Audio
        const AUDIO_PLAYBACK    = 1 << 24;
        const AUDIO_CAPTURE     = 1 << 25;
        const AUDIO_EXCLUSIVE   = 1 << 26;

        // Input
        const INPUT_RAW         = 1 << 32;
        const INPUT_HOTKEYS     = 1 << 33;

        // System
        const SYS_TIME_HIRES    = 1 << 48;
        const SYS_PERF_COUNTER  = 1 << 49;
        const SYS_DEBUG         = 1 << 56;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessCaps {
    pub caps: Capability,
}

impl ProcessCaps {
    pub const fn empty() -> Self { Self { caps: Capability::NONE } }
    pub fn allows(&self, c: Capability) -> bool { self.caps.contains(c) }
}
