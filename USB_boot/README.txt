======================================================
  FastOS - USB Boot Image
======================================================

  Image:   fastos.img (71638KB)
  Built:   2026-04-20 08:39:02
  Target:  Ryzen 5 5600X + RTX 3060 12G

------------------------------------------------------
  HOW TO FLASH TO USB
------------------------------------------------------

  Option 1 - PowerShell (included script):
    .\flash_usb.ps1 -DiskNumber <N> -Partition 3

  Option 2 - dd (Linux/WSL):
    sudo dd if=fastos.img of=/dev/sdX bs=512

  Option 3 - Rufus:
    Select fastos.img, write in "DD Image" mode

  IMPORTANT: Use a USB 2.0 port if possible.
  Some UEFI CSM implementations have issues with
  legacy boot from USB 3.0 ports.

------------------------------------------------------
  MEMORY MAP (bare metal)
------------------------------------------------------
  0x007C00          MBR (stage1)
  0x007E00          Stage2
  0x020000          Kernel load buffer (256KB)
  0x100000 (1MB)    Kernel final location
  0x400000 (4MB)    DMA buffer pool
  0x800000 (8MB)    Stack (grows down)

------------------------------------------------------
  TROUBLESHOOTING
------------------------------------------------------
  Q: Only see "Stage1: MBR loaded" repeated?
  A: Stage2 may not be loading. Check:
     - USB is formatted correctly (raw image, not partition)
     - Try a different USB port (prefer USB 2.0)
     - Check BIOS: enable CSM/Legacy Boot

  Q: See "S2" at bottom-left of screen?
  A: Stage2 code IS reached but crashes before printing.
     This is a CPU/memory init issue - report the exact
     screen contents.

  Q: See "NO LBA EXTENSIONS"?
  A: Your BIOS doesn't support INT 13h extended reads.
     This is very rare on modern hardware.

  Q: See "STAGE2 DATA INVALID"?
  A: INT 13h claimed success but loaded wrong data.
     Try re-flashing the USB drive.

======================================================
