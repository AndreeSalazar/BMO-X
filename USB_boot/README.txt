======================================================
  FastOS - UEFI Native Boot Image
======================================================

  Bootloader: BOOTX64.EFI (2KB)
  Kernel:    kernel.bin (0KB)
  Built:     2026-04-20 10:27:55
  Target:    Ryzen 5 5600X + RTX 3060 12G
  Mode:      UEFI Native (No CSM/Legacy)

------------------------------------------------------
  HOW TO FLASH TO USB (UEFI) - AUTOMATED
------------------------------------------------------

  Option 1 - Automated Script (RECOMMENDED):
    .\flash_uefi.ps1 -DiskNumber <N>
    
    Example: If your USB is Disk 3:
    .\flash_uefi.ps1 -DiskNumber 3

  The script will:
    - Format USB as GPT + FAT32 (ESP)
    - Create EFI\BOOT\ directory
    - Copy BOOTX64.EFI to EFI\BOOT\BOOTX64.EFI
    - Copy kernel.bin to root
    - Make USB bootable

  Option 2 - Manual:
    1. Format USB as GPT + FAT32 (ESP)
       - Disk Management â†’ Delete partitions â†’ New â†’ GPT â†’ FAT32
    2. Copy EFI files to ESP
       - Create: EFI\BOOT\ on USB
       - Copy: BOOTX64.EFI â†’ EFI\BOOT\BOOTX64.EFI
       - Copy: kernel.bin â†’ root of ESP

  Step 3: Boot from USB
    - Disable CSM/Legacy Boot in BIOS (set to UEFI Only)
    - Add USB to boot order
    - Select USB from UEFI boot menu

------------------------------------------------------
  MEMORY MAP (UEFI Native)
------------------------------------------------------
  Bootloader: Loaded by UEFI firmware
  Kernel:     Loaded at 0x100000 (1MB) by bootloader
  DMA:        0x400000 (4MB) buffer pool
  Stack:      0x800000 (8MB) grows down

------------------------------------------------------
  ADVANTAGES OF UEFI NATIVE
------------------------------------------------------
  - No legacy BIOS limitations (INT 15h, MBR, etc.)
  - GPT partition support (> 2TB)
  - Secure Boot support
  - Faster boot times
  - Modern firmware interface
  - Better driver support

======================================================
