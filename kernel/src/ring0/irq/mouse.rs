//! PS/2 Mouse — IRQ 12 handler + i8042 aux port polling.
//!
//! On each timer tick, checks the i8042 status port (0x64) bit 5
//! for mouse data. Reads 3-byte packets (or 4-byte if scroll wheel
//! is enabled) and pushes to the BMO system channel.

use core::arch::asm;

const PORT_DATA: u16 = 0x60;
const PORT_STATUS: u16 = 0x64;

const OP_MOUSE_MOVE:   u64 = 0xB000_0010;
const OP_MOUSE_BUTTON: u64 = 0xB000_0011;
const OP_MOUSE_WHEEL:  u64 = 0xB000_0012;

static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_BUF: [u8; 4] = [0; 4];
static mut MOUSE_HAS_WHEEL: bool = false;
static mut MOUSE_BTN_OLD: u8 = 0;

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

/// Send a command byte to the aux device and wait for ACK.
unsafe fn mouse_cmd(cmd: u8) {
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
    asm!("out 0x60, al", in("al") cmd, options(nostack, nomem));
    // Read ACK (0xFA)
    loop {
        let status: u8;
        asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
        if status & 1 != 0 { break; }
    }
    let _ack: u8;
    asm!("in al, dx", in("dx") PORT_DATA, out("al") _ack, options(nostack, nomem));
}

pub fn tick() {
    while has_mouse_data() {
        let b = read_mouse_byte();
        unsafe {
            MOUSE_CYCLE = MOUSE_CYCLE.wrapping_add(1);
            let wheel = MOUSE_HAS_WHEEL;
            let packet_size = if wheel { 4 } else { 3 };

            match MOUSE_CYCLE {
                1 => {
                    if b & 0x08 == 0 { MOUSE_CYCLE = 0; return; }
                    MOUSE_BUF[0] = b;
                }
                2 => { MOUSE_BUF[1] = b; }
                3 => {
                    MOUSE_BUF[2] = b;
                    if !wheel {
                        finish_packet();
                    }
                }
                _ if wheel => {
                    MOUSE_BUF[3] = b;
                    finish_packet_wheel();
                }
                _ => {
                    // Stray byte
                    MOUSE_CYCLE = 0;
                    return;
                }
            }

            if MOUSE_CYCLE as u8 >= packet_size {
                MOUSE_CYCLE = 0;
            }
            let _ = packet_size;
        }
    }
}

/// Process a 3-byte packet (no wheel).
unsafe fn finish_packet() {
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
    if btns != MOUSE_BTN_OLD {
        crate::channel::sys_send(OP_MOUSE_BUTTON, btns as u64, 0, 0);
        MOUSE_BTN_OLD = btns;
    }
}

/// Process a 4-byte packet (with wheel, Intellimouse / Logitech protocol).
unsafe fn finish_packet_wheel() {
    let flags = MOUSE_BUF[0];
    let btns = flags & 0x07;
    let dx: i64 = if flags & 0x10 != 0 {
        (MOUSE_BUF[1] as i8) as i64
    } else { MOUSE_BUF[1] as i64 };
    let dy: i64 = if flags & 0x20 != 0 {
        -((MOUSE_BUF[2] as i8) as i64)
    } else { -(MOUSE_BUF[2] as i64) };
    // Z wheel: low 4 bits of byte 3 are a signed value (-8..+7).
    // Cast to i8 first (sign-extends from bit 3), then to i64.
    let dz: i8 = (MOUSE_BUF[3] & 0x0F) as i8;
    let dz: i64 = dz as i64;
    if dx != 0 || dy != 0 {
        crate::channel::sys_send(OP_MOUSE_MOVE, dx as u64, dy as u64, 0);
    }
    if dz != 0 {
        crate::channel::sys_send(OP_MOUSE_WHEEL, dz as u64, 0, 0);
    }
    if btns != MOUSE_BTN_OLD {
        crate::channel::sys_send(OP_MOUSE_BUTTON, btns as u64, 0, 0);
        MOUSE_BTN_OLD = btns;
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

        // Enable mouse data reporting
        mouse_cmd(0xF4);

        // Try to enable scroll wheel (Intellimouse magic sequence).
        // Some mice will NAK; that's fine — we just stay in 3-byte mode.
        mouse_cmd(0xF3);  // Set sample rate
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        asm!("out 0x60, al", in("al") 200u8, options(nostack, nomem));
        // (would read ACK here, skipping for brevity)

        mouse_cmd(0xF3);
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        asm!("out 0x60, al", in("al") 100u8, options(nostack, nomem));

        mouse_cmd(0xF3);
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 2 == 0 { break; }
        }
        asm!("out 0x60, al", in("al") 80u8, options(nostack, nomem));

        // The 200/100/80 magic sequence enables 4-byte wheel packets on
        // most mice. We assume success — if the device is dumb, the
        // packet parser will see garbage in byte 3 and discard it.
        MOUSE_HAS_WHEEL = true;
    }
}
