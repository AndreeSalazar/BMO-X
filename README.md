# FastOS

> **Minimal custom OS for AMD Ryzen 5 5600X + NVIDIA RTX 3060 12G**
> Pure Rust · UEFI Native · Ring 0 · No userspace

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                    FastOS Stack — ALL RING 0                     │
├──────────────────────────────────────────────────────────────────┤
│  Driver_Canon GA106 (Rust, #![no_std])                           │
│  ├── nv_kernel/    │ Driver entry: init → enable → teardown      │
│  ├── nv_gpu/       │ GPU core: BARs, engines, VRAM, interrupts   │
│  ├── nv_cmd/       │ FIFO: channels, pushbuffers, fences         │
│  ├── nv_display/   │ Display: heads, SOR, EDID, modeset          │
│  ├── nv_firmware/  │ FALCON loader: GSP, PMU, SEC2               │
│  ├── nv_hal/       │ HAL: MMIO, PCI config, DMA, Platform trait  │
│  ├── nv_regs/      │ GA106 register map                          │
│  └── nv_error/     │ NV_ERR_* codes                              │
├──────────────────────────────────────────────────────────────────┤
│  Rust Kernel (#![no_std], Ring 0)                                │
│  ├── platform.rs   │ FastOsPlatform → implements nv_hal::Platform│
│  ├── arch/         │ CPUID, IDT, ACPI/MCFG, page allocator       │
│  ├── drivers/      │ PCI ECAM, serial COM1, GPU, GSP loader      │
│  ├── gpu/          │ FIFO pushbuffer, DMA, command submission    │
│  ├── fb.rs         │ Framebuffer drawing (GOP linear FB)         │
│  ├── console.rs    │ Text console over framebuffer               │
│  ├── render3d.rs   │ Software 3D cube renderer                   │
│  ├── shell.rs      │ Interactive command interpreter             │
│  └── main.rs       │ Kernel entry point (_start)                 │
├──────────────────────────────────────────────────────────────────┤
│  UEFI Bootloader (Rust, uefi-rs 0.37)                            │
│  ├── main.rs       │ UEFI entry point                            │
│  ├── ELF64 parser  │ Manual ELF loading (no external crate)      │
│  ├── GOP query     │ Framebuffer from UEFI GOP                   │
│  ├── RSDP lookup   │ ACPI tables from UEFI config table          │
│  ├── GSP loader    │ Loads gsp_ga10x.bin into DMA-safe pages     │
│  ├── Exit BS       │ Exits UEFI boot services                    │
│  └── Kernel jump   │ Sets RSP, passes BootInfo in RDI            │
├──────────────────────────────────────────────────────────────────┤
│  Hardware                                                        │
│  CPU: AMD Ryzen 5 5600X (Zen 3, 6C/12T)                          │
│  GPU: NVIDIA RTX 3060 12GB (GA106, Ampere)                       │
│  Firmware: UEFI Native (CSM Disabled)                            │
└──────────────────────────────────────────────────────────────────┘
```

## Boot Sequence

1. UEFI firmware loads `BOOTX64.EFI` from ESP
2. Bootloader queries GOP framebuffer
3. Bootloader loads `kernel.elf` (ELF64, manual parser)
4. Bootloader loads `gsp_ga10x.bin` (optional GPU firmware)
5. Bootloader finds RSDP via UEFI config tables
6. Bootloader builds BootInfo struct
7. Bootloader exits boot services (point of no return)
8. Bootloader jumps to kernel `_start` with BootInfo in RDI
9. Kernel validates BootInfo, inits serial COM1
10. Kernel loads IDT (exception handlers)
11. Kernel parses ACPI MCFG → PCI ECAM base
12. Kernel scans PCI bus via ECAM MMIO
13. Kernel initializes page frame allocator from UEFI memory map
14. Kernel starts interactive shell on GOP framebuffer

## Building

### Prerequisites
- **Rust nightly** with `rust-src` component
- **UEFI target**: `x86_64-unknown-uefi`
- **Kernel target**: `x86_64-unknown-none`

### Complete Build
```powershell
.\build_uefi.ps1        # Build bootloader + kernel
.\build_uefi.ps1 -Clean # Clean all build artifacts
```

Output:
- `BOOTX64.EFI` — UEFI bootloader
- `kernel.elf` — Kernel binary
- `USB_boot/` — Ready to copy to FAT32 USB

### Flash to USB
```powershell
.\flash_uefi.ps1 -DiskNumber <N>
```

### GPU Firmware (Optional)
Place `gsp_ga10x.bin` in the repo root. The bootloader loads it from the ESP
and passes its address to the kernel via BootInfo. This is the NVIDIA GSP-RM
firmware required for full GPU initialization on Ampere GPUs.

## Verified on Hardware

| Feature | Status |
|---------|--------|
| UEFI boot from USB | ✅ Works |
| GOP framebuffer (1920×1080) | ✅ Works |
| Serial COM1 debug output | ✅ Works |
| ACPI MCFG → PCI ECAM | ✅ Works |
| PCI bus scan | ✅ Works |
| GPU PCI discovery (10DE:2504) | ✅ Works |
| GPU BAR0 MMIO register access | ✅ Works |
| GPU BOOT_0 chip ID read | ✅ Works |
| GPU PTIMER liveness | ✅ Works |
| GPU VRAM detection (12 GB) | ✅ Works |
| GPU engine enable mask | ✅ Works |
| GPU PFIFO/PGRAPH/PBDMA/CE/Display regs | ✅ Works |
| IDT (exceptions → halt, not triple-fault) | ✅ Works |
| Page frame allocator | ✅ Works |
| Text console with scroll | ✅ Works |
| Interactive shell (serial input) | ✅ Works |
| 3D rotating cube (software renderer) | ✅ Works |
| GSP PRIV Ring init | ✅ Works |
| GSP Falcon scratch register W/R | ✅ Works |

## Shell Commands

All commands operate on live hardware state:

| Command | Description |
|---------|-------------|
| `cpuinfo` | CPU features via CPUID |
| `gpuinfo` | GPU PCI/BAR0/VRAM (live reads) |
| `pci` | PCI device list (ECAM scan) |
| `meminfo` | UEFI memory map summary |
| `gputest` | 15-test GPU register probe suite |
| `gpucmd` | GPU pushbuffer command engine |
| `cube` | 3D rotating cube (software) |
| `tsc` | TSC counter value |

## Project Structure

```
FastOS/
├── bootloader/           # UEFI bootloader (Rust, uefi-rs 0.37)
├── boot_protocol/        # Shared BootInfo types
├── kernel/               # Rust kernel (x86_64-unknown-none)
│   └── src/
│       ├── arch/          # CPU, IDT, ACPI, page allocator, paging
│       ├── drivers/       # PCI, serial, GPU, GSP loader
│       ├── gpu/           # FIFO, DMA, pushbuffer, commands
│       ├── tests/         # Live GPU register test suite
│       └── *.rs           # Core kernel modules
├── Driver_Canon GA106/   # 8-crate GPU driver workspace
│   ├── nv_error/         # Error codes
│   ├── nv_regs/          # GA106 register definitions
│   ├── nv_hal/           # Hardware abstraction layer
│   ├── nv_gpu/           # GPU core (init, VRAM, engines)
│   ├── nv_cmd/           # Command submission (FIFO)
│   ├── nv_display/       # Display engine
│   ├── nv_firmware/      # Falcon firmware loader
│   └── nv_kernel/        # Top-level driver orchestration
├── build_uefi.ps1        # Build pipeline
└── flash_uefi.ps1        # USB flash tool
```

## License

MIT
