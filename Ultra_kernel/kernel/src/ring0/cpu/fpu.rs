//! FPU/SSE/AVX context save/restore — XSAVE/XRSTOR for Ryzen 5 5600X.

use core::arch::asm;

pub const XSAVE_AREA_SIZE: usize = 1024;

/// 64-byte aligned buffer to store FPU/SSE/AVX state.
/// XSAVE/XRSTOR instructions require the memory address to be 64-byte aligned.
#[repr(align(64))]
#[derive(Clone, Copy, Debug)]
pub struct FpuStateBuffer(pub [u8; XSAVE_AREA_SIZE]);

impl FpuStateBuffer {
    pub const fn zero() -> Self {
        Self([0; XSAVE_AREA_SIZE])
    }
}

static mut INITIAL_FPU_STATE: FpuStateBuffer = FpuStateBuffer::zero();

/// Save full x87+SSE+AVX state using XSAVE (up to 896+ bytes).
#[inline]
pub unsafe fn xsave(area: *mut u8) {
    let mask_lo: u32 = 0x7; // x87 | SSE | AVX
    let mask_hi: u32 = 0;
    asm!(
        "xsave [{}]",
        in(reg) area,
        in("eax") mask_lo,
        in("edx") mask_hi,
        options(nostack),
    );
}

/// Restore full x87+SSE+AVX state using XRSTOR.
#[inline]
pub unsafe fn xrstor(area: *const u8) {
    let mask_lo: u32 = 0x7;
    let mask_hi: u32 = 0;
    asm!(
        "xrstor [{}]",
        in(reg) area,
        in("eax") mask_lo,
        in("edx") mask_hi,
        options(nostack),
    );
}

/// Initialize FPU/SSE/AVX for the boot CPU, and capture the initial state.
pub fn init_fpu() {
    unsafe {
        // Initialize x87 FPU state
        asm!(
            "fninit",
            options(nostack),
        );

        // Set MXCSR to default: all exceptions masked, round to nearest
        let mxcsr: u32 = 0x1F80; // Default MXCSR value
        asm!(
            "ldmxcsr [{addr}]",
            addr = in(reg) &mxcsr as *const u32,
            options(nostack),
        );

        crate::ring0::dev::console::serial_write("[FPU] x87 FPU + MXCSR initialized\n");

        // Capture initial FPU state
        let ptr = core::ptr::addr_of_mut!(INITIAL_FPU_STATE) as *mut u8;
        xsave(ptr);
        crate::ring0::dev::console::serial_write("[FPU] captured initial clean FPU state\n");
    }
}

/// Copy the initial clean FPU state into a buffer.
pub fn copy_initial_state(dest: &mut FpuStateBuffer) {
    unsafe {
        dest.0.copy_from_slice(&INITIAL_FPU_STATE.0);
    }
}

/// Save FPU context for a task.
pub unsafe fn save_task_fpu(task: &mut crate::ring0::proc::task::Task) {
    let ptr = core::ptr::addr_of_mut!(task.fpu_save) as *mut u8;
    xsave(ptr);
}

/// Restore FPU context for a task.
pub unsafe fn restore_task_fpu(task: &crate::ring0::proc::task::Task) {
    let ptr = core::ptr::addr_of!(task.fpu_save) as *const u8;
    xrstor(ptr);
}
