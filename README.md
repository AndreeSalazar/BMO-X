# FastOS

> **Minimal custom OS for AMD Ryzen 5 5600X + NVIDIA RTX 3060 12G**
> Three-language architecture: NASM (boot) → Rust (kernel) → C (upper layers)

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        FastOS Stack                              │
├──────────────────────────────────────────────────────────────────┤
│  ADead-BIB (C)          │ Upper layers, userspace                │
│  └── Future             │ Builds on stable kernel                │
├──────────────────────────────────────────────────────────────────┤
│  Rust Kernel (#![no_std])                                        │
│  ├── arch/              │ Ryzen 5 5600X: CPUID, paging, ACPI     │
│  ├── drivers/           │ PCI, GPU (RTX 3060), storage           │
│  ├── fs/                │ exFAT filesystem                       │
│  └── main.rs            │ Kernel entry point                     │
├──────────────────────────────────────────────────────────────────┤
│  NASM Bootloader (Ring 0)                                        │
│  ├── stage1.asm         │ 512-byte MBR boot sector               │
│  ├── stage2.asm         │ 16-bit → 32-bit → 64-bit               │
│  ├── gdt.asm            │ GDT for protected + long mode          │
│  ├── paging.asm         │ 4-level page tables, 4GB identity map  │
│  ├── cpucheck.asm       │ CPUID verification                     │
│  └── sse_avx.asm        │ SSE4.2 + AVX2 + FMA3 initialization    │
├──────────────────────────────────────────────────────────────────┤
│  Hardware                                                        │
│  CPU: AMD Ryzen 5 5600X (Zen 3, 6C/12T, AVX2, AES-NI)            │
│  GPU: NVIDIA RTX 3060 12GB (GA106, Ampere)                       │
│  RAM: DDR4                                                       │
└──────────────────────────────────────────────────────────────────┘
```

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
4. **Rust kernel** initializes drivers, detects hardware

## Building

### Prerequisites
- **NASM** — Netwide Assembler
- **Rust nightly** — with `rust-src` component
- **QEMU** — for testing (optional)

### Boot only (NASM)
```bash
cd boot
# Linux/macOS:
make
# Windows:
powershell -File build.ps1
```

### Kernel (Rust)
```bash
cd kernel
cargo build
```

### Test with QEMU
```bash
qemu-system-x86_64 -drive format=raw,file=boot/fastos.img -serial stdio
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
| GPU driver | 🔲 Stub (SigDead integration pending) |
| exFAT filesystem | 🔲 Structures defined |
| ACPI | 🔲 RSDP search implemented |
| ADead-BIB (C) | 🔲 Stub prepared |

## Integration with SigDead-BIB

The GPU driver (`kernel/src/drivers/gpu/rtx3060.rs`) uses register definitions
from the [SigDead-BIB](../SigDead/) project's `Driver_Canon GA106` workspace.
Once the kernel has MMIO mapping, the full `nv_gpu` initialization sequence
(BAR mapping → chip ID → engine enable → firmware load) will be integrated.

## License

MIT
