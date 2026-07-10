//! Intel High Definition Audio controller discovery.
//!
//! This probe is deliberately read-only: codec command transport, widget
//! routing, and DMA streams are required before analog headphones can be
//! called usable. Discovery must never take ownership from firmware or make
//! the mandatory boot path hang.

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

const CLASS_MULTIMEDIA: u8 = 0x04;
const SUBCLASS_HDA: u8 = 0x03;
const MAP_SIZE: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProbeState {
    NotProbed = 0,
    NotFound = 1,
    InvalidBar = 2,
    MapFailed = 3,
    Detected = 4,
    CodecPresent = 5,
}

static STATE: AtomicU8 = AtomicU8::new(ProbeState::NotProbed as u8);
static MMIO_BASE: AtomicU64 = AtomicU64::new(0);
static CODEC_MASK: AtomicU8 = AtomicU8::new(0);

fn set_state(state: ProbeState) -> ProbeState {
    STATE.store(state as u8, Ordering::Release);
    state
}

pub fn state() -> ProbeState {
    match STATE.load(Ordering::Acquire) {
        1 => ProbeState::NotFound,
        2 => ProbeState::InvalidBar,
        3 => ProbeState::MapFailed,
        4 => ProbeState::Detected,
        5 => ProbeState::CodecPresent,
        _ => ProbeState::NotProbed,
    }
}

pub fn codec_mask() -> u8 {
    CODEC_MASK.load(Ordering::Acquire)
}

pub fn mmio_base() -> u64 {
    MMIO_BASE.load(Ordering::Acquire)
}

/// Locate and safely inspect the HDA controller. This does not claim that a
/// usable headphone route exists and does not alter controller registers.
pub fn probe() -> ProbeState {
    let device = match crate::dev::pcie::find_by_class(CLASS_MULTIMEDIA, SUBCLASS_HDA) {
        Some(device) => device,
        None => return set_state(ProbeState::NotFound),
    };

    let bar_low = crate::dev::pcie::pci_read32(device.bus, device.device, device.function, 0x10);
    if bar_low == 0 || bar_low == u32::MAX || bar_low & 1 != 0 {
        return set_state(ProbeState::InvalidBar);
    }
    let physical = if bar_low & 0x06 == 0x04 {
        let high = crate::dev::pcie::pci_read32(device.bus, device.device, device.function, 0x14);
        ((high as u64) << 32) | ((bar_low & !0x0F) as u64)
    } else {
        (bar_low & !0x0F) as u64
    };
    if physical == 0 { return set_state(ProbeState::InvalidBar); }

    let map_phys = physical & !((MAP_SIZE as u64) - 1);
    let map_virt = crate::mm::vmm::HIGH_MEM_BASE + map_phys;
    if unsafe { crate::mm::vmm::map_kernel_mmio_huge(map_phys, map_virt, MAP_SIZE) }.is_err() {
        return set_state(ProbeState::MapFailed);
    }
    let mmio = map_virt + (physical - map_phys);

    // GCAP and version must not both read as an absent MMIO aperture.
    let gcap = unsafe { core::ptr::read_volatile(mmio as *const u16) };
    let version = unsafe { core::ptr::read_volatile((mmio + 0x02) as *const u16) };
    if (gcap == 0 && version == 0) || (gcap == u16::MAX && version == u16::MAX) {
        return set_state(ProbeState::InvalidBar);
    }

    let codecs = unsafe { core::ptr::read_volatile((mmio + 0x0E) as *const u16) } as u8;
    MMIO_BASE.store(mmio, Ordering::Release);
    CODEC_MASK.store(codecs, Ordering::Release);

    crate::dev::console::serial_write("[audio] HDA detected; PCM routing is not initialized\n");
    set_state(if codecs != 0 { ProbeState::CodecPresent } else { ProbeState::Detected })
}
