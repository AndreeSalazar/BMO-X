//! PIC 8259A — Programmable Interrupt Controller.
//! Remaps IRQ 0-15 to interrupt vectors 32-47.
//! Ring 0, direct port I/O.

use core::arch::asm;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;
const PIC_EOI: u8 = 0x20;

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags)); }
}

#[inline]
fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { asm!("in al, dx", out("al") val, in("dx") port, options(nostack, preserves_flags)); }
    val
}

#[inline]
fn io_wait() {
    outb(0x80, 0);
}

/// Initialize both PICs. Remap IRQ 0-7 to vectors 32-39, IRQ 8-15 to 40-47.
pub fn init_pic() {
    let mask1 = inb(PIC1_DATA);
    let mask2 = inb(PIC2_DATA);

    // ICW1: start init sequence
    outb(PIC1_CMD, ICW1_INIT); io_wait();
    outb(PIC2_CMD, ICW1_INIT); io_wait();

    // ICW2: vector offsets
    outb(PIC1_DATA, 32); io_wait();  // IRQ 0-7  → vectors 32-39
    outb(PIC2_DATA, 40); io_wait();  // IRQ 8-15 → vectors 40-47

    // ICW3: cascading
    outb(PIC1_DATA, 4); io_wait();   // IRQ2 has slave
    outb(PIC2_DATA, 2); io_wait();   // Slave cascade identity

    // ICW4: 8086 mode
    outb(PIC1_DATA, ICW4_8086); io_wait();
    outb(PIC2_DATA, ICW4_8086); io_wait();

    // Restore masks (all masked initially)
    outb(PIC1_DATA, mask1);
    outb(PIC2_DATA, mask2);
}

/// Enable specific IRQ line (0-15).
pub fn enable_irq(irq: u8) {
    if irq < 8 {
        let mask = inb(PIC1_DATA) & !(1 << irq);
        outb(PIC1_DATA, mask);
    } else {
        let mask = inb(PIC2_DATA) & !(1 << (irq - 8));
        outb(PIC2_DATA, mask);
    }
}

/// Mask (disable) specific IRQ line.
pub fn disable_irq(irq: u8) {
    if irq < 8 {
        let mask = inb(PIC1_DATA) | (1 << irq);
        outb(PIC1_DATA, mask);
    } else {
        let mask = inb(PIC2_DATA) | (1 << (irq - 8));
        outb(PIC2_DATA, mask);
    }
}

/// Mask all IRQs except the ones we need.
pub fn set_mask_keyboard_timer() {
    // Master PIC: enable IRQ0 (timer) and IRQ1 (keyboard), mask rest
    outb(PIC1_DATA, 0b1111_1100);  // bits 0,1 = enabled
    // Slave PIC: mask all
    outb(PIC2_DATA, 0xFF);
}

/// Send End-Of-Interrupt for IRQ (0-15).
pub fn send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_CMD, PIC_EOI);
    }
    outb(PIC1_CMD, PIC_EOI);
}
