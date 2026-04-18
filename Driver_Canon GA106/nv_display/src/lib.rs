//! # nv_display — Display Controller for GA106 (RTX 3060 12G)
//!
//! Display heads, SOR outputs, EDID reading, and modeset.
//! Maps to nvlddmkm.sys sections: _DDTEXT (Display Driver, non-paged),
//! PAGE_DD (Display Driver, paged).
//!
//! SigDead-BIB: display subsystem strings include "Display Underflow",
//! NV_ERR_DUAL_LINK_IN_USE, NV_ERR_FREQ_NOT_SUPPORTED.
//!
//! `#![no_std]` compatible.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_regs::pdisplay;
use nv_hal::{MmioRegion, Platform};

// ── Output Types ────────────────────────────────────────────────────────────

/// Connector type detected on a SOR output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    /// DisplayPort connector.
    DisplayPort,
    /// HDMI connector.
    Hdmi,
    /// DVI connector.
    Dvi,
    /// No connector detected.
    None,
}

/// Link training / connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// Output connected and link trained.
    Connected,
    /// No display connected.
    Disconnected,
    /// Link training in progress.
    Training,
}

// ── Display Head ────────────────────────────────────────────────────────────

/// A single display head (output pipeline).
/// GA106 supports up to 4 heads (`pdisplay::HEAD_COUNT`).
#[derive(Debug, Clone, Copy)]
pub struct DisplayHead {
    /// Head index (0..HEAD_COUNT).
    pub index: u32,
    /// Whether this head is currently enabled.
    pub enabled: bool,
    /// Horizontal resolution in pixels.
    pub width: u32,
    /// Vertical resolution in pixels.
    pub height: u32,
    /// Framebuffer pitch in bytes (typically width * bytes_per_pixel).
    pub pitch: u32,
    /// Pixel format (e.g. `PIXEL_FORMAT_BGRA8888`).
    pub pixel_format: u32,
    /// Physical address of the framebuffer.
    pub fb_phys: u64,
}

impl DisplayHead {
    /// Create a disabled head with default values.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            enabled: false,
            width: 0,
            height: 0,
            pitch: 0,
            pixel_format: 0,
            fb_phys: 0,
        }
    }
}

// ── SOR Output ──────────────────────────────────────────────────────────────

/// Serial Output Resource — drives DisplayPort / HDMI / DVI.
/// GA106 has `pdisplay::SOR_COUNT` SOR outputs.
#[derive(Debug, Clone, Copy)]
pub struct SorOutput {
    /// SOR index (0..SOR_COUNT).
    pub index: u32,
    /// Detected connector type.
    pub output_type: OutputType,
    /// Current link status.
    pub link_status: LinkStatus,
}

impl SorOutput {
    /// Create an unconnected SOR output.
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            output_type: OutputType::None,
            link_status: LinkStatus::Disconnected,
        }
    }
}

// ── Display Configuration ───────────────────────────────────────────────────

/// Complete display configuration: heads + SOR outputs.
pub struct DisplayConfig {
    /// Display heads (up to 4).
    pub heads: [DisplayHead; pdisplay::HEAD_COUNT as usize],
    /// SOR outputs (up to 4).
    pub sors: [SorOutput; pdisplay::SOR_COUNT as usize],
}

impl DisplayConfig {
    /// Create default configuration with all heads disabled and SORs disconnected.
    pub const fn new() -> Self {
        Self {
            heads: [
                DisplayHead::new(0),
                DisplayHead::new(1),
                DisplayHead::new(2),
                DisplayHead::new(3),
            ],
            sors: [
                SorOutput::new(0),
                SorOutput::new(1),
                SorOutput::new(2),
                SorOutput::new(3),
            ],
        }
    }
}

// ── Display Initialization ──────────────────────────────────────────────────

/// Initialize the display engine.
///
/// Clears pending display interrupts and disables all heads.
/// Should be called after `pmc::ENABLE_PDISPLAY` is set.
pub fn display_init(bar0: &MmioRegion) -> DisplayConfig {
    // Clear any pending display interrupts
    let pending = bar0.read32(pdisplay::INTR_0);
    bar0.write32(pdisplay::INTR_0, pending);

    // Disable display interrupt generation
    bar0.write32(pdisplay::INTR_EN_0, 0);

    // Disable all heads
    for h in 0..pdisplay::HEAD_COUNT {
        bar0.write32(pdisplay::HEAD_SET_CONTROL(h), 0);
    }

    DisplayConfig::new()
}

// ── Head Enable / Disable ───────────────────────────────────────────────────

/// Enable a display head with the given configuration.
///
/// Writes framebuffer offset, size, pitch, pixel format, and sets the
/// enable bit (bit 31) in HEAD_SET_CONTROL.
pub fn head_enable(bar0: &MmioRegion, head: &DisplayHead) {
    let h = head.index;

    // Framebuffer physical address (low 32 bits used as offset)
    bar0.write32(pdisplay::HEAD_SET_OFFSET(h), head.fb_phys as u32);

    // Size: width in upper 16 bits, height in lower 16 bits
    let size = (head.width << 16) | (head.height & 0xFFFF);
    bar0.write32(pdisplay::HEAD_SET_SIZE(h), size);

    // Storage format / pixel format
    bar0.write32(pdisplay::HEAD_SET_STORAGE(h), head.pixel_format);

    // Pitch in bytes
    bar0.write32(pdisplay::HEAD_SET_PITCH(h), head.pitch);

    // Control: bit 31 = enable, lower bits = pixel format
    let control = (1u32 << 31) | head.pixel_format;
    bar0.write32(pdisplay::HEAD_SET_CONTROL(h), control);
}

/// Disable a display head by clearing its control register.
pub fn head_disable(bar0: &MmioRegion, head_index: u32) {
    bar0.write32(pdisplay::HEAD_SET_CONTROL(head_index), 0);
}

// ── SOR Detection ───────────────────────────────────────────────────────────

/// Detect connector type and link status on a SOR output.
///
/// Reads the SOR status register to determine what is connected.
pub fn sor_detect(bar0: &MmioRegion, sor_index: u32) -> SorOutput {
    let status = bar0.read32(pdisplay::SOR_BASE(sor_index));

    // Status register bit layout (from nouveau / envytools):
    //   bits [1:0] — connector type: 0=none, 1=DP, 2=HDMI, 3=DVI
    //   bit  2     — link status: 1=connected
    //   bit  3     — link training in progress
    let output_type = match status & 0x03 {
        1 => OutputType::DisplayPort,
        2 => OutputType::Hdmi,
        3 => OutputType::Dvi,
        _ => OutputType::None,
    };

    let link_status = if status & 0x08 != 0 {
        LinkStatus::Training
    } else if status & 0x04 != 0 {
        LinkStatus::Connected
    } else {
        LinkStatus::Disconnected
    };

    SorOutput {
        index: sor_index,
        output_type,
        link_status,
    }
}

// ── I2C / EDID ──────────────────────────────────────────────────────────────

/// Perform a single I2C byte read on the given port.
///
/// Used primarily for EDID reading. The sequence:
/// 1. Write slave address + register to I2C_DATA
/// 2. Trigger the transaction via I2C_CTRL
/// 3. Poll I2C_CTRL for completion (bit 31 clears)
/// 4. Read result from I2C_DATA
pub fn i2c_read_byte(
    bar0: &MmioRegion,
    port: u32,
    addr: u8,
    reg: u8,
    platform: &dyn Platform,
) -> NvResult<u8> {
    // Write slave address (7-bit, shifted) + register index
    let cmd = ((addr as u32) << 8) | (reg as u32);
    bar0.write32(pdisplay::I2C_DATA(port), cmd);

    // Trigger I2C read: bit 31 = start, bit 0 = read direction
    bar0.write32(pdisplay::I2C_CTRL(port), (1u32 << 31) | 0x01);

    // Poll for completion: bit 31 clears when transaction finishes
    let mut remaining: u32 = 1000;
    loop {
        let ctrl = bar0.read32(pdisplay::I2C_CTRL(port));
        if ctrl & (1u32 << 31) == 0 {
            break;
        }
        if remaining == 0 {
            return Err(NvError::Timeout);
        }
        platform.stall_us(1);
        remaining -= 1;
    }

    // Check for I2C error (bit 30 = NACK / error)
    let ctrl = bar0.read32(pdisplay::I2C_CTRL(port));
    if ctrl & (1u32 << 30) != 0 {
        return Err(NvError::I2cError);
    }

    // Read result byte from I2C_DATA
    let data = bar0.read32(pdisplay::I2C_DATA(port));
    Ok((data & 0xFF) as u8)
}

/// Read the 8-byte EDID header from a display connected on `port`.
///
/// The EDID header is always: `00 FF FF FF FF FF FF 00`.
/// DDC address for EDID is 0x50.
pub fn read_edid_header(
    bar0: &MmioRegion,
    port: u32,
    platform: &dyn Platform,
) -> NvResult<[u8; 8]> {
    const EDID_ADDR: u8 = 0x50;
    let mut header = [0u8; 8];

    for i in 0..8u8 {
        header[i as usize] = i2c_read_byte(bar0, port, EDID_ADDR, i, platform)?;
    }

    Ok(header)
}

// ── Mode Setting ────────────────────────────────────────────────────────────

/// Configure and enable a display head with BGRA8888 pixel format.
///
/// Creates a `DisplayHead` with `pitch = width * 4`, enables it, and returns
/// the configuration.
pub fn set_display_mode(
    bar0: &MmioRegion,
    head_index: u32,
    width: u32,
    height: u32,
    fb_phys: u64,
    _platform: &dyn Platform,
) -> NvResult<DisplayHead> {
    if head_index >= pdisplay::HEAD_COUNT {
        return Err(NvError::InvalidIndex);
    }

    let head = DisplayHead {
        index: head_index,
        enabled: true,
        width,
        height,
        pitch: width * 4,
        pixel_format: pdisplay::PIXEL_FORMAT_BGRA8888,
        fb_phys,
    };

    head_enable(bar0, &head);

    Ok(head)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_head_defaults() {
        let head = DisplayHead::new(2);
        assert_eq!(head.index, 2);
        assert!(!head.enabled);
        assert_eq!(head.width, 0);
        assert_eq!(head.height, 0);
        assert_eq!(head.pitch, 0);
        assert_eq!(head.pixel_format, 0);
        assert_eq!(head.fb_phys, 0);
    }

    #[test]
    fn sor_output_defaults() {
        let sor = SorOutput::new(1);
        assert_eq!(sor.index, 1);
        assert_eq!(sor.output_type, OutputType::None);
        assert_eq!(sor.link_status, LinkStatus::Disconnected);
    }

    #[test]
    fn display_config_defaults() {
        let cfg = DisplayConfig::new();
        for (i, head) in cfg.heads.iter().enumerate() {
            assert_eq!(head.index, i as u32);
            assert!(!head.enabled);
        }
        for (i, sor) in cfg.sors.iter().enumerate() {
            assert_eq!(sor.index, i as u32);
            assert_eq!(sor.output_type, OutputType::None);
        }
    }

    #[test]
    fn output_type_variants() {
        assert_ne!(OutputType::DisplayPort, OutputType::Hdmi);
        assert_ne!(OutputType::Hdmi, OutputType::Dvi);
        assert_ne!(OutputType::Dvi, OutputType::None);
    }

    #[test]
    fn link_status_variants() {
        assert_ne!(LinkStatus::Connected, LinkStatus::Disconnected);
        assert_ne!(LinkStatus::Disconnected, LinkStatus::Training);
        assert_ne!(LinkStatus::Training, LinkStatus::Connected);
    }
}
