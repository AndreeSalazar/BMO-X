======================================================
  FastOS - UEFI Native Boot Image
======================================================

  Bootloader: BOOTX64.EFI (39KB)
  Kernel:    kernel.elf (251KB)
  Built:     2026-04-20 21:04:06
  Target:    Ryzen 5 5600X + RTX 3060 12G
  Mode:      UEFI Native (No CSM/Legacy)

------------------------------------------------------
  HOW TO FLASH TO USB (UEFI) - AUTOMATED
------------------------------------------------------

  Option 1 - flash_uefi.ps1 (RECOMMENDED, simple):
    .\flash_uefi.ps1 -DiskNumber <N>
    
    Example: If your USB is Disk 3:
    .\flash_uefi.ps1 -DiskNumber 3

  Option 2 - flash_direct.ps1 (auto-detect USB):
    .\flash_direct.ps1
    # Or: .\flash_direct.ps1 -DiskNumber 3

  Both scripts will:
    - Format USB as GPT + FAT32 (full size ESP)
    - Create EFI\BOOT\ directory
    - Copy BOOTX64.EFI to EFI\BOOT\BOOTX64.EFI
    - Copy kernel.elf to root
    - Make USB bootable

  Option 3 - Manual:
    1. Format USB as GPT + FAT32 (full size)
       - Disk Management -> Delete partitions -> New -> GPT -> FAT32
    2. Copy EFI files to ESP
       - Create: EFI\BOOT\ on USB
       - Copy: BOOTX64.EFI -> EFI\BOOT\BOOTX64.EFI
       - Copy: kernel.elf -> root of ESP

  BIOS Setup:
    - Disable CSM/Legacy Boot (set to UEFI Only)
    - Disable Secure Boot
    - Add USB to boot order
    - Select USB from UEFI boot menu

------------------------------------------------------
  BOOT SEQUENCE
------------------------------------------------------
  1. UEFI firmware loads BOOTX64.EFI
  2. Bootloader queries GOP (framebuffer)
  3. Bootloader loads kernel.elf (ELF64)
  4. Bootloader finds RSDP (ACPI)
  5. Bootloader builds BootInfo struct
  6. Bootloader exits boot services
  7. Bootloader jumps to kernel _start
  8. Kernel validates BootInfo, inits serial
  9. Kernel inits PIC/IDT/PIT, enables IRQs
  10. Kernel runs interactive shell on GOP FB

======================================================
