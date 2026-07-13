//! Layer 0 ??? `uefi_enter`
//!
//! Responsibilities (only these, nothing else):
//! 1. Receive the UEFI handoff (`ImageHandle`, `SystemTable*`).
//! 2. Bring up COM1 serial for the rest of the chain.
//! 3. Build the `BootContext` skeleton, stamp `MAGIC` and `version`.
//! 4. Jump to layer 1 (`uefi_efi_getmem`).
//!
//! This layer MUST NOT touch memory map, GOP, ESP, or boot services
//! directly. That's the next layer's job.

#![allow(dead_code)]

use boot_context::BootContext;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

extern "C" {
    /// Layer 1 entry point. Resolved at link time within the same EFI
    /// binary.
    fn l1_entry(ctx: *mut BootContext, ih: EfiHandle, st: *mut core::ffi::c_void) -> !;
}

#[no_mangle]
pub extern "efiapi" fn layer0_efi_main(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
) -> EfiStatus {
    crate::serial::init();
    crate::serial::puts("\n[L0 uefi_enter] BMO Ultra Kernel\n");

    let mut ctx = BootContext::new();
    ctx.magic = boot_context::MAGIC;
    ctx.version = 2;

    crate::serial::puts("[L0] magic=");
    crate::serial::hex(ctx.magic);
    crate::serial::puts(" version=");
    crate::serial::dec(ctx.version as usize);
    crate::serial::puts("\n");

    crate::serial::puts("[L0] jump -> layer1_getmem\n");

    unsafe { l1_entry(&mut ctx, image_handle, system_table) }
}
