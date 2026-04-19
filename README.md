# FastOS

> **Minimal custom OS for AMD Ryzen 5 5600X + NVIDIA RTX 3060 12G**
> Two-language architecture: NASM (boot) → Rust (kernel + drivers)
> **Fagging-Scale: Everything Ring 0 — no userspace, no abstractions.**

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
│  NASM Bootloader (Ring 0)                                        │
│  ├── stage1.asm    │ 512-byte MBR boot sector                    │
│  ├── stage2.asm    │ 16-bit → 32-bit → 64-bit transition         │
│  ├── gdt.asm       │ GDT for protected + long mode               │
│  ├── paging.asm    │ 4-level page tables, 4GB identity map       │
│  ├── cpucheck.asm  │ CPUID verification (Long Mode, etc.)        │
│  └── sse_avx.asm   │ SSE4.2 + AVX2 + FMA3 initialization         │
├──────────────────────────────────────────────────────────────────┤
│  Hardware                                                        │
│  CPU: AMD Ryzen 5 5600X (Zen 3, 6C/12T, AVX2, AES-NI)            │
│  GPU: NVIDIA RTX 3060 12GB (GA106, Ampere)                       │
│  RAM: DDR4                                                       │
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

1. **BIOS** loads Stage 1 (MBR) at `0x7C00`
2. **Stage 1** loads Stage 2 from disk to `0x7E00`
3. **Stage 2** performs:
   - CPU capability checks (CPUID, Long Mode)
   - A20 line enable
   - E820 memory detection
   - Loads Rust kernel to `0x10000`, copies to `0x100000` (1MB)
   - Enters 32-bit Protected Mode (GDT)
   - Sets up 4-level paging (identity maps 4GB with 2MB pages)
   - Enters 64-bit Long Mode
   - Initializes SSE + AVX + AVX2
   - Jumps to Rust kernel with boot info in RDI
4. **Rust kernel** initializes:
   - VGA text output
   - CPU feature detection
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
- **NASM** — Netwide Assembler
- **Rust nightly** — with `rust-src` component
- **QEMU** — for testing (optional)

### Boot only (NASM)
```bash
cd boot
# Windows:
powershell -File build.ps1
```

### Kernel (Rust)
```bash
cd kernel
cargo build
```

### GPU Driver (standalone build)
```bash
cd "Driver_Canon GA106"
cargo build
```

### Test with QEMU
```bash
qemu-system-x86_64 -drive format=raw,file=boot/fastos.img -serial stdio -m 512M
```

## Project Status

| Component | Status |
|-----------|--------|
| Stage 1 (MBR) | ✅ Complete |
| Stage 2 (16→32→64) | ✅ Complete |
| GDT/Paging/AVX2 | ✅ Complete |
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
