//! PS/2 Mouse — IRQ 12 handler + i8042 aux port polling.
//!
//! On each timer tick, checks the i8042 status port (0x64) bit 5
//! for mouse data. Reads 3-byte packets and pushes to the BMO
//! system channel.

use core::arch::asm;

const PORT_DATA: u16 = 0x60;
const PORT_STATUS: u16 = 0x64;

const OP_MOUSE_MOVE: u64   = 0xB000_0010;
const OP_MOUSE_BUTTON: u64 = 0xB000_0011;

static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_BUF: [u8; 3] = [0; 3];

fn has_mouse_data() -> bool {
    unsafe {
        let status: u8;
        asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
        (status & 0x20 != 0) && (status & 1 != 0)
    }
}

fn read_mouse_byte() -> u8 {
    unsafe {
        let data: u8;
        asm!("in al, dx", in("dx") PORT_DATA, out("al") data, options(nostack, nomem));
        data
    }
}

pub fn tick() {
    while has_mouse_data() {
        let b = read_mouse_byte();
        unsafe {
            MOUSE_CYCLE = MOUSE_CYCLE.wrapping_add(1);
            match MOUSE_CYCLE {
                1 => {
                    if b & 0x08 == 0 { MOUSE_CYCLE = 0; return; }
                    MOUSE_BUF[0] = b;
                }
                2 => { MOUSE_BUF[1] = b; }
                _ => {
                    MOUSE_BUF[2] = b; MOUSE_CYCLE = 0;
                    let flags = MOUSE_BUF[0];
                    let btns = flags & 0x07;
                    let dx: i64 = if flags & 0x10 != 0 {
                        (MOUSE_BUF[1] as i8) as i64
                    } else { MOUSE_BUF[1] as i64 };
                    let dy: i64 = if flags & 0x20 != 0 {
                        -((MOUSE_BUF[2] as i8) as i64)
                    } else { -(MOUSE_BUF[2] as i64) };
                    if dx != 0 || dy != 0 {
                        crate::channel::sys_send(OP_MOUSE_MOVE, dx as u64, dy as u64, 0);
                    }
                    crate::channel::sys_send(OP_MOUSE_BUTTON, btns as u64, 0, 0);
                }
            }
        }
    }
}

pub fn init() {
    unsafe {
        // Enable auxiliary port
        let _ack: u8;
        asm!("out 0x64, al", in("al") 0xA8u8, options(nostack, nomem));

        // Read configuration byte
        let mut cfg: u8;
        asm!("out 0x64, al", in("al") 0x20u8, options(nostack, nomem));
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 1 != 0 { break; }
        }
        asm!("in al, dx", in("dx") PORT_DATA, out("al") cfg, options(nostack, nomem));

        // Set bit 1 (enable aux interrupt) and bit 5 (enable aux clock)
        if cfg & 0x22 != 0x22 {
            cfg |= 0x22;
            asm!("out 0x64, al", in("al") 0x60u8, options(nostack, nomem));
            loop {
                let status: u8;
                asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
                if status & 2 == 0 { break; }
            }
            asm!("out 0x60, al", in("al") cfg, options(nostack, nomem));
        }

        // Enable mouse data reporting: write 0xF4 to aux device
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        asm!("out 0x64, al", in("al") 0xD4u8, options(nostack, nomem));
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        asm!("out 0x60, al", in("al") 0xF4u8, options(nostack, nomem));
        // Read ACK
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 1 != 0 { break; }
        }
        asm!("in al, dx", in("dx") PORT_DATA, out("al") _ack, options(nostack, nomem));
    }
}
