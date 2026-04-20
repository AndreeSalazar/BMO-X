# FastOS

> **Minimal custom OS for AMD Ryzen 5 5600X + NVIDIA RTX 3060 12G**
> Pure Rust architecture: UEFI Bootloader → Rust (kernel + drivers)
> **Fagging-Scale: Everything Ring 0 — no userspace, no abstractions.**
> **UEFI Native — No CSM/Legacy BIOS**

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
│  ├── nv_regs/      │ GA106 register map (from SigDead-BIB)       │
│  └── nv_error/     │ NV_ERR_* codes (from nvlddmkm.sys strings)  │
├──────────────────────────────────────────────────────────────────┤
│  Rust Kernel (#![no_std], Ring 0)                                │
│  ├── platform.rs   │ FastOsPlatform → implements nv_hal::Platform│
│  ├── arch/         │ CPUID, paging (CR3), ACPI/RSDP, rdtsc       │
│  ├── drivers/      │ PCI scan, serial COM1, GPU (→ nv_kernel)    │
│  ├── fs/           │ exFAT filesystem structures                 │
│  ├── vga.rs        │ VGA text-mode 80×25 at 0xB8000              │
│  └── main.rs       │ Kernel entry point (_start)                 │
├──────────────────────────────────────────────────────────────────┤
│  UEFI Bootloader (Rust, uefi-rs 0.26)                            │
│  ├── main.rs       │ UEFI entry point, loads kernel from ESP     │
│  ├── File I/O      │ Reads kernel.bin from EFI System Partition  │
│  ├── Memory Map    │ Gets memory map from UEFI firmware          │
│  ├── Exit BS       │ Exits boot services before kernel jump      │
│  └── Kernel Jump   │ Transfers control to kernel at 0x100000     │
├──────────────────────────────────────────────────────────────────┤
│  Hardware                                                        │
│  CPU: AMD Ryzen 5 5600X (Zen 3, 6C/12T, AVX2, AES-NI)            │
│  GPU: NVIDIA RTX 3060 12GB (GA106, Ampere)                       │
│  RAM: DDR4                                                       │
│  Firmware: UEFI Native (CSM Disabled)                            │
└──────────────────────────────────────────────────────────────────┘
```

## Fagging-Scale Philosophy

Everything runs at **Ring 0**. No user/kernel split, no syscalls, no context switches.
The kernel IS the application. Direct hardware access everywhere:

- **PCI** → I/O ports 0xCF8/0xCFC (inline asm)
- **GPU** → MMIO via identity-mapped BAR0 (volatile read/write)
- **Display** → Direct register writes to display heads
- **Timer** → rdtsc for nanosecond precision
- **Memory** → Identity-mapped first 4GB, no virtual memory overhead

## Boot Sequence

1. **UEFI Firmware** loads BOOTX64.EFI from EFI System Partition (ESP)
2. **UEFI Bootloader** (Rust, uefi-rs 0.26) performs:
   - Initializes UEFI services (console, file I/O)
   - Reads kernel.bin from ESP
   - Gets memory map from UEFI firmware
   - Allocates memory for kernel at 0x100000 (1MB)
   - Exits boot services (point of no return)
   - Jumps to kernel entry point
3. **Rust kernel** initializes:
   - CPU feature detection (already in 64-bit Long Mode)
   - Serial port (COM1)
   - PCI bus scan
   - **Full GPU driver stack** (Driver_Canon GA106 → nv_kernel)

## Data Flow: Kernel → GPU Driver

```
FastOS kernel (_start)
  │
  ├─ PCI scan → finds NVIDIA 0x10DE:0x2504
  │
  ├─ drivers::gpu::rtx3060::init_gpu_driver()
  │   │
  │   └─ nv_kernel::driver_init(&FastOsPlatform)
  │       ├─ nv_hal::find_gpu()       → PCI enumeration
  │       ├─ nv_gpu::gpu_init()       → BAR mapping, chip ID, VRAM
  │       ├─ nv_gpu::enable_engines() → PGRAPH, PFIFO, PCOPY, PDISPLAY
  │       ├─ nv_cmd::fifo_init()      → Command submission engine
  │       ├─ nv_display::display_init() → Display heads
  │       └─ nv_gpu::enable_interrupts()
  │
  └─ GPU ready: VRAM detected, engines enabled, interrupts active
```

## Building

### Prerequisites
- **Rust nightly** — with `rust-src` component
- **UEFI target** — `x86_64-unknown-uefi` (add with `rustup target add`)
- **QEMU with OVMF** — for testing UEFI (optional)

### Complete Build (UEFI Bootloader + Kernel)
```bash
# Windows:
powershell -File build_uefi.ps1

# Or with clean:
powershell -File build_uefi.ps1 -Clean
```

This produces:
- `BOOTX64.EFI` — UEFI bootloader
- `kernel.bin` — Raw kernel binary
- `USB_boot/` — Folder with files ready to copy to ESP

### Kernel only (Rust)
```bash
cd kernel
cargo build --release
```

### UEFI Bootloader only (Rust)
```bash
cd bootloader
cargo build --release
```

### GPU Driver (standalone build)
```bash
cd "Driver_Canon GA106"
cargo build
```

### Test with QEMU (UEFI)
```bash
# Requires OVMF firmware for UEFI
qemu-system-x86_64 \
  -bios /usr/share/OVMF/OVMF_CODE.fd \
  -drive format=raw,file=fat:rw:USB_boot \
  -serial stdio -m 512M
```

## Project Status

| Component | Status |
|-----------|--------|
| UEFI Bootloader (Rust) | ✅ Minimal version compiles |
| UEFI File I/O | 🔲 To implement (kernel.bin loading) |
| UEFI Memory Map | 🔲 To implement |
| UEFI Exit Boot Services | 🔲 To implement |
| Rust kernel entry | ✅ Complete |
| VGA text output | ✅ Complete |
| CPU detection | ✅ Complete |
| PCI bus scan | ✅ Complete |
| FastOsPlatform (nv_hal) | ✅ Complete |
| GPU driver integration | ✅ Connected (nv_kernel stack) |
| nv_regs (register map) | ✅ Complete (from SigDead-BIB) |
| nv_hal (HAL) | ✅ Complete |
| nv_gpu (GPU core) | ✅ Complete |
| nv_cmd (FIFO) | ✅ Complete |
| nv_display (display) | ✅ Complete |
| nv_firmware (FALCON) | ✅ Complete |
| nv_error (error codes) | ✅ Complete |
| exFAT filesystem | 🔲 Structures defined |
| ACPI | 🔲 RSDP search implemented |

## Integration with SigDead-BIB

The `Driver_Canon GA106/` workspace contains 8 Rust crates reconstructed from
NVIDIA's `nvlddmkm.sys` by [SigDead-BIB](../SigDead/). The kernel connects to
this stack via `platform.rs` which implements `nv_hal::Platform` for Ring 0.

## License

MIT
