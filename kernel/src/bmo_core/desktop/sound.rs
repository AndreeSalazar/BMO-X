//! Sound — PC speaker via PIT channel 2 + port 0x61.

#![allow(dead_code)]

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

/// Play a tone at `freq_hz` for `duration_ms` milliseconds.
/// If `freq_hz == 0`, just silence the speaker.
pub fn beep(freq_hz: u32, duration_ms: u32) {
    unsafe {
        if freq_hz == 0 {
            let p = inb(0x61);
            outb(0x61, p & 0xFC);
            return;
        }
        let div = (1_193_180u32 / freq_hz) as u16;
        outb(0x43, 0xB6);
        outb(0x42, (div & 0xFF) as u8);
        outb(0x42, ((div >> 8) & 0xFF) as u8);
        let p = inb(0x61);
        outb(0x61, p | 0x03);

        let cycles = (duration_ms as u64) * 3_700_000;
        let start = crate::cpu::rdtsc();
        while (crate::cpu::rdtsc() - start) < cycles {
            core::hint::spin_loop();
        }

        let p2 = inb(0x61);
        outb(0x61, p2 & 0xFC);
    }
}
