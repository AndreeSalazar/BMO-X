# Faggin — 12 single-purpose boot stages

> Federico Faggin (1941–) diseñó el Zilog Z80 en 1974. Su principio:
> cada chip hace **una sola cosa**, mide pocos transistores, y se
> conecta al siguiente por una interfaz mínima.

We apply the same principle to the boot chain. Each `s*_*.rs` is a
**single-purpose** stage that:

- Is ~30-100 lines of Rust.
- Compiles to a 1-4 KB flat binary.
- Loads at a fixed physical address.
- Writes only its own fields to `BootContext`.
- Jumps to the next stage.

## Chain order

| # | Stage        | Address     | Does                          | Writes to `BootContext`        |
|---|--------------|-------------|-------------------------------|--------------------------------|
| 1 | `s1_serial`  | 0x100000    | COM1 init                    | (none)                          |
| 2 | `s2_gdt`     | 0x110000    | GDT + TSS + IST stacks       | `gdt_ptr`, `tss_ptr`, `kernel_stack_top` |
| 3 | `s3_idt`     | 0x120000    | 256-entry IDT                | `idt_ptr`                       |
| 4 | `s4_cpuid`   | 0x130000    | vendor + brand + features    | (none — read by s5 directly)    |
| 5 | `s5_control` | 0x140000    | CR0 + CR4 + XCR0             | (CPU registers)                 |
| 6 | `s6_fpu`     | 0x150000    | fninit + MXCSR + xsave       | (FPU state)                     |
| 7 | `s7_tsc`     | 0x160000    | TSC calibration              | `tsc_freq`                      |
| 8 | `s8_syscall` | 0x170000    | STAR + LSTAR + FMASK         | `syscall_entry`                 |
| 9 | `s9_paging`  | 0x180000    | PML4 + identity + higher-half| `pml4`                          |
| 10 | `s10_heap`  | 0x190000    | bitmap + buddy + slab        | `heap_base`, `heap_size`        |
| 11 | `s11_acpi`  | 0x1A0000    | RSDP scan                    | `rsdp`                          |
| 12 | `s12_devices`| 0x1B0000    | ACPI + PCI + APIC + HPET + i8042 | `ioapic_base`, `hpet_base`, `pci_count`, `pci_devices[]` |

After `s12_devices`, the chain `jmp`s to `kernel@0x400000` (the
Ring 0 base), which is built separately.

## Build

Each stage compiles with `opt-level = "z"` (size), `lto = true`,
`codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`. The
`Cargo.toml` of each stage inherits the workspace default profile
and applies a per-package override for size.

From the Ultra_kernel/ root:

```powershell
# Build everything (uefi_chain, all 12 faggin stages, kernel)
.\build.ps1

# Clean first
.\build.ps1 -Clean

# Just compile, don't flash
.\build.ps1 -BuildOnly

# Validate that every stage is < 4 KB and in the right order
.\validate_chain.ps1
```

## How a stage declares its next

Every stage's `src/main.rs` ends with:

```rust
unsafe {
    asm!(
        "jmp {next}",
        next = in(reg) s_next as *const () as u64,
        in("rdi") ctx_ptr,
        options(noreturn)
    );
}
```

The `s_next` symbol is resolved at link time within the same binary
(since `serial_shared` is statically linked into every stage). Each
stage's `Cargo.toml` declares `boot-context` and `serial-shared` as
path dependencies.

## Adding a new stage

1. Create `s<N>_<name>/` with the same structure as the others
   (`Cargo.toml`, `linker.ld`, `.cargo/config.toml`, `src/main.rs`).
2. In the new `Cargo.toml`, use a unique package name (e.g.
   `s13-somefeature`) and a path dep on `serial-shared` and
   `boot-context`.
3. Set `. = 0xNN0000;` in `linker.ld` (the next available 64 KB page).
4. In the new `src/main.rs`, expose a `pub extern "C" fn l<N>_entry`
   that does the work, fills the relevant `BootContext` field, and
   `jmp`s to the next stage's entry symbol.
5. Add the package name to `[profile.release.package."..."]` in this
   workspace's `Cargo.toml` with `opt-level = "z"`.
6. Add the stage to the `$stages` array in `..\build.ps1` and to the
   `expected` array in `..\validate_chain.ps1`.
7. Update `..\README.md` with the new stage row.

## Per-package `opt-level = "z"` explained

The faggin principle is "do one thing, do it small". The compiler
flag `opt-level = "z"` optimizes for binary size at the cost of
speed — perfect for a 30-line boot stage. With LTO + 1 codegen
unit + `panic = "abort"` + `strip = "symbols"`, a typical faggin
stage lands at 1-3 KB flat binary after `llvm-objcopy -O binary`.

Compare to `opt-level = 3` (default for `[profile.release]`), which
optimizes for speed and produces binaries 2-3x larger.

## Why the order matters

Each stage assumes a specific CPU state set by previous stages:

- `s1_serial` — needs nothing.
- `s2_gdt` — needs nothing.
- `s3_idt` — needs GDT (s2).
- `s4_cpuid` — needs working IDT for fault handlers.
- `s5_control` — needs CPUID to know which CR4 bits are supported.
- `s6_fpu` — needs CR4.OSFXSR (s5) and AVX detection.
- `s7_tsc` — needs nothing (reads CPUID directly).
- `s8_syscall` — needs GDT selectors (s2) for STAR.
- `s9_paging` — needs frame allocator (s10 — but s9 has its own local
  bitmap of 64 frames for PTE pages).
- `s10_heap` — needs paging off (so it can use identity-mapped
  physical addresses).
- `s11_acpi` — needs paging on (s9), needs memory map (from
  BootContext filled by UEFI chain).
- `s12_devices` — needs RSDP (s11), needs APIC access (no paging
  yet for MMIO since identity map covers 0-2GB).
- `kernel@0x400000` — needs everything.

The `validate_chain.ps1` script verifies that each stage's `jmp`
target matches the expected next stage.
