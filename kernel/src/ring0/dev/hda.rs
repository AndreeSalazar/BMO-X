//! HD Audio (Intel HDA / Realtek ALC) driver.
//!
//! Controls the onboard audio controller found on most AMD/Intel chipsets.
//! Provides volume control for 7.1 headphones via codec verbs.
//!
//! ## Architecture
//!
//! ```text
//! PCI device (class=0x04, subclass=0x03)
//!   ├── MMIO registers (GCTL, CORB, RIRB, stream descriptors)
//!   ├── CORB (Command Output Ring Buffer) → sends verbs to codec
//!   ├── RIRB (Response Input Ring Buffer)  ← receives responses
//!   └── Codec (Realtek ALC) → amplifier, DAC, jack detection
//! ```
//!
//! ## Codec verbs used
//!
//! - SET_AMPLIFIER_GAIN_MUTE (0x3) — volume control per widget
//! - SET_PIN_WIDGET_CONTROL (0x707) — enable/disable output pin
//! - GET_CONNECTION_LIST_ENTRY (0xF02) — widget topology discovery

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, AtomicBool, Ordering};

// ── PCI ─────────────────────────────────────────────────────────────

const HDA_CLASS: u8 = 0x04;
const HDA_SUBCLASS: u8 = 0x03;

// ── MMIO Registers (offsets from BAR0) ──────────────────────────────

const REG_GCAP:        u32 = 0x00;  // Global Capabilities
const REG_VMIN:        u32 = 0x02;  // Minor Version
const REG_VMAJ:        u32 = 0x03;  // Major Version
const REG_OUTPAY:      u32 = 0x04;  // Output Payload Capacity
const REG_INPAY:       u32 = 0x06;  // Input Payload Capacity
const REG_GCTL:        u32 = 0x08;  // Global Control (CRST bit 0)
const REG_WAKEEN:      u32 = 0x0C;  // Wake Enable
const REG_STATESTS:    u32 = 0x0E;  // State Change Status
const REG_INTCTL:      u32 = 0x20;  // Interrupt Control
const REG_CORBLBASE:   u32 = 0x40;  // CORB Lower Base Address
const REG_CORBUBASE:   u32 = 0x44;  // CORB Upper Base Address
const REG_CORBWP:      u32 = 0x48;  // CORB Write Pointer
const REG_CORBRP:      u32 = 0x4A;  // CORB Read Pointer
const REG_CORBCTL:     u32 = 0x4C;  // CORB Control
const REG_CORBSTS:     u32 = 0x4D;  // CORB Status
const REG_CORBSIZE:    u32 = 0x4E;  // CORB Size
const REG_RIRBLBASE:   u32 = 0x50;  // RIRB Lower Base Address
const REG_RIRBUBASE:   u32 = 0x54;  // RIRB Upper Base Address
const REG_RIRBWP:     u32 = 0x58;  // RIRB Write Pointer
const REG_RIRBSTS:     u32 = 0x5D;  // RIRB Status
const REG_DPLBASE:     u32 = 0x70;  // DMA Position Lower Base
const REG_DPUBASE:     u32 = 0x74;  // DMA Position Upper Base
const REG_SD_BASE:     u32 = 0x80;  // Stream Descriptor 0

// ── GCTL ─────────────────────────────────────────────────────────────

const GCTL_CRST: u32 = 1;  // Controller Reset

// ── CORBCTL ──────────────────────────────────────────────────────────

const CORBCTL_RUN: u8 = 2;  // Enable CORB DMA engine

// ── RIRBCTL ──────────────────────────────────────────────────────────

const RIRBCTL_RUN: u8 = 2;  // Enable RIRB DMA engine
const RIRBCTL_RINT: u8 = 1; // Enable RIRB interrupts

// ── Codec verbs (32-bit, sent via CORB) ──────────────────────────────

/// Build a 32-bit codec verb: [31:28]=codec_addr, [27:20]=node_id, [19:0]=verb+payload
fn verb(codec: u8, node: u8, cmd: u32, payload: u8) -> u32 {
    ((codec as u32) << 28) | ((node as u32) << 20) | (cmd << 8) | (payload as u32)
}

const VERB_GET_PARAMETER:           u32 = 0xF00;
const VERB_SET_AMPLIFIER_GAIN_MUTE: u32 = 0x3;
const VERB_SET_PIN_WIDGET_CONTROL:   u32 = 0x707;
const VERB_GET_CONNECTION_LIST:      u32 = 0xF02;
const VERB_GET_CONNECTION_SELECT:    u32 = 0xF01;
const VERB_SET_CONNECTION_SELECT:    u32 = 0x701;

// ── Parameters ───────────────────────────────────────────────────────

const PARAM_AUDIO_WIDGET_CAP:       u32 = 0x09;
const PARAM_OUTPUT_AMP_CAP:         u32 = 0x12;
const PARAM_CONNECTION_LIST_LEN:    u32 = 0x0E;

// ── Widget types ─────────────────────────────────────────────────────

const WIDGET_AUDIO_OUTPUT: u32 = 0x0;
const WIDGET_PIN_COMPLEX:  u32 = 0x4;
const WIDGET_AUDIO_MIXER:  u32 = 0x2;

// ── Pin widget control ───────────────────────────────────────────────

const PIN_OUT_ENABLE: u8   = 0x40;  // Output enable
const PIN_HP_ENABLE: u8    = 0x80;  // Headphone amp enable

// ── Amplifier gain (0-64 range, 0.5dB steps) ────────────────────────

const AMP_MUTE: u8   = 0x80;  // Mute bit
const AMP_LEFT: u8   = 0x20;  // Left channel
const AMP_RIGHT: u8  = 0x10;  // Right channel
const AMP_BOTH: u8   = 0x30;  // Both channels
const AMP_GAIN_MAX: u8 = 64;

// ── Driver state ─────────────────────────────────────────────────────

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static VOLUME_LEVEL: AtomicU8 = AtomicU8::new(32); // 0-64 range, default 50%

/// MMIO base address of the HDA controller (set after PCI scan).
static mut MMIO_BASE: u64 = 0;
/// Codec address (0-15, usually 0).
static mut CODEC_ADDR: u8 = 0;
/// Output amplifier node ID (discovered from widget tree).
static mut OUTPUT_NODE: u8 = 0;
/// Output pin node ID.
static mut OUTPUT_PIN: u8 = 0;

/// Initialize the HD Audio driver with the controller's MMIO base.
pub fn init(mmio_base: u64) {
    if mmio_base == 0 { return; }
    if INITIALIZED.swap(true, Ordering::SeqCst) { return; }

    unsafe { MMIO_BASE = mmio_base; }

    crate::dev::console::serial_write("[hda] initializing HD Audio controller\n");

    // 1. Reset controller
    unsafe {
        hda_write32(REG_GCTL, 0);
        crate::cpu::busy_wait_ms(10);
        hda_write32(REG_GCTL, GCTL_CRST);
    }
    // Wait for CRST bit to read back as 1
    for _ in 0..100 {
        if unsafe { hda_read32(REG_GCTL) } & GCTL_CRST != 0 { break; }
        crate::cpu::busy_wait_ms(1);
    }
    crate::dev::console::serial_write("[hda] controller reset complete\n");

    // 2. Discover codec
    let cap = unsafe { hda_read16(REG_GCAP) };
    let num_codecs = ((cap >> 0) & 0xF) as u8;
    crate::dev::console::serial_write("[hda] ");
    crate::dev::console::serial_write_u64(num_codecs as u64, 10);
    crate::dev::console::serial_write(" codec(s) detected\n");
    unsafe { CODEC_ADDR = 0; }

    // 3. Set up CORB/RIRB — simple polling mode
    unsafe {
        hda_write8(REG_CORBSIZE, 0x42);  // 256 entries
        hda_write8(REG_CORBCTL, CORBCTL_RUN);
    }
    crate::cpu::busy_wait_ms(10);

    // 4. Discover output path
    discover_output_path();
    crate::dev::console::serial_write("[hda] output path discovered: node=");
    crate::dev::console::serial_write_u64(unsafe { OUTPUT_NODE as u64 }, 16);
    crate::dev::console::serial_write(", pin=");
    crate::dev::console::serial_write_u64(unsafe { OUTPUT_PIN as u64 }, 16);
    crate::dev::console::serial_write("\n");

    // 5. Enable output pin
    if unsafe { OUTPUT_PIN != 0 } {
        let val = PIN_OUT_ENABLE | PIN_HP_ENABLE;
        send_verb(verb(unsafe { CODEC_ADDR }, unsafe { OUTPUT_PIN }, VERB_SET_PIN_WIDGET_CONTROL, val));
    }

    // 6. Unmute and set initial volume
    set_volume_raw(32);
    crate::dev::console::serial_write("[hda] initialized with volume=50%\n");
}

/// Set volume level (0-100). 0 = mute, 100 = max.
pub fn set_volume(level: u8) {
    let lv = level.min(100);
    let gain = ((lv as u32 * AMP_GAIN_MAX as u32) / 100) as u8;
    VOLUME_LEVEL.store(gain, Ordering::Relaxed);
    set_volume_raw(gain);
}

/// Get current volume (0-100).
pub fn get_volume() -> u8 {
    let gain = VOLUME_LEVEL.load(Ordering::Relaxed);
    ((gain as u32 * 100) / AMP_GAIN_MAX as u32) as u8
}

/// Set raw amplifier gain (0-64).
fn set_volume_raw(gain: u8) {
    if unsafe { OUTPUT_NODE == 0 } { return; }
    let g = gain.min(AMP_GAIN_MAX);
    let payload = if g == 0 {
        AMP_BOTH | AMP_MUTE  // mute both channels
    } else {
        AMP_BOTH | g        // both channels at gain
    };
    let v = verb(unsafe { CODEC_ADDR }, unsafe { OUTPUT_NODE }, VERB_SET_AMPLIFIER_GAIN_MUTE, payload);
    send_verb(v);
}

/// Send a codec verb and return the response (32-bit).
/// Uses a simple polling approach: write verb, poll for response.
fn send_verb(verb: u32) -> u32 {
    unsafe {
        // Write verb directly to CORB at the current write pointer
        let wp = hda_read16(REG_CORBWP) as usize & 0xFF;
        let corb_base = hda_read32(REG_CORBLBASE);
        if corb_base != 0 {
            let entry = (corb_base as usize + wp * 4) as *mut u32;
            core::ptr::write_volatile(entry, verb);
        }
        // Advance CORB write pointer
        hda_write16(REG_CORBWP, (wp as u16).wrapping_add(1));

        // Poll for response in RIRB
        for _ in 0..5000 {
            let wp_rirb = hda_read16(REG_RIRBWP) as usize & 0xFF;
            if wp_rirb > 0 {
                let rirb_base = hda_read32(REG_RIRBLBASE);
                if rirb_base != 0 {
                    // RIRB entry: 8 bytes — response (4 bytes) + extended (4 bytes)
                    let resp_ptr = (rirb_base as usize + (wp_rirb.wrapping_sub(1) & 0xFF) * 8 + 4) as *const u32;
                    return core::ptr::read_volatile(resp_ptr);
                }
            }
        }
        // Timeout — controller may not be fully initialized
        0
    }
}

/// Discover the output amplifier and pin widgets.
fn discover_output_path() {
    // Walk widget tree starting from node 0
    // Find first Pin Complex with Output capability
    for node in 0u8..16 {
        let cap_raw = send_verb(verb(unsafe { CODEC_ADDR }, node, VERB_GET_PARAMETER, PARAM_AUDIO_WIDGET_CAP as u8));
        if cap_raw == 0xFFFFFFFF { continue; }
        let widget_type = (cap_raw >> 20) & 0xF;

        if widget_type == WIDGET_PIN_COMPLEX {
            // Check if this pin supports output
            let pin_cap = send_verb(verb(unsafe { CODEC_ADDR }, node, VERB_GET_PARAMETER, PARAM_AUDIO_WIDGET_CAP as u8));
            if pin_cap & (1 << 4) != 0 { // Output capable
                unsafe { OUTPUT_PIN = node; }

                // Find the connected output amplifier
                let conn_list_len = send_verb(verb(unsafe { CODEC_ADDR }, node, VERB_GET_PARAMETER, PARAM_CONNECTION_LIST_LEN as u8)) & 0x7F;
                if conn_list_len > 0 {
                    let conn = send_verb(verb(unsafe { CODEC_ADDR }, node, VERB_GET_CONNECTION_LIST, 0));
                    let amp_node = (conn & 0xFFFF) as u8;
                    if amp_node > 0 && amp_node < 16 {
                        let amp_cap = send_verb(verb(unsafe { CODEC_ADDR }, amp_node, VERB_GET_PARAMETER, PARAM_AUDIO_WIDGET_CAP as u8));
                        let amp_type = (amp_cap >> 20) & 0xF;
                        if amp_type == WIDGET_AUDIO_OUTPUT || amp_type == WIDGET_AUDIO_MIXER {
                            unsafe { OUTPUT_NODE = amp_node; }
                            // Select this connection
                            send_verb(verb(unsafe { CODEC_ADDR }, node, VERB_SET_CONNECTION_SELECT, amp_node));
                        }
                    }
                }
                break;
            }
        }
    }
}

// ── MMIO access ─────────────────────────────────────────────────────

fn hda_mmio() -> u64 { unsafe { MMIO_BASE } }

unsafe fn hda_read8(reg: u32) -> u8 {
    core::ptr::read_volatile((hda_mmio() + reg as u64) as *const u8)
}

unsafe fn hda_read16(reg: u32) -> u16 {
    core::ptr::read_volatile((hda_mmio() + reg as u64) as *const u16)
}

unsafe fn hda_read32(reg: u32) -> u32 {
    core::ptr::read_volatile((hda_mmio() + reg as u64) as *const u32)
}

unsafe fn hda_write8(reg: u32, val: u8) {
    core::ptr::write_volatile((hda_mmio() + reg as u64) as *mut u8, val);
}

unsafe fn hda_write16(reg: u32, val: u16) {
    core::ptr::write_volatile((hda_mmio() + reg as u64) as *mut u16, val);
}

unsafe fn hda_write32(reg: u32, val: u32) {
    core::ptr::write_volatile((hda_mmio() + reg as u64) as *mut u32, val);
}
