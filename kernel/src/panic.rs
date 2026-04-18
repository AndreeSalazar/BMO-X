//! Panic handler for #![no_std].

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Write PANIC to VGA last row
    let buf = 0xB8000 as *mut u16;
    let msg = b"KERNEL PANIC";
    let row_offset = 24 * 80; // last row
    for (i, &byte) in msg.iter().enumerate() {
        unsafe { buf.add(row_offset + i).write_volatile(0x4F00 | byte as u16); } // white on red
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
