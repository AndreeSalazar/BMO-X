#![allow(dead_code)]

//! RTL8168/RTL8111 (Realtek GbE) NIC driver for FastOS.
//! PCI Class 0x02 SubClass 0x00 (Ethernet Controller).
//!
//! MMIO BAR0 (identity-mapped by UEFI), TX/RX descriptor rings, DMA-capable.
//! Uses polling — no interrupt wiring needed.

use crate::arch::page_alloc;
use crate::drivers::pci::{self, PciDevice};
use core::ptr::{read_volatile, write_volatile};

pub static mut RTL_DRIVER: Option<Rtl8168Driver> = None;

const RTL8168_VENDOR: u16 = 0x10EC;
const RTL8168_DEVICE: u16 = 0x8168;

// ── Register offsets (MMIO BAR0) ──────────────────────────────────────
const IDR0: usize = 0x00;
const TX_DESC_ADDR0: usize = 0x20;
const RX_DESC_ADDR0: usize = 0x28;
const CMD_REG: usize = 0x37;
const RX_MISSED: usize = 0x4C;
const CFG9346_REG: usize = 0x50;
const CONFIG0: usize = 0x51;
const TX_CONFIG: usize = 0x40;
const RX_CONFIG: usize = 0x44;
const IMR_REG: usize = 0x3C;
const PHYSTATUS: usize = 0x6C;

// ── Command register bits ─────────────────────────────────────────────
const CMD_RESET: u8 = 0x10;
const CMD_RX_ENB: u8 = 0x08;
const CMD_TX_ENB: u8 = 0x04;

// ── PHY status bits ───────────────────────────────────────────────────
const PHY_LINK_UP: u8 = 0x04;
const PHY_SPEED_100: u8 = 0x08;
const PHY_SPEED_1000: u8 = 0x10;
const PHY_FULL_DUPLEX: u8 = 0x20;

// ── TX/RX ring size ───────────────────────────────────────────────────
const TX_DESC_COUNT: usize = 64;
const RX_DESC_COUNT: usize = 128;
const RX_BUF_SIZE: usize = 2048;
const TX_BUF_SIZE: usize = 1536;

// ── TX Descriptor (16 bytes) ──────────────────────────────────────────
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TxDesc {
    buf_addr: u64,
    opts1: u32,
    opts2: u32,
}

// ── RX Descriptor (16 bytes) ──────────────────────────────────────────
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RxDesc {
    buf_addr: u64,
    opts1: u32,
    opts2: u32,
}

pub struct Rtl8168Driver {
    bar0: u64,
    mac: [u64; 6],
    tx_ring: *mut TxDesc,
    rx_ring: *mut RxDesc,
    tx_bufs: u64,
    rx_bufs: u64,
    tx_next: usize,
    rx_cur: usize,
    link_up: bool,
    speed_mbps: u32,
    duplex_full: bool,
}

impl Rtl8168Driver {
    pub unsafe fn detect() -> Option<Self> {
        let pci_devs = pci::scan_pci_bus();
        for i in 0..pci_devs.count {
            let dev = pci_devs.devices[i];
            if dev.vendor_id == RTL8168_VENDOR && dev.device_id == RTL8168_DEVICE {
                return Self::init(dev);
            }
        }
        None
    }

    const TIMEOUT_CYCLES: u64 = 7_400_000_000;

    unsafe fn init(dev: PciDevice) -> Option<Self> {
        // Enable PCI Bus Master + Memory Space
        let cmd_reg = pci::pci_read32(dev.bus, dev.device, dev.function, 0x04);
        pci::pci_write32(
            dev.bus, dev.device, dev.function, 0x04,
            (cmd_reg & 0xFFFF) | 0x06,
        );

        // Read BAR0 (memory-mapped I/O). UEFI identity-maps first 4 GB.
        let bar0_raw = pci::pci_read32(dev.bus, dev.device, dev.function, 0x10);
        if bar0_raw == 0 || bar0_raw == 0xFFFFFFFF {
            return None;
        }
        let bar0 = (bar0_raw & 0xFFFFFFF0) as u64;

        // Software reset
        write_volatile((bar0 + CMD_REG as u64) as *mut u8, CMD_RESET);
        let start = crate::arch::cpu::rdtsc();
        loop {
            let v = read_volatile((bar0 + CMD_REG as u64) as *const u8);
            if v & CMD_RESET == 0 {
                break;
            }
            if crate::arch::cpu::rdtsc().wrapping_sub(start) > Self::TIMEOUT_CYCLES {
                return None;
            }
        }

        // Read MAC address from IDR0-IDR5
        let mut mac = [0u64; 6];
        for i in 0..6 {
            mac[i] = read_volatile((bar0 + IDR0 as u64 + i as u64) as *const u8) as u64;
        }

        // Allocate TX ring (1 page = 64 descriptors * 16 bytes = 1024)
        let tx_ring_phys = match page_alloc::alloc_pages_contiguous(1) {
            Some(p) => p,
            None => return None,
        };
        let tx_ring = tx_ring_phys as *mut TxDesc;
        core::ptr::write_bytes(tx_ring as *mut u8, 0, core::mem::size_of::<TxDesc>() * TX_DESC_COUNT);

        // Allocate TX buffer pool (TX_DESC_COUNT * 1536 bytes)
        let tx_buf_pages = (TX_DESC_COUNT * TX_BUF_SIZE + 4095) / 4096;
        let tx_bufs = match page_alloc::alloc_pages_contiguous(tx_buf_pages) {
            Some(p) => p,
            None => return None,
        };
        core::ptr::write_bytes(tx_bufs as *mut u8, 0, TX_DESC_COUNT * TX_BUF_SIZE);

        // Allocate RX ring (1 page = 128 * 16 = 2048)
        let rx_ring_phys = match page_alloc::alloc_pages_contiguous(1) {
            Some(p) => p,
            None => return None,
        };
        let rx_ring = rx_ring_phys as *mut RxDesc;
        core::ptr::write_bytes(rx_ring as *mut u8, 0, core::mem::size_of::<RxDesc>() * RX_DESC_COUNT);

        // Allocate RX buffer pool (RX_DESC_COUNT * RX_BUF_SIZE bytes)
        let rx_buf_pages = (RX_DESC_COUNT * RX_BUF_SIZE + 4095) / 4096;
        let rx_bufs = match page_alloc::alloc_pages_contiguous(rx_buf_pages) {
            Some(p) => p,
            None => return None,
        };
        core::ptr::write_bytes(rx_bufs as *mut u8, 0, RX_DESC_COUNT * RX_BUF_SIZE);

        // Initialize TX descriptors
        for i in 0..TX_DESC_COUNT {
            (*tx_ring.add(i)).buf_addr = tx_bufs + (i * TX_BUF_SIZE) as u64;
            (*tx_ring.add(i)).opts1 = (TX_BUF_SIZE as u32) << 16;
            (*tx_ring.add(i)).opts2 = 0;
        }

        // Initialize RX descriptors — OWN bit set, buffers assigned
        for i in 0..RX_DESC_COUNT {
            (*rx_ring.add(i)).buf_addr = rx_bufs + (i * RX_BUF_SIZE) as u64;
            (*rx_ring.add(i)).opts1 = 1 << 31; // OWN bit = DMA owns
            (*rx_ring.add(i)).opts2 = 0;
        }

        // Program TX/RX descriptor base addresses
        write_volatile((bar0 + TX_DESC_ADDR0 as u64) as *mut u32, tx_ring_phys as u32);
        write_volatile((bar0 + TX_DESC_ADDR0 as u64 + 4) as *mut u32, (tx_ring_phys >> 32) as u32);
        write_volatile((bar0 + RX_DESC_ADDR0 as u64) as *mut u32, rx_ring_phys as u32);
        write_volatile((bar0 + RX_DESC_ADDR0 as u64 + 4) as *mut u32, (rx_ring_phys >> 32) as u32);

        // Configure TX: IFG=11, DMA burst
        write_volatile((bar0 + TX_CONFIG as u64) as *mut u32, 0x03000000);

        // Configure RX: accept broadcast, multicast, all physical
        write_volatile((bar0 + RX_CONFIG as u64) as *mut u32, 0x0000000F);

        // Clear missed counter
        write_volatile((bar0 + RX_MISSED as u64) as *mut u32, 0);

        // Enable TX and RX
        let cmd = read_volatile((bar0 + CMD_REG as u64) as *const u8);
        write_volatile((bar0 + CMD_REG as u64) as *mut u8, cmd | CMD_TX_ENB | CMD_RX_ENB);

        // Mask all interrupts (polling mode)
        write_volatile((bar0 + IMR_REG as u64) as *mut u16, 0);

        Some(Self {
            bar0,
            mac,
            tx_ring,
            rx_ring,
            tx_bufs,
            rx_bufs,
            tx_next: 0,
            rx_cur: 0,
            link_up: false,
            speed_mbps: 0,
            duplex_full: false,
        })
    }

    unsafe fn update_link_status(&mut self) {
        let phy = read_volatile((self.bar0 + PHYSTATUS as u64) as *const u8);
        self.link_up = (phy & PHY_LINK_UP) != 0;
        if self.link_up {
            if (phy & PHY_SPEED_1000) != 0 {
                self.speed_mbps = 1000;
            } else if (phy & PHY_SPEED_100) != 0 {
                self.speed_mbps = 100;
            } else {
                self.speed_mbps = 10;
            }
            self.duplex_full = (phy & PHY_FULL_DUPLEX) != 0;
        } else {
            self.speed_mbps = 0;
            self.duplex_full = false;
        }
    }

    pub fn mac_address(&self) -> [u8; 6] {
        [
            self.mac[0] as u8, self.mac[1] as u8, self.mac[2] as u8,
            self.mac[3] as u8, self.mac[4] as u8, self.mac[5] as u8,
        ]
    }

    pub fn is_link_up(&mut self) -> bool {
        unsafe { self.update_link_status() };
        self.link_up
    }

    pub fn speed(&mut self) -> u32 {
        unsafe { self.update_link_status() };
        self.speed_mbps
    }

    pub fn send(&mut self, pkt: &[u8]) -> Result<usize, &'static str> {
        if pkt.len() > 1514 {
            return Err("packet too large");
        }

        let idx = self.tx_next;
        let desc = unsafe { &mut *self.tx_ring.add(idx) };

        // Check OWN bit — must be owned by CPU
        if (desc.opts1 & (1u32 << 31)) == 0 {
            return Err("TX ring full");
        }

        // Copy packet into TX buffer
        let buf = (self.tx_bufs + (idx * TX_BUF_SIZE) as u64) as *mut u8;
        unsafe { core::ptr::copy_nonoverlapping(pkt.as_ptr(), buf, pkt.len()); }

        // Set descriptor: length | FS | LS, clear OWN (start DMA)
        let len = pkt.len() as u32;
        desc.opts1 = (len << 16) | (1u32 << 29) | (1u32 << 28); // FS | LS

        self.tx_next = (idx + 1) % TX_DESC_COUNT;
        Ok(pkt.len())
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        let idx = self.rx_cur;
        let desc = unsafe { &*self.rx_ring.add(idx) };

        // OWN bit set = DMA still owns it
        if (desc.opts1 & (1u32 << 31)) != 0 {
            return None;
        }

        // Check for error
        if (desc.opts1 & 0x01) != 0 {
            unsafe { (*self.rx_ring.add(idx)).opts1 = 1u32 << 31; }
            self.rx_cur = (idx + 1) % RX_DESC_COUNT;
            return None;
        }

        let pkt_len = (desc.opts1 & 0xFFFF) as usize;
        if pkt_len == 0 || pkt_len > buf.len() {
            unsafe { (*self.rx_ring.add(idx)).opts1 = 1u32 << 31; }
            self.rx_cur = (idx + 1) % RX_DESC_COUNT;
            return None;
        }

        let src = (self.rx_bufs + (idx * RX_BUF_SIZE) as u64) as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), pkt_len);
            (*self.rx_ring.add(idx)).opts1 = 1u32 << 31; // Re-arm
        }

        self.rx_cur = (idx + 1) % RX_DESC_COUNT;
        Some(pkt_len)
    }

    pub fn chip_version(&self) -> u32 {
        unsafe {
            let c = read_volatile((self.bar0 + CFG9346_REG as u64) as *const u8);
            let g = read_volatile((self.bar0 + CONFIG0 as u64) as *const u8);
            ((g as u32) << 8) | (c as u32)
        }
    }
}

pub unsafe fn send_frame(dst_mac: &[u8; 6], ethertype: u16, payload: &[u8]) -> Result<usize, &'static str> {
    let drv = RTL_DRIVER.as_mut().ok_or("RTL8168 not initialized")?;
    let mut pkt = [0u8; 1514];
    let hdr_len = 14;

    pkt[0..6].copy_from_slice(dst_mac);
    let mac = drv.mac_address();
    pkt[6..12].copy_from_slice(&mac);
    pkt[12] = (ethertype >> 8) as u8;
    pkt[13] = ethertype as u8;

    let copy_len = payload.len().min(pkt.len() - hdr_len);
    pkt[hdr_len..hdr_len + copy_len].copy_from_slice(&payload[..copy_len]);

    drv.send(&pkt[..hdr_len + copy_len])
}

pub unsafe fn recv_frame(buf: &mut [u8]) -> Option<(u16, [u8; 6], usize)> {
    let drv = RTL_DRIVER.as_mut()?;
    let len = drv.recv(buf)?;
    if len < 14 {
        return None;
    }

    let ethertype = ((buf[12] as u16) << 8) | (buf[13] as u16);
    let mut src_mac = [0u8; 6];
    src_mac.copy_from_slice(&buf[6..12]);

    Some((ethertype, src_mac, len))
}
