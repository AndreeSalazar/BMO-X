//! # nv_kernel — Kernel-Level Driver Entry
//!
//! Top-level orchestration: full GPU init, teardown, interrupt dispatch,
//! and display setup. This is the OS-facing entry point, equivalent to
//! `DriverEntry` / `DriverUnload` in nvlddmkm.sys.
//!
//! SigDead-BIB found: the INIT section (39KB, discarded after DriverEntry)
//! performs the entire probe → init → enable sequence we replicate here.
//!
//! `#![no_std]` compatible.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_gpu::{Gpu, GpuState};
use nv_hal::Platform;
use nv_cmd::Channel;
use nv_display::DisplayConfig;
use nv_regs::pdisplay;

/// Maximum channels tracked by the driver.
const MAX_CHANNELS: usize = 4;

/// Maximum display heads supported (GA106).
const HEAD_COUNT: u32 = pdisplay::HEAD_COUNT;

// ── Driver State ────────────────────────────────────────────────────────────

/// Top-level driver state — owns GPU, channels, and display config.
pub struct DriverState {
    pub gpu: Gpu,
    pub channels: [Option<Channel>; MAX_CHANNELS],
    pub display: DisplayConfig,
    pub initialized: bool,
    pub irq_count: u64,
}

/// Snapshot of driver information for queries (all `Copy` types).
#[derive(Debug, Clone, Copy)]
pub struct DriverInfo {
    pub chip_id: u32,
    pub vram_size_mb: u64,
    pub gpu_time_ns: u64,
    pub irq_count: u64,
    pub gpu_state: GpuState,
}

impl Default for DriverInfo {
    fn default() -> Self {
        Self {
            chip_id: 0,
            vram_size_mb: 0,
            gpu_time_ns: 0,
            irq_count: 0,
            gpu_state: GpuState::Uninitialized,
        }
    }
}

// ── Init / Teardown ─────────────────────────────────────────────────────────

/// Initialize the full driver stack.
///
/// On bare metal with UEFI CSM, the VBIOS has already initialized the GPU
/// (proven by VBE 1920x1080 working). We must NOT reset engines that are
/// already running — this would kill the display.
///
/// Strategy: probe GPU state, attach to existing config, only enable
/// missing engines.
pub fn driver_init(platform: &dyn Platform) -> NvResult<DriverState> {
    // 1. Find the GPU on the PCI bus
    let pci = nv_hal::find_gpu(platform).ok_or(NvError::CardNotPresent)?;

    // 2. Set D0 power + bus mastering (harmless if already done)
    nv_hal::set_power_d0(platform, pci);
    nv_hal::enable_bus_master(platform, pci);

    // 3. Map BAR0 (register MMIO space)
    let bar0_phys = nv_hal::read_bar0(platform, pci);
    let bar0_size = nv_regs::BAR0_SIZE;
    let bar0_ptr = platform.map_mmio(bar0_phys, bar0_size);
    if bar0_ptr.is_null() {
        return Err(NvError::InvalidAddress);
    }
    let bar0 = unsafe { nv_hal::MmioRegion::new(bar0_ptr, bar0_size) };

    // 4. Read chip ID — verify GPU is responding
    let boot0 = bar0.read32(nv_regs::pmc::BOOT_0);
    if boot0 == 0 || boot0 == 0xFFFF_FFFF {
        return Err(NvError::CardNotPresent);
    }
    let chip = nv_gpu::ChipInfo::from_boot0(boot0);

    // 5. Read current engine state (VBIOS may have already enabled engines)
    let current_enable = bar0.read32(nv_regs::pmc::ENABLE);

    // 6. Detect VRAM (Ampere-aware: units of 16MB on GA106)
    let vram_size = nv_gpu::detect_vram(&bar0);

    // 7. Map BAR1 (VRAM aperture) — optional
    let bar1_phys = nv_hal::read_bar1(platform, pci);
    let bar1 = if bar1_phys != 0 {
        let ptr = platform.map_mmio(bar1_phys, nv_regs::BAR1_SIZE);
        if !ptr.is_null() {
            Some(unsafe { nv_hal::MmioRegion::new(ptr, nv_regs::BAR1_SIZE) })
        } else {
            None
        }
    } else {
        None
    };

    // 8. Determine GPU state based on what VBIOS left us
    let state = if current_enable != 0 {
        // VBIOS has engines running — attach to existing state
        GpuState::EnginesReset
    } else {
        GpuState::BarsMapping
    };

    let mut gpu = Gpu {
        bar0,
        bar1,
        pci,
        chip_id: chip.chip_id,
        vram_size,
        state,
    };

    // 9. If no engines are enabled, try to enable them
    //    Otherwise, VBIOS already did this — don't touch!
    if current_enable == 0 {
        // Cold init path — VBIOS didn't init (unlikely if VBE works)
        nv_gpu::enable_engines(&mut gpu)?;
    }

    // 10. Init display config (read current state, don't reconfigure)
    let display = nv_display::display_init(&gpu.bar0);

    // 11. Mark ready
    gpu.state = GpuState::Ready;

    Ok(DriverState {
        gpu,
        channels: [None, None, None, None],
        display,
        initialized: true,
        irq_count: 0,
    })
}

/// Tear down the driver — disable interrupts, heads, and mark uninitialized.
pub fn driver_teardown(state: &mut DriverState, _platform: &dyn Platform) {
    // 1. Disable interrupts
    nv_gpu::disable_interrupts(&state.gpu);

    // 2. Disable all display heads
    for head in 0..HEAD_COUNT {
        nv_display::head_disable(&state.gpu.bar0, head);
    }

    // 3. Mark driver as shut down
    state.gpu.state = GpuState::Uninitialized;
    state.initialized = false;
}

// ── Interrupt Handling ──────────────────────────────────────────────────────

/// Top-half IRQ handler — dispatch to nv_gpu and track count.
/// Returns pending interrupt mask (0 = not our IRQ).
pub fn driver_handle_irq(state: &mut DriverState) -> u32 {
    let pending = nv_gpu::handle_interrupt(&state.gpu);
    if pending != 0 {
        state.irq_count += 1;
    }
    pending
}

// ── Info Query ──────────────────────────────────────────────────────────────

/// Gather a point-in-time snapshot of driver state.
pub fn driver_info(state: &DriverState) -> DriverInfo {
    DriverInfo {
        chip_id: state.gpu.chip_id,
        vram_size_mb: state.gpu.vram_size / (1024 * 1024),
        gpu_time_ns: nv_gpu::gpu_time_ns(&state.gpu),
        irq_count: state.irq_count,
        gpu_state: state.gpu.state,
    }
}

// ── Display Setup ───────────────────────────────────────────────────────────

/// Configure a display head with the given mode and framebuffer.
pub fn driver_setup_display(
    state: &mut DriverState,
    head: u32,
    width: u32,
    height: u32,
    fb_phys: u64,
    platform: &dyn Platform,
) -> NvResult<()> {
    if head >= HEAD_COUNT {
        return Err(NvError::InvalidIndex);
    }

    let display_head = nv_display::set_display_mode(
        &state.gpu.bar0, head, width, height, fb_phys, platform,
    )?;
    state.display.heads[head as usize] = display_head;

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_info_default() {
        let info = DriverInfo::default();
        assert_eq!(info.chip_id, 0);
        assert_eq!(info.vram_size_mb, 0);
        assert_eq!(info.gpu_time_ns, 0);
        assert_eq!(info.irq_count, 0);
        assert_eq!(info.gpu_state, GpuState::Uninitialized);
    }

    #[test]
    fn head_count_matches_regs() {
        assert_eq!(HEAD_COUNT, pdisplay::HEAD_COUNT);
        assert_eq!(HEAD_COUNT, 4);
    }

    #[test]
    fn max_channels_nonzero() {
        assert!(MAX_CHANNELS > 0);
    }
}
