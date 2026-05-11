"""
FastOS Payload Generator v3.0 — DMA Falcon Boot
=================================================
SEC2 HWCFG returns real value (0xB0420100) = engine accessible.
But CPUCTL returns 0xBADF5620 = Falcon in High Secure mode.
HS mode blocks direct IMEM/DMEM writes. Must use DMA transfer.

New opcode: OP_FALCON_DMA (0x06)
  - reg = DMA transfer base register (e.g., 0x840110 for SEC2)
  - data = firmware to transfer into Falcon IMEM/DMEM
  - Kernel allocates DMA buffer, copies data, transfers in 256-byte chunks
"""

import struct
import os
import sys

OP_WRITE32 = 0x01
OP_POLL32 = 0x02
OP_WRITE_BLOCK = 0x03
OP_SETUP_WPR2 = 0x04
OP_READ32 = 0x05
OP_FALCON_DMA = 0x06

FIRMWARE_DIR = os.path.join(os.path.dirname(__file__), "USB_boot", "firmware")
OUTPUT_FILE = os.path.join(os.path.dirname(__file__), "fastos_boot.bin")


class PayloadBuilder:
    def __init__(self):
        self.entries = []
        self.log = []

    def write32(self, reg, val, desc=""):
        self.entries.append(struct.pack("<B I I I", OP_WRITE32, reg, 4, val))
        self.log.append(f"[{len(self.entries):02d}] WRITE32  0x{reg:06X} <- 0x{val:08X}  // {desc}")

    def read32(self, reg, desc=""):
        self.entries.append(struct.pack("<B I I", OP_READ32, reg, 0))
        self.log.append(f"[{len(self.entries):02d}] READ32   0x{reg:06X}              // {desc}")

    def poll32(self, reg, mask, expected, desc=""):
        payload = struct.pack("<I I", mask, expected)
        self.entries.append(struct.pack("<B I I", OP_POLL32, reg, 8) + payload)
        self.log.append(f"[{len(self.entries):02d}] POLL32   0x{reg:06X} & 0x{mask:08X} == 0x{expected:08X} // {desc}")

    def write_block(self, reg, data, desc=""):
        padded = data
        remainder = len(data) % 4
        if remainder != 0:
            padded = data + b'\x00' * (4 - remainder)
        self.entries.append(struct.pack("<B I I", OP_WRITE_BLOCK, reg, len(padded)) + padded)
        self.log.append(f"[{len(self.entries):02d}] W_BLOCK  0x{reg:06X} <- {len(data):,} bytes // {desc}")

    def falcon_dma(self, engine_base, target, data, desc=""):
        """
        DMA transfer firmware to Falcon IMEM or DMEM.
        engine_base: base register of the Falcon engine (e.g., 0x840000 for SEC2)
        target: 0 = IMEM, 1 = DMEM
        data: firmware bytes to load
        """
        # Pad to 256 bytes (DMA transfer unit)
        padded = data
        remainder = len(data) % 256
        if remainder != 0:
            padded = data + b'\x00' * (256 - remainder)
        # Encode: [engine_base:u32][target:u32][padded_data]
        header = struct.pack("<I I", engine_base, target)
        total = header + padded
        self.entries.append(struct.pack("<B I I", OP_FALCON_DMA, engine_base, len(total)) + total)
        tname = "IMEM" if target == 0 else "DMEM"
        self.log.append(f"[{len(self.entries):02d}] FLC_DMA  0x{engine_base:06X} {tname} <- {len(data):,} bytes ({len(padded):,} padded) // {desc}")

    def build(self):
        header = struct.pack("<4s I I", b'FOSB', 3, len(self.entries))
        return header + b''.join(self.entries)


def main():
    print("=" * 60)
    print(" FastOS GA106 Payload Generator v3.0 (DMA Boot)")
    print("=" * 60)

    bootloader_path = os.path.join(FIRMWARE_DIR, "bootloader-535.113.01.bin")
    booter_load_path = os.path.join(FIRMWARE_DIR, "booter_load-535.113.01.bin")

    for path, name in [(bootloader_path, "bootloader"), (booter_load_path, "booter_load")]:
        if not os.path.exists(path):
            print(f"[ERROR] {name} not found: {path}")
            sys.exit(1)

    with open(bootloader_path, 'rb') as f:
        bootloader_data = f.read()
    with open(booter_load_path, 'rb') as f:
        booter_load_data = f.read()

    print(f"[*] bootloader: {len(bootloader_data):,} bytes")
    print(f"[*] booter_load: {len(booter_load_data):,} bytes")

    SEC2_BASE = 0x840000

    b = PayloadBuilder()

    # ── Phase 1: PRIV Ring ──
    b.write32(0x12004C, 0x00000001, "PRIV_SYS_INIT")
    b.write32(0x122204, 0x00000001, "PRIV_RING_START")
    b.poll32(0x122100, 0x00000001, 0x00000001, "PRIV_RING_STATUS")

    # ── Phase 2: Engine Enable ──
    b.write32(0x000200, 0xFFFFFFFF, "PMC_ENABLE")
    b.write32(0x000600, 0xFFFFFFFF, "PMC_DEVICE_ENABLE")

    # ── Diagnostic: Probe DMA registers ──
    b.read32(SEC2_BASE + 0x108, "SEC2_HWCFG")
    b.read32(SEC2_BASE + 0x110, "SEC2_DMATRFBASE (before)")
    b.read32(SEC2_BASE + 0x11C, "SEC2_DMATRFCMD (before)")
    b.read32(SEC2_BASE + 0x100, "SEC2_CPUCTL (before DMA)")

    # ── Phase 3: WPR2 (hardcoded 12GB) ──
    b.write32(0x100CD4, 0x00002F80, "WPR2_START")
    b.write32(0x100CD8, 0x00003000, "WPR2_END")

    # ── Phase 4: DMA-based Falcon firmware load ──
    # Load bootloader to IMEM via DMA
    b.falcon_dma(SEC2_BASE, 0, bootloader_data, "bootloader-535 -> IMEM via DMA")
    # Load booter_load to DMEM via DMA
    b.falcon_dma(SEC2_BASE, 1, booter_load_data, "booter_load-535 -> DMEM via DMA")

    # ── Diagnostic: Check state after DMA ──
    b.read32(SEC2_BASE + 0x100, "SEC2_CPUCTL (after DMA)")
    b.read32(SEC2_BASE + 0x110, "SEC2_DMATRFBASE (after)")

    # ── Phase 5: Boot Falcon ──
    b.write32(SEC2_BASE + 0x104, 0x00000000, "SEC2_BOOTVEC = 0")
    b.write32(SEC2_BASE + 0x100, 0x00000002, "SEC2_CPUCTL Start")

    # ── Diagnostic after start ──
    b.read32(SEC2_BASE + 0x100, "SEC2_CPUCTL (after start)")
    b.read32(SEC2_BASE + 0x008, "SEC2_IRQSTAT (after start)")
    b.read32(SEC2_BASE + 0x040, "SEC2_MAILBOX0")
    b.read32(SEC2_BASE + 0x044, "SEC2_MAILBOX1")

    # ── Phase 6: Poll for result ──
    b.poll32(SEC2_BASE + 0x008, 0x00000010, 0x00000010, "SEC2_IRQSTAT (wait HALT)")

    # Build
    binary_data = b.build()
    with open(OUTPUT_FILE, 'wb') as f:
        f.write(binary_data)

    print()
    print("[+] Payload Log:")
    for log in b.log:
        print("    " + log)

    print()
    print(f"[+] Generated: {len(binary_data):,} bytes, {len(b.entries)} entries")
    print()
    print("Key diagnostics:")
    print("  SEC2_DMATRFBASE/CMD: non-0xBADF5620 = DMA regs accessible")
    print("  SEC2_CPUCTL after DMA: changed = firmware loaded")
    print("  SEC2_MAILBOX0/1: status from Falcon after boot")


if __name__ == "__main__":
    main()
