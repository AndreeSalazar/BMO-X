# FastOS — Ultra Kernel

Faggin-style layered boot for FastOS. Each layer does **one thing**
and jumps to the next. The boot chain has **18 layers** (5 UEFI + 12
bare-metal + kernel Ring 0). No external crates beyond the local
`boot_context`.

## Boot chain (Faggin scale)

```
  UEFI firmware
       │
       ▼
  ┌─────────────────────────────────────────────────────────────────┐
  │  uefi_chain.efi  (one EFI binary, 5 hand-off layers)           │
  │                                                                 │
  │   L0 uefi_enter       entry + serial + ctx skeleton             │
  │   L1 uefi_efi_getmem  GetMemoryMap → ctx.memory_map             │
  │   L2 uefi_efi_getgop  LocateProtocol(GOP) → ctx.fb_*            │
  │   L3 uefi_loader      read *.bin from ESP → phys addrs           │
  │   L4 uefi_exit        ExitBootServices + jmp to s1_serial        │
  └─────────────────────────────────────────────────────────────────┘
       │ (Ring 0 begins)
       ▼
  ┌─────────────────────────────────────────────────────────────────┐
  │  faggin/  (12 bare-metal stages, each ~30-100 lines)          │
  │                                                                 │
  │   s1_serial    0x100000   COM1 init                            │
  │   s2_gdt       0x110000   GDT + TSS + IST stacks                 │
  │   s3_idt       0x120000   256-entry IDT + exception handlers     │
  │   s4_cpuid     0x130000   vendor + brand + features              │
  │   s5_control   0x140000   CR0 + CR4 + XCR0                      │
  │   s6_fpu       0x150000   fninit + MXCSR + xsave initial state   │
  │   s7_tsc       0x160000   TSC calibration → ctx.tsc_freq         │
  │   s8_syscall   0x170000   STAR + LSTAR + FMASK + EFER.SCE       │
  │   s9_paging    0x180000   PML4 + identity-map + higher-half      │
  │   s10_heap     0x190000   bitmap frame allocator + buddy + slab │
  │   s11_acpi     0x1A0000   RSDP scan → ctx.rsdp                  │
  │   s12_devices  0x1B0000   MCFG/HPET/MADT + PCI + APIC + i8042  │
  └─────────────────────────────────────────────────────────────────┘
       │ (jmp to kernel@0x400000)
       ▼
  ┌─────────────────────────────────────────────────────────────────┐
  │  bmo-kernel  (Ring 0 base)                                      │
  │                                                                 │
  │   _start (naked asm)                                            │
  │     └─ BSS zero                                                 │
  │        └─ kernel_main_real(*const BootContext)                  │
  │           └─ phase::main(ctx)                                   │
  │              ├─ phase0_arch:   re-init gdt+idt+cpu (idempotent) │
  │              ├─ phase1_mem:    phys frame allocator             │
  │              ├─ phase2_dev:    fb init, hpet, watchdog          │
  │              └─ phase3_sched:  proc + irq init                  │
  │           └─ splash animation runs as each phase advances       │
  │           └─ hlt loop (single-CPU idle)                         │
  └─────────────────────────────────────────────────────────────────┘
```

## Faggin principle in this tree

In the Z80 of Faggin, each chip did one function, measured in a few
KB, and chained to the next. We replicate that:

- **`s1_serial`** does **only** COM1 init. ~30 lines.
- **`s2_gdt`** does **only** GDT + TSS. ~80 lines.
- **`s3_idt`** does **only** IDT. ~70 lines.
- **`s4_cpuid`** does **only** CPUID detection. ~50 lines.
- **`s5_control`** does **only** CR0/CR4/XCR0. ~40 lines.
- **`s6_fpu`** does **only** FPU init. ~25 lines.
- **`s7_tsc`** does **only** TSC calibration. ~25 lines.
- **`s8_syscall`** does **only** SYSCALL MSRs. ~30 lines.
- **`s9_paging`** does **only** PML4 + maps. ~80 lines.
- **`s10_heap`** does **only** bitmap + buddy + slab. ~50 lines.
- **`s11_acpi`** does **only** RSDP scan. ~50 lines.
- **`s12_devices`** does **only** device init. ~110 lines.

Each is a separate crate that compiles to a ~1-4 KB flat binary
loaded at a fixed physical address. The previous monolithic stages
(stage1_arch, stage2_mm, stage3_dev) were 736 + 673 + 650 = **2059
lines** doing 4-5 things each. The new chain is **~720 lines** total,
each line doing one thing.

## ABI: BootContext

Same struct as the UEFI chain. Each stage writes only its own fields
and jumps to the next. There are no cross-stage function calls —
only `jmp` with `rdi = *const BootContext`.

| Field written by | Field                                          |
|------------------|------------------------------------------------|
| `s1_serial`      | (none)                                          |
| `s2_gdt`         | `gdt_ptr`, `tss_ptr`, `kernel_stack_top`        |
| `s3_idt`         | `idt_ptr`                                       |
| `s4_cpuid`       | (logs only — features read by s5 directly)     |
| `s5_control`     | (CPU control registers)                         |
| `s6_fpu`         | (FPU control)                                   |
| `s7_tsc`         | `tsc_freq`                                      |
| `s8_syscall`     | `syscall_entry`                                 |
| `s9_paging`      | `pml4`                                          |
| `s10_heap`       | `heap_base`, `heap_size`                        |
| `s11_acpi`       | `rsdp`                                          |
| `s12_devices`    | `ioapic_base`, `hpet_base`, `pci_count`, `pci_devices[]` |

## Crates

| Path                  | Target                | Role                          |
|-----------------------|-----------------------|-------------------------------|
| `boot_context/`       | shared lib            | BootContext ABI struct        |
| `uefi_chain/`         | `x86_64-unknown-uefi` | 5 UEFI layers                  |
| `faggin/`            | workspace             | 12 faggin stages + serial    |
| └─ `serial_shared/`   | rlib                  | COM1 helpers (static-linked)  |
| └─ `s1_serial/`       | `x86_64-unknown-none` | COM1 init                      |
| └─ `s2_gdt/`          | `x86_64-unknown-none` | GDT + TSS                      |
| └─ `s3_idt/`          | `x86_64-unknown-none` | IDT                            |
| └─ `s4_cpuid/`        | `x86_64-unknown-none` | CPUID                          |
| └─ `s5_control/`      | `x86_64-unknown-none` | CR0/CR4/XCR0                   |
| └─ `s6_fpu/`          | `x86_64-unknown-none` | FPU init                       |
| └─ `s7_tsc/`          | `x86_64-unknown-none` | TSC calibration                |
| └─ `s8_syscall/`      | `x86_64-unknown-none` | SYSCALL MSRs                   |
| └─ `s9_paging/`       | `x86_64-unknown-none` | PML4 + paging                  |
| └─ `s10_heap/`        | `x86_64-unknown-none` | Frame allocator + heap         |
| └─ `s11_acpi/`        | `x86_64-unknown-none` | RSDP scan                      |
| └─ `s12_devices/`     | `x86_64-unknown-none` | ACPI + PCI + APIC + HPET + i8042 |
| `kernel/`             | `x86_64-unknown-none` | Ring 0 runtime base            |

## Memory map (post-UEFI)

| Range            | Use                                |
|------------------|------------------------------------|
| 0x000000-0x0FFFFF| BIOS / UEFI runtime (untouched)    |
| 0x100000 (1MB)   | s1_serial                          |
| 0x110000         | s2_gdt                             |
| 0x120000         | s3_idt                             |
| 0x130000         | s4_cpuid                           |
| 0x140000         | s5_control                         |
| 0x150000         | s6_fpu                             |
| 0x160000         | s7_tsc                             |
| 0x170000         | s8_syscall                         |
| 0x180000         | s9_paging                          |
| 0x190000         | s10_heap                           |
| 0x1A0000         | s11_acpi                           |
| 0x1B0000         | s12_devices                        |
| 0x400000 (4MB)   | bmo-kernel (Ring 0)                |
| 0x500000+        | heap, PML4, page tables, frames    |

Higher-half mirror: `0xFFFF_8000_0000_0000 + phys`.

## Build

```powershell
.\build.ps1              # Compile only
.\build.ps1 -BuildOnly   # Don't ask about flashing
.\build.ps1 -Flash       # Compile + flash to SSD
.\build.ps1 -Flash -Drive D -Yes  # Flash to D: without prompt
.\build.ps1 -Clean       # Clean target/ and staging/
```

Output staged at `staging\EFI\BOOT\`:

- `BOOTX64.EFI`    — the 5-layer UEFI chain
- `s1_serial.bin` … `s12_devices.bin` — the 12 faggin stages
- `kernel.bin`      — the Ring 0 kernel

## What was migrated / removed

| Path              | Status |
|-------------------|--------|
| `kernel/` (legacy)| ❌ removed |
| `Kernel_Upgrade/` (legacy ref) | ❌ removed |
| `stage1_arch/` (monolithic 736 L) | ❌ replaced by s2_gdt..s8_syscall |
| `stage2_mm/` (monolithic 673 L) | ❌ replaced by s9_paging..s10_heap |
| `stage3_dev/` (monolithic 650 L) | ❌ replaced by s11_acpi..s12_devices |
| `target/`, `target_build/` | ❌ removed (2.8 GB) |
| `Ultra_userspace/` (sibling) | ✅ kept, unchanged |

## Related

- `..\Ultra_userspace\` — Ring 3 stub workspace (apps, services, drivers).
- The full migration history is in git log.
