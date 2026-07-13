# Ultra_kernel_x86-64

**The x86-64 port of the FastOS/BMO kernel.**

This directory is the **CPU-specific** kernel tree for the `x86_64` (AMD64 / Intel 64) architecture.
Everything in this tree — the bootloader, the 12 faggin pre-kernel stages, the Ring 0 base, and
the userland workspace that hangs off it — is built around the **System V AMD64** calling
convention, the **x86-64 segmented memory model** (with paging), and the **UEFI x86-64 boot
protocol**.

For other CPU architectures, the entire tree is duplicated as `Ultra_kernel_<arch>/`
(planned: `Ultra_kernel_aarch64/`, `Ultra_kernel_riscv64/`).

## Why the architecture-specific suffix?

`Uso_Reales_Crates/` (the shared `abi/`, `shared/`, `drivers/`, `services/` tree) is
**CPU-agnostic by design**:

- `bmo-abi` defines the BEF (BMO Executable Format) header, syscall IDs, and type layouts
  that are pure data and can be interpreted on any CPU.
- `bmo-channel` uses only `core::sync::atomic`, which the Rust standard library implements
  per-target.
- `cabina-core`, `byte-defender`, `timeback`, `hw-profile` are pure data + policy code
  that never touches registers or instructions.
- `bmo-hal` is the **HAL pattern** — it defines `HalServices` as a vtable of function
  pointers (init, serial, framebuffer, MMU, GDT, syscall, …) so that any port fills in
  the implementation for its CPU.

The kernel tree (`Ultra_kernel_x86-64/`), by contrast, is **entirely CPU-specific**:

- `uefi_chain/` — 5 UEFI bootloader layers (EFI target, x86-64 only).
- `faggin/` — 12 pre-kernel stages. Each one contains x86-64 bare-metal inline asm
  (`lgdt`, `lidt`, `ltr`, `cpuid`, `wrmsr`, `rdmsr`, `fninit`, `xsave`).
- `kernel/` — the Ring 0 base, also x86-64 asm (`_start` naked function, CR3 load).
- `Ultra_userspace/` — Ring 3 side; depends on `boot_context` which currently exposes
  x86-64 fields (`ioapic_base`, `hpet_base`, `tsc_freq`, `cr3`).

The `_x86-64` suffix makes this contract explicit in the directory name itself: anything
under this tree is **prepared specifically for x86-64**, and the moment a future
`Ultra_kernel_aarch64/` shows up, the porting surface is self-documenting.

## How a port to ARM (AArch64) would work

Each per-CPU tree contains the same five top-level pieces, but with arch-specific
content. The shared contract is `BootContext` (`boot_context/`) and the wider
`Uso_Reales_Crates/` ABI.

| Piece | x86-64 (this tree) | AArch64 (port plan) |
|-------|-------------------|---------------------|
| `uefi_chain/` | 5 UEFI layers for x86-64 | 5 UEFI layers for AArch64 (different `EFI_BOOT_SERVICES` table layout) |
| `faggin/s2_gdt` | GDT + TSS + IST stacks | GICv3 distributor + redistributor setup |
| `faggin/s3_idt` | 256-entry IDT (`extern "x86-interrupt"`) | Vector table at `VBAR_EL1` |
| `faggin/s4_cpuid` | `cpuid` leaves 0, 1, 0x80000002-04 | `MIDR_EL1`, `MPIDR_EL1` |
| `faggin/s5_control` | `CR0` / `CR4` / `XCR0` | `SCTLR_EL1`, `CPACR_EL1` |
| `faggin/s6_fpu` | `fninit` + `ldmxcsr` + `xsave` | `FPCR` / `FPSR` init |
| `faggin/s7_tsc` | TSC via `rdtsc` | `CNTVCT_EL0` |
| `faggin/s8_syscall` | `STAR` / `LSTAR` / `FMASK` MSRs | `SVC #0` instruction |
| `faggin/s9_paging` | PML4 / PDPT / PD / PT, 4-level page tables | `TTBR0_EL1` + `MAIR_EL1` + `TCR_EL1` (4 KB granule, 48-bit VA) |
| `faggin/s11_acpi` | RSDP scan via `EBDA` / `ROM` | Same ACPI tables, but MADT describes GIC, not IOAPIC |
| `faggin/s12_devices` | PCI ECAM + IOAPIC + HPET + i8042 | PCI ECAM (universal) + GICv3 + HPET + UART (no i8042) |
| `kernel/_start` | Naked asm `mov cr3, ...` | `msr ttbr0_el1, ...` |
| `boot_context` | x86-64 fields (`ioapic_base`, `tsc_freq`, `cr3`) | aarch64 fields (`gic_dist_base`, `cntfrq_el0`, `ttbr0_el1`) |

**What does NOT change in a port:**

- `Uso_Reales_Crates/` (entire tree: `abi/`, `shared/`, `drivers/`, `services/`)
  — by design CPU-agnostic.
- The `bmo-rt` userspace runtime (one 2-line asm change: `syscall` → `svc #0`).
- The BEF format and its loader.
- The C / C++ / COBOL frontends at the parser level (only the AOT codegen backend
  needs a new variant: `AotAArch64` next to `AotX86_64`).

The drivers in `Uso_Reales_Crates/drivers/` (xhci, ahci, nvme, fat32, net, audio,
input, uhid) **are** x86-64-specific in their current form because they use PCI
MMIO patterns and `volatile` reads. For a port, they would either:

1. Be reused as-is if the arch provides PCIe (AArch64 servers do), or
2. Be replaced with arch-specific versions that go through the HAL function pointers.

## Layout of this tree

```
Ultra_kernel_x86-64/
├── Cargo.toml              # this tree's workspace root
├── README.md               # this file
├── build.ps1               # one-shot build + objcopy + flash script
├── validate_chain.ps1      # post-build chain order / size validator
├── boot_context/           # CPU-agnostic BootContext struct (lib, no_std)
├── uefi_chain/             # 5-layer UEFI bootloader (UEFI target)
├── faggin/                 # 12-stage Faggin-style pre-kernel chain
│   ├── serial_shared/      # COM1 helpers used by every stage
│   ├── s1_serial/          # COM1 init at 115200 8N1
│   ├── s2_gdt/             # GDT + TSS + IST stacks
│   ├── s3_idt/             # 256-entry IDT
│   ├── s4_cpuid/           # CPUID vendor + brand
│   ├── s5_control/         # CR0 + CR4 + XCR0
│   ├── s6_fpu/             # fninit + MXCSR + xsave
│   ├── s7_tsc/             # TSC calibration
│   ├── s8_syscall/         # STAR + LSTAR + FMASK
│   ├── s9_paging/          # 4-level page tables + higher-half
│   ├── s10_heap/           # bitmap frame allocator
│   ├── s11_acpi/           # RSDP scan
│   └── s12_devices/        # ACPI + PCI + APIC + HPET + i8042
├── kernel/                 # Ring 0 base (single .bin, loaded at 0x400000)
│   ├── Cargo.toml
│   ├── linker.ld           # kernel linker script (loads at 0x400000)
│   ├── .cargo/config.toml
│   └── src/ring0/
│       ├── core/           # entry, phase, splash
│       ├── cpu/            # CPUID detection (vendor, family, brand, features)
│       └── dev/            # console, framebuffer
├── Ultra_userspace/        # Ring 3 side (sibling workspace, also x86-64)
└── target/                 # cargo target dir (one per cargo workspace)
    └── staging/EFI/BOOT/   # final 14 files: BOOTX64.EFI + 12 .bin + kernel.bin
```

## Boot flow

```
Power-on
   │
   ▼
UEFI firmware (on this machine: Kingston SA400S37120GB SSD, partition "FASTOS-EFI")
   │
   ▼  /EFI/BOOT/BOOTX64.EFI  (uefi_chain, 5 layers)
   │
   ▼  layer3_load reads s1..s12 .bin + kernel.bin from ESP,
   │            places each at its fixed physical address
   │            (s1=0x100000, s2=0x110000, …, s12=0x1B0000, kernel=0x400000)
   │            fills BootContext, jumps to s1
   ▼
s1_serial  (0x100000)  ──►  s2_gdt  (0x110000)  ──►  s3_idt  (0x120000)
                                                  ──►  s4_cpuid  (0x130000)
                                                  ──►  s5_control  (0x140000)
                                                  ──►  s6_fpu  (0x150000)
                                                  ──►  s7_tsc  (0x160000)
                                                  ──►  s8_syscall  (0x170000)
                                                  ──►  s9_paging  (0x180000)
                                                  ──►  s10_heap  (0x190000)
                                                  ──►  s11_acpi  (0x1A0000)
                                                  ──►  s12_devices  (0x1B0000)
                                                  ──►  kernel  (0x400000)
   │
   ▼
bmo-kernel (Ring 0 base)  — splash animation, framebuffer init, serial shell
```

## Build

From this directory:

```powershell
# Build everything (uefi_chain + 12 faggin stages + kernel)
.\build.ps1

# Clean first
.\build.ps1 -Clean

# Build only (skip flashing)
.\build.ps1 -BuildOnly

# Build + flash to SSD S: (will prompt unless -Yes)
.\build.ps1 -Flash -Drive S -Yes

# Validate chain order and sizes
.\validate_chain.ps1
```

### Optimization

- **faggin stages**: `opt-level = "z"` + LTO + `codegen-units = 1` + `strip = "symbols"`.
  Each stage is 4-5 KB raw because the stages are single-purpose and the linker drops
  anything not used.
- **kernel**: `opt-level = 3` because the kernel hosts the splash animation loop, the
  framebuffer blit, and the serial shell — all code that runs forever and benefits
  from speed over size.

### Cargo workspaces

There are **three nested workspaces** at play here:

1. `C:\Users\andre\Documents\FastOS\Cargo.toml` — the **top-level** FastOS workspace,
   listing `Ultra_kernel_x86-64/{boot_context,kernel,uefi_chain,faggin,Ultra_userspace}`
   plus all of `Uso_Reales_Crates/`.
2. `Ultra_kernel_x86-64/Cargo.toml` — the **kernel sub-workspace**, listing
   `boot_context/` and `kernel/`. faggin stages are NOT members (they each have their
   own `[workspace]` stub to keep them isolated).
3. `Ultra_kernel_x86-64/uefi_chain/Cargo.toml` — the **UEFI sub-workspace**, listing
   just `uefi-chain/`. Separate from the kernel sub-workspace because it targets
   `x86_64-unknown-uefi`, which would otherwise confuse the bare-metal linker.

## Specs

- **Target CPU:** x86_64 (AMD64 / Intel 64), baseline = x86-64-v2 (SSE4.2 + POPCNT).
- **Tested hardware:** AMD Ryzen 5 5600X (Vermeer, Zen 3, Family 19h, Model 0x01),
  6 cores / 12 threads.
- **Boot device:** UEFI 2.x, ESP at `\EFI\BOOT\`.
- **Build host:** Windows + `cargo +nightly` + `rust-lld` + `llvm-objcopy`.

## Status (as of 2026-07-13)

- 17 of 20 crates in `Uso_Reales_Crates/` build clean.
- 12 of 12 faggin stages build clean (4-5 KB each).
- uefi_chain builds clean (19.5 KB).
- bmo-kernel builds clean (~31 KB raw, includes CPUID detection log).
- All 14 files (BOOTX64.EFI + 12 .bin + kernel.bin) successfully flashed to `S:\EFI\BOOT\`.

The boot chain is functional up to and including the kernel splash + serial shell.
The serial shell accepts: `help`, `info`, `fb`, `splash`, `panic`, `reboot`, `halt`.
CPUID detection prints the Ryzen 5 5600X brand string at boot.
