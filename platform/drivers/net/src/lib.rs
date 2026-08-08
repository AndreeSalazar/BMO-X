//! BMO Network Stack -- NIC driver (Intel e1000) + packet I/O.
//!
//! ## Architecture
//!   - Ring 0: NIC detection (PCIe class=0x02), MMIO setup, TX/RX rings
//!   - Ring 3: smoltcp TCP/IP stack, sends raw frames via syscall SYS_NET_SEND
//!   - Kernel provides `sys_net_poll()` to check for received frames
//!
//! ## Usage (module side)
//! ```ignore
//! let mut nic = Nic::probe(mmio_bar, &backend, &mac_addr)?;
//! nic.send(packet_data, packet_len); // raw ethernet frame
//! if let Some(data) = nic.receive(buf) { /* process frame */ }
//! ```

#![no_std]

use bitflags::bitflags;
use core::ptr;

/// Backend for DMA allocation (same pattern as NVMe).
pub trait NetBackend {
    fn alloc_dma(&self, pages: usize) -> Option<u64>;
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    fn wait_ms(&self, ms: u64);
}

/// MAC address (6 bytes).
pub type MacAddr = [u8; 6];

// -- e1000 Registers (BAR0 MMIO) ----------------------------------------

struct Regs(*mut u32);

impl Regs {
    #[inline] unsafe fn read(&self, off: usize) -> u32 {
        (self.0.add(off / 4)).read_volatile()
    }
    #[inline] unsafe fn write(&self, off: usize, val: u32) {
        (self.0.add(off / 4)).write_volatile(val);
    }
}

const CTRL:   usize = 0x0000;
const STATUS: usize = 0x0008;
const EERD:   usize = 0x0014;
const IMS:    usize = 0x00D0;
const ICR:    usize = 0x00C0;
const RCTL:   usize = 0x0100;
const TCTL:   usize = 0x0400;

const RDBAL:  usize = 0x2800;
const RDBAH:  usize = 0x2804;
const RDLEN:  usize = 0x2808;
const RDH:    usize = 0x2810;
const RDT:    usize = 0x2818;

const TDBAL:  usize = 0x3800;
const TDBAH:  usize = 0x3804;
const TDLEN:  usize = 0x3808;
const TDH:    usize = 0x3810;
const TDT:    usize = 0x3818;

const RAL0:   usize = 0x5400;
const RAH0:   usize = 0x5404;

// Control flags
const CTRL_FD: u32  = 1 << 0;  // Full Duplex
const CTRL_SLU: u32 = 1 << 6;  // Set Link Up
const RCTL_EN: u32  = 1 << 1;
const RCTL_BSIZE_2048: u32 = 0 << 16;
const RCTL_BSIZE_4096: u32 = (1 << 25) | (3 << 16);
const TCTL_EN: u32  = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;

// Descriptor flags
const TX_CMD_EOP: u8 = 1 << 0;
const TX_CMD_IFCS: u8 = 1 << 1;
const TX_CMD_RS: u8 = 1 << 3;
const TX_STAT_DD: u8 = 1 << 0;
const RX_STAT_DD: u8 = 1 << 0;

const NUM_TX_DESC: usize = 32;
const NUM_RX_DESC: usize = 128;
const RX_BUF_SIZE: usize = 2048;

#[repr(C)]
struct TxDesc {
    addr_lo: u32,
    addr_hi: u32,
    length: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

#[repr(C)]
struct RxDesc {
    addr_lo: u32,
    addr_hi: u32,
    length: u16,
    cso: u8,
    cmd: u8,       // actually packet checksum for RX
    status: u8,
    errors: u8,
    special: u16,
}

/// NIC driver state.
pub struct Nic {
    mmio: Regs,
    /// MAC address.
    pub mac: MacAddr,
    tx_desc_phys: u64,
    tx_descs: *mut TxDesc,
    tx_bufs_phys: [u64; NUM_TX_DESC],
    tx_bufs: [*mut u8; NUM_TX_DESC],
    tx_tail: usize,
    rx_desc_phys: u64,
    rx_descs: *mut RxDesc,
    rx_bufs_phys: [u64; NUM_RX_DESC],
    rx_bufs: [*mut u8; NUM_RX_DESC],
    rx_tail: usize,
}

impl Nic {
    /// Probe and initialize e1000 at MMIO BAR address.
    pub fn probe(mmio_bar: u64, backend: &impl NetBackend) -> Option<Self> {
        let mmio = Regs(mmio_bar as *mut u32);

        // Reset + link up
        unsafe {
            let ctrl = mmio.read(CTRL);
            mmio.write(CTRL, ctrl | CTRL_SLU);
        }
        backend.wait_ms(10);

        // Read MAC from EEPROM
        let mut mac: MacAddr = [0; 6];
        for i in 0..3 {
            unsafe {
                mmio.write(EERD, (1 | ((i as u32) << 8)));
                backend.wait_ms(1);
                let val = mmio.read(EERD);
                if val & (1 << 4) != 0 {
                    mac[i * 2] = (val >> 16) as u8;
                    mac[i * 2 + 1] = (val >> 24) as u8;
                }
            }
        }

        if mac[0] == 0 && mac[1] == 0 && mac[2] == 0 {
            return None; // No valid MAC
        }

        // Allocate TX descriptors (1 page)
        let tx_desc_phys = backend.alloc_dma(1)?;
        let tx_descs = backend.phys_to_virt(tx_desc_phys) as *mut TxDesc;
        unsafe { ptr::write_bytes(tx_descs as *mut u8, 0, 4096); }

        // Allocate TX buffers
        let mut tx_bufs = [ptr::null_mut::<u8>(); NUM_TX_DESC];
        let mut tx_bufs_phys = [0u64; NUM_TX_DESC];
        for i in 0..NUM_TX_DESC {
            let phys = backend.alloc_dma(1)?;
            let virt = backend.phys_to_virt(phys);
            unsafe {
                ptr::write_bytes(virt, 0, 4096);
                let desc = &mut *tx_descs.add(i);
                desc.addr_lo = phys as u32;
                desc.addr_hi = (phys >> 32) as u32;
                desc.cmd = 0;
                desc.status = TX_STAT_DD; // mark ready
            }
            tx_bufs[i] = virt;
            tx_bufs_phys[i] = phys;
        }

        // Allocate RX descriptors (2 pages)
        let rx_desc_phys = backend.alloc_dma(2)?;
        let rx_descs = backend.phys_to_virt(rx_desc_phys) as *mut RxDesc;
        unsafe { ptr::write_bytes(rx_descs as *mut u8, 0, 8192); }

        // Allocate RX buffers
        let mut rx_bufs = [ptr::null_mut::<u8>(); NUM_RX_DESC];
        let mut rx_bufs_phys = [0u64; NUM_RX_DESC];
        for i in 0..NUM_RX_DESC {
            let phys = backend.alloc_dma(1)?;
            let virt = backend.phys_to_virt(phys);
            unsafe {
                ptr::write_bytes(virt, 0, 4096);
                let desc = &mut *rx_descs.add(i);
                desc.addr_lo = phys as u32;
                desc.addr_hi = (phys >> 32) as u32;
            }
            rx_bufs[i] = virt;
            rx_bufs_phys[i] = phys;
        }

        // Program RX
        unsafe {
            mmio.write(RDBAL, rx_desc_phys as u32);
            mmio.write(RDBAH, (rx_desc_phys >> 32) as u32);
            mmio.write(RDLEN, (NUM_RX_DESC * 16) as u32);
            mmio.write(RDH, 0);
            mmio.write(RDT, (NUM_RX_DESC - 1) as u32);
            mmio.write(RCTL, RCTL_EN | RCTL_BSIZE_4096);

            // Program TX
            mmio.write(TDBAL, tx_desc_phys as u32);
            mmio.write(TDBAH, (tx_desc_phys >> 32) as u32);
            mmio.write(TDLEN, (NUM_TX_DESC * 16) as u32);
            mmio.write(TDH, 0);
            mmio.write(TDT, 0);
            mmio.write(TCTL, TCTL_EN | TCTL_PSP);

            // Set MAC
            let mac_lo = (mac[0] as u32) | ((mac[1] as u32) << 8) | ((mac[2] as u32) << 16) | ((mac[3] as u32) << 24);
            let mac_hi = (mac[4] as u32) | ((mac[5] as u32) << 8) | 0x8000_0000;
            mmio.write(RAL0, mac_lo);
            mmio.write(RAH0, mac_hi);
        }

        Some(Self {
            mmio,
            mac,
            tx_desc_phys,
            tx_descs,
            tx_bufs,
            tx_bufs_phys,
            tx_tail: 0,
            rx_desc_phys,
            rx_descs,
            rx_bufs,
            rx_bufs_phys,
            rx_tail: NUM_RX_DESC - 1,
        })
    }

    /// Send a raw ethernet frame. Returns false if TX ring full.
    pub fn send(&mut self, data: &[u8]) -> bool {
        if data.len() > 4096 { return false; }
        let idx = self.tx_tail;
        unsafe {
            let desc = &mut *self.tx_descs.add(idx);
            if desc.status & TX_STAT_DD == 0 { return false; } // still in flight

            let buf = self.tx_bufs[idx];
            ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len());
            desc.length = data.len() as u16;
            desc.cmd = TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS;
            desc.status = 0;

            self.tx_tail = (idx + 1) % NUM_TX_DESC;
            self.mmio.write(TDT, self.tx_tail as u32);
        }
        true
    }

    /// Poll for a received frame. Returns (packet_data, length).
    pub fn receive(&mut self, buf: &mut [u8]) -> Option<usize> {
        let idx = (self.rx_tail + 1) % NUM_RX_DESC;
        unsafe {
            let desc = &mut *self.rx_descs.add(idx);
            if desc.status & RX_STAT_DD == 0 { return None; }

            let len = desc.length as usize;
            if len > 0 && len <= buf.len() {
                let src = self.rx_bufs[idx];
                ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), len);
            }

            // Re-arm descriptor
            desc.status = 0;
            self.rx_tail = idx;
            self.mmio.write(RDT, self.rx_tail as u32);

            Some(len.min(buf.len()))
        }
    }

    /// Check if a link is detected.
    pub fn link_up(&self) -> bool {
        unsafe { self.mmio.read(STATUS) & 0x02 != 0 }
    }
}
