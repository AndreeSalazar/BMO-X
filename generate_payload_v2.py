"""
FastOS Payload Generator v5.2 — Correct Section Loading
==========================================================
v5.1 result: CPUCTL=0x00 (CPU RUNNING!) but stuck (never halts).
Reason: loaded entire file to IMEM/DMEM. Must load correct sections.

File structure (from analysis):
  bootloader-535 (20588 bytes):
    Header: 0x00-0x6B (108 bytes)  
    Data:   0x6C-0x506B (20480 bytes) → DMEM
    Code is embedded, BROM handles extraction
    
  booter_load-535 (59768 bytes):
    Header: 0x00-0x377 (888 bytes)
    Data:   0x378-0xE977 (58880 bytes) → DMEM

nvidia-open approach for BootFromHs:
  1. IMEM gets CODE section from bootloader image
  2. DMEM gets DATA section from bootloader image
  3. booter_load goes to system memory (BROM DMA's it)

Since we can't DMA, let's try:
  - Load bootloader DATA section (signed blob) to DMEM at offset 0
  - Don't load to IMEM (BROM has its own ROM code)
  - Set BOOTVEC=0 (BROM entry)
  - BROM reads DMEM, authenticates, loads code
"""

import struct
import os
import sys

OP_WRITE32 = 0x01
OP_POLL32 = 0x02
OP_WRITE_BLOCK = 0x03
OP_READ32 = 0x05

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

    def build(self):
        header = struct.pack("<4s I I", b'FOSB', 5, len(self.entries))
        return header + b''.join(self.entries)


def main():
    print("=" * 60)
    print(" FastOS GA106 v5.2 — Section-Correct Loading")
    print("=" * 60)

    bootloader_path = os.path.join(FIRMWARE_DIR, "bootloader-535.113.01.bin")
    booter_load_path = os.path.join(FIRMWARE_DIR, "booter_load-535.113.01.bin")

    for path, name in [(bootloader_path, "bootloader"), (booter_load_path, "booter_load")]:
        if not os.path.exists(path):
            print(f"[ERROR] {name} not found: {path}")
            sys.exit(1)

    with open(bootloader_path, 'rb') as f:
        bl_raw = f.read()
    with open(booter_load_path, 'rb') as f:
        br_raw = f.read()

    # Parse bootloader header
    bl_data_off = struct.unpack_from('<I', bl_raw, 0x10)[0]  # 0x6C
    bl_data_size = struct.unpack_from('<I', bl_raw, 0x14)[0]  # 0x5000
    bl_data = bl_raw[bl_data_off:bl_data_off + bl_data_size]
    bl_header = bl_raw[:bl_data_off]  # Everything before data

    # Parse booter_load header  
    br_data_off = struct.unpack_from('<I', br_raw, 0x10)[0]  # 0x378
    br_data_size = struct.unpack_from('<I', br_raw, 0x14)[0]  # 0xE600
    br_data = br_raw[br_data_off:br_data_off + br_data_size]
    br_header = br_raw[:br_data_off]

    print(f"[*] bootloader: {len(bl_raw):,} bytes")
    print(f"    header: {len(bl_header):,} bytes (0x00-0x{bl_data_off-1:X})")
    print(f"    data:   {len(bl_data):,} bytes (0x{bl_data_off:X}-0x{bl_data_off+bl_data_size-1:X})")
    print(f"[*] booter_load: {len(br_raw):,} bytes")
    print(f"    header: {len(br_header):,} bytes (0x00-0x{br_data_off-1:X})")
    print(f"    data:   {len(br_data):,} bytes (0x{br_data_off:X}-0x{br_data_off+br_data_size-1:X})")

    SEC2 = 0x840000
    SEC2_RISCV = 0x841000
    BCR_CTRL = SEC2_RISCV + 0x668

    b = PayloadBuilder()

    # ═══ Phase 1: PRIV Ring ═══
    b.write32(0x12004C, 0x00000001, "PRIV_SYS_INIT")
    b.write32(0x122204, 0x00000001, "PRIV_RING_START")
    b.poll32(0x122100, 0x00000001, 0x00000001, "PRIV_RING_STATUS")
    b.write32(0x000200, 0xFFFFFFFF, "PMC_ENABLE")
    b.write32(0x000600, 0xFFFFFFFF, "PMC_DEVICE_ENABLE")

    # ═══ Phase 2: BCR_CTRL + SRESET ═══
    b.write32(BCR_CTRL, 0x00000000, "BCR_CTRL (RISCV->Falcon)")
    b.write32(SEC2 + 0x100, 0x00000040, "CPUCTL SRESET")
    b.read32(SEC2 + 0x100, "CPUCTL after SRESET")
    b.write32(SEC2 + 0x10C, 0x00000000, "DMACTL=0")
    b.write32(0x840600, 0x00000005, "FBIF_TRANSCFG")

    # ═══ Phase 3: Read HWCFG for IMEM size ═══
    b.read32(SEC2 + 0x108, "HWCFG (IMEM_SIZE in bits 8:0)")

    # ═══ Phase 4: Load bootloader HEADER to DMEM offset 0 ═══
    # The header contains descriptor info the BROM needs
    b.write32(SEC2 + 0x1C0, 0x01000000, "DMEMC (auto-inc, offset 0)")
    b.write_block(SEC2 + 0x1C4, bl_raw, "DMEMD <- entire bootloader")

    # ═══ Phase 5: Load bootloader to IMEM (entire file) ═══
    b.write32(SEC2 + 0x180, 0x01000000, "IMEMC (auto-inc, offset 0)")
    b.write_block(SEC2 + 0x184, bl_raw, "IMEMD <- entire bootloader")

    # Readback verify
    b.write32(SEC2 + 0x180, 0x02000000, "IMEMC (read, offset 0)")
    b.read32(SEC2 + 0x184, "IMEM[0] verify")

    # ═══ Phase 6: PKC BROM registers ═══
    b.write32(SEC2_RISCV + 0x180, 0x00000001, "MOD_SEL=RSA3K")
    b.write32(SEC2_RISCV + 0x198, 0x00000000, "BROM_UCODE_ID")
    b.write32(SEC2_RISCV + 0x19C, 0x00000000, "BROM_ENGIDMASK")
    b.write32(SEC2_RISCV + 0x210, 0x00000000, "BROM_PARAADDR(0)")

    # ═══ Phase 7: BOOTVEC + Start ═══
    b.write32(SEC2 + 0x104, 0x00000000, "BOOTVEC = 0")
    b.write32(SEC2 + 0x040, 0x00000000, "MAILBOX0 = 0")
    b.write32(SEC2 + 0x044, 0x00000000, "MAILBOX1 = 0")
    b.write32(SEC2 + 0x100, 0x00000002, "CPUCTL Start")

    # ═══ Phase 8: Immediate diagnostics ═══
    b.read32(SEC2 + 0x100, "CPUCTL (immediate)")
    b.read32(SEC2 + 0x008, "IRQSTAT (immediate)")

    # ═══ Phase 9: Poll HALT with longer reads ═══
    # Read multiple times with spacing to catch state changes
    b.read32(SEC2 + 0x040, "MAILBOX0 (early)")
    b.read32(SEC2 + 0x044, "MAILBOX1 (early)")
    b.read32(SEC2 + 0x100, "CPUCTL (mid)")
    b.read32(SEC2 + 0x008, "IRQSTAT (mid)")

    # Final poll
    b.poll32(SEC2 + 0x008, 0x00000010, 0x00000010, "IRQSTAT wait HALT")

    b.read32(SEC2 + 0x040, "MAILBOX0 (final)")
    b.read32(SEC2 + 0x044, "MAILBOX1 (final)")
    b.read32(SEC2 + 0x100, "CPUCTL (final)")
    b.read32(SEC2 + 0x008, "IRQSTAT (final)")
    # Also read EXC_ADDR for any exception info
    b.read32(SEC2 + 0x00C, "IRQMODE (exception info)")
    b.read32(SEC2 + 0x01C, "EXCI (exception PC)")

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
    print("v5.2: Load entire bootloader to BOTH IMEM and DMEM")
    print("  + Exception registers (IRQMODE, EXCI) for debugging")
    print("  + Multiple diagnostic reads during execution")


if __name__ == "__main__":
    main()
