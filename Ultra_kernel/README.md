# FastOS — Ultra Kernel (Ring 0 base)

Faggin-style layered boot for FastOS. Each layer does one thing, then
jumps to the next. **No external crates** beyond `boot_context` (the
shared ABI struct). Pure UEFI, pure Ring 0, single-CPU.

## Boot chain

```
  UEFI firmware
       │
       ▼
  ┌─────────────────────────────────────────────────────────────┐
  │  uefi_chain.efi  (one EFI binary, 5 hand-off layers)       │
  │                                                             │
  │   L0 uefi_enter       entry + serial + ctx skeleton         │
  │   L1 uefi_efi_getmem  GetMemoryMap → ctx.memory_map         │
  │   L2 uefi_efi_getgop  LocateProtocol(GOP) → ctx.fb_*        │
  │   L3 uefi_loader      read stage*.bin from ESP → phys addrs │
  │   L4 uefi_exit        ExitBootServices + jmp to stage1      │
  └─────────────────────────────────────────────────────────────┘
       │ (Ring 0 begins)
       ▼
  stage1_arch    ── GDT, IDT, SYSCALL, CPU, FPU, TSC
       │ jmp
       ▼
  stage2_mm      ── PML4, allocators
       │ jmp
       ▼
  stage3_dev     ── ACPI, PCI, APIC, HPET
       │ jmp
       ▼
  ┌─────────────────────────────────────────────────────────────┐
  │  bmo-kernel  (this crate) — Ring 0 runtime base             │
  │                                                             │
  │   _start (naked asm)                                        │
  │     └─ BSS zero                                             │
  │        └─ kernel_main_real(*const BootContext)              │
  │           └─ phase::main(ctx)                               │
  │              ├─ phase0_arch:   gdt+idt+syscall+cpu::init    │
  │              ├─ phase1_mem:    phys frame allocator         │
  │              ├─ phase2_dev:    fb init, hpet, watchdog     │
  │              └─ phase3_sched:  proc + irq init              │
  │           └─ splash animation runs as each phase advances   │
  │           └─ hlt loop (single-CPU idle)                     │
  └─────────────────────────────────────────────────────────────┘
```

## Crates

| Crate           | Target                | What it does                                   |
|-----------------|-----------------------|------------------------------------------------|
| `boot_context`  | shared lib            | `BootContext` — the ABI struct between layers  |
| `uefi_chain`    | `x86_64-unknown-uefi` | The 5 UEFI layers, linked into one `.efi`      |
| `stage1_arch`   | `x86_64-unknown-none` | GDT, IDT, SYSCALL MSRs, CPU features, FPU, TSC |
| `stage2_mm`     | `x86_64-unknown-none` | PML4, frame/buddy/slab allocators, ACPI RSDP    |
| `stage3_dev`    | `x86_64-unknown-none` | ACPI parsing, PCI enum, APIC, HPET, i8042       |
| `kernel`        | `x86_64-unknown-none` | Ring 0 runtime base — splash, phases, hardware  |

## The kernel (`bmo-kernel`)

```
kernel/
├── Cargo.toml                ← only deps: boot-context (local)
├── linker.ld                 ← .text @ 0x400000, .bss tracked
├── .cargo/config.toml        ← -T kernel/linker.ld
└── src/
    ├── main.rs               ← #![no_std], #![no_main]
    ├── info.rs               ← FB_ADDR/WIDTH/HEIGHT/STRIDE/PIXEL_FORMAT
    └── ring0/
        ├── mod.rs            ← module root
        ├── core/
        │   ├── entry.rs      ← _start (naked asm) + kernel_main_real
        │   ├── phase.rs      ← phase0..3 orchestrator
        │   └── splash.rs     ← animated logo + smooth progress bar
        ├── arch/
        │   ├── gdt.rs, idt.rs, ctx.rs, tlb.rs, apic.rs
        │   ├── syscall.rs    ← syscall dispatch (bmo_channel removed)
        │   ├── context.rs    ← BootContext DI wrapper
        │   └── smp/          ← stubs (multi-core deferred)
        ├── cpu/
        │   ├── tsc, msr, regs, fpu, features, cache, info
        │   └── vendor_shim.rs ← local stand-in for cpu_vendor_profile
        ├── dev/
        │   ├── console, framebuffer, timer, watchdog
        │   ├── pc_speaker, hpet, timestamp, timer_wheel
        │   ├── power.rs, audio.rs (stubs)
        │   └── acpi.rs, pcie.rs (stubs)
        ├── mm/
        │   ├── types.rs      ← local MemoryEntry
        │   ├── phys.rs       ← bitmap frame allocator
        │   ├── slab, vdso    ← copied from legacy
        │   ├── frame_alloc, vmm_stub, heap_stub, buddy_stub
        ├── proc/             ← minimal scheduler (single task)
        ├── irq/              ← mod + lapic + ioapic + apic_mmio
        ├── boot/             ← serial, log
        └── channel.rs        ← stub
```

## Splash animation

The boot splash is preserved from the legacy kernel (verbatim where
possible). It runs at every phase transition:

1. **Animated logo** (5 phases, inside-out expansion)
   - r=0..=4: core dot cyan
   - r=8..=14: inner ring indigo
   - r=16..=22: mid ring deep indigo
   - r=24..=30: outer ring slate
   - r=34: glow accent
2. **Title fade-in** "BMO v2.0" + subtitle "Pure Ring 0"
3. **Smooth progress bar** with adaptive pixel-level interpolation
4. **Phase label** under the bar (e.g. "CPU, GDT, IDT...")

If the UEFI chain reports `ctx.fb_addr == 0` (headless firmware),
the splash is skipped silently and the kernel continues.

## BuildContext → ring0 mapping

| BootContext field      | Consumed in                              |
|------------------------|------------------------------------------|
| `magic`, `version`     | `core::entry` (validation)               |
| `memory_map[]`         | `mm::phys::init` (frame allocator)       |
| `fb_addr, fb_*`        | `info::init_from` → `dev::framebuffer`   |
| `tsc_freq`             | (already calibrated by `stage1_arch`)   |
| `gdt_ptr, idt_ptr`     | (already set up by `stage1_arch`)        |
| `rsdp`                 | `dev::acpi::parse_mcfg` (stub returns None) |
| `pci_devices[]`        | (already filled by `stage3_dev`)         |
| `ioapic_base, hpet_base` | (already set by `stage3_dev`)          |

## Boot phases

| Phase  | Modules touched                                          | Splash %  |
|--------|----------------------------------------------------------|-----------|
| 0 arch | `arch::gdt`, `arch::idt`, `arch::syscall`, `cpu::init`  | 15→35     |
| 1 mem  | `mm::phys::init`, `mm::vmm_stub`, `mm::heap_stub`       | 35→55     |
| 2 dev  | `dev::framebuffer::init_gop`, `dev::timer`, `dev::watchdog` | 55→80 |
| 3 sched| `proc::init`, `irq::init`                                | 80→100    |

## Deferred (stubs only — not in the Ring 0 base)

- SMP multi-core (`arch::smp/*` returns online=1)
- ACPI/PCI parsing (real work is in `stage3_dev` already)
- Audio mixer
- BMO Channel (lock-free IPC)
- AHCI / USB / network / FS
- Real paging (`mm::vmm_stub`)
- Real heap allocator (`mm::heap_stub`)
- Real buddy allocator (`mm::buddy_stub`)

## Build

```powershell
cd C:\Users\andre\Documents\FastOS\Ultra_kernel
.\build.ps1 -BuildOnly
```

## What was migrated from `FastOS\kernel\` (legacy)

| Source                                  | Destination in `Ultra_kernel\kernel\` | Status |
|-----------------------------------------|---------------------------------------|--------|
| `core/splash.rs` (409 L)                | `ring0/core/splash.rs`                | verbatim, paths adjusted |
| `core/entry.rs` (96 L)                  | `ring0/core/entry.rs`                 | rewritten to take `*const BootContext` |
| `core/phase.rs` (308 L)                 | `ring0/core/phase.rs`                 | rewritten, vendor calls removed |
| `cpu/{tsc,msr,regs,fpu,features,cache,info,mod}.rs` (~900 L) | `ring0/cpu/*` | paths + `cpu_vendor_profile` shim |
| `dev/{console,framebuffer,timer,watchdog,pc_speaker,hpet,timestamp,timer_wheel}.rs` (~900 L) | `ring0/dev/*` | paths, `PixelFormat` made local |
| `arch/{gdt,idt,ctx,tlb,context,apic,syscall}.rs` (~1800 L) | `ring0/arch/*` | paths, channel syscalls removed |
| `mm/{slab,vdso}.rs` (~520 L)            | `ring0/mm/{slab,vdso}.rs`             | paths |
| `mm/{mod,frame_alloc,phys,buddy,vmm}.rs` | `ring0/mm/{types,phys,frame_alloc,vmm_stub,heap_stub,buddy_stub}` | rewritten to use `BootContext::MemoryEntry` |
| `proc/{mod,process,task}.rs`            | `ring0/proc/*`                        | rewritten minimal scheduler |
| `irq/{mod,lapic,ioapic,apic_mmio}.rs` (~430 L) | `ring0/irq/*`                  | paths |
| `boot/{serial,log}.rs`                  | `ring0/boot/*`                        | paths |
| `cpu_vendor_profile::*`                 | `ring0/cpu/vendor_shim.rs`            | local shim, no external crate |
| `bmo_channel::*` (syscall 0x34-0x36)    | (removed)                              | reserved as `stub` in SYSCALL_TABLE |
| `bmo_hal::HalServices` (227 L)          | (removed)                              | direct function calls instead |
| `integration.rs` (140 L)                | (removed)                              | unused in legacy anyway |
| `ipc_channel.rs` (350 L)                | `ring0/channel.rs` stub                | |
| `dev/{acpi,pcie,ahci,fs,storage,hda}.rs` | `ring0/dev/{acpi,pcie,power,audio}.rs` stubs | |
| `arch/smp/*` (6 archivos, ~1100 L)      | `ring0/arch/smp/*` stubs               | SMP deferred |
| `ring3/*` (5 archivos)                  | (removed)                              | UEFI pure, no userland loader |

## What was NOT migrated

- `ring3/*`: the legacy Ring 3 ELF loader. Ultra_kernel is UEFI-pure and
  doesn't have a `.bmo` module format yet.
- `bmo_hal::HalServices` (227-line function table): we use direct
  function calls instead. A `HalServices` table will be re-introduced
  when there are real userspace services to expose.
- `integration.rs`: dead code in the legacy (0 callers).
- `ipc_channel.rs` (350 L): the lock-free IPC implementation.
  Replaced by a stub; will be replaced by a different IPC mechanism
  (probably ports or hypercalls) when needed.
- All `bmo_*` external crates (`bmo_channel`, `bmo_hal`,
  `bmo_boot_protocol`, `cpu_vendor_profile`, `nvram_log`, `llfree`).

## Related

- `..\Kernel_Upgrade\`: previous monolithic `nano_wake` reference.
- `..\kernel\`: source of the migrated Ring 0 code (legacy).
