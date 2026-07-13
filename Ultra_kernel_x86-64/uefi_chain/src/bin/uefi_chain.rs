//! BMO Ultra Kernel ??? UEFI entry point.
//!
//! This is the firmware-facing symbol. The actual work is split across
//! 5 layers, each one a single function with `jmp` hand-off:
//!
//!   layer0_enter ??? layer1_getmem ??? layer2_getgop
//!                ??? layer3_load    ??? layer4_exit ??? (jmp to stage1_arch)

#![no_std]
#![no_main]

use core::panic::PanicInfo;
type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
) -> EfiStatus {
    // Keep the firmware-facing Microsoft x64 ABI here. Layer 0 performs
    // the explicit SysV-register handoff used by the remaining layers.
    uefi_chain::layer0_efi_main(image_handle, system_table)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("hlt"); } }
}
