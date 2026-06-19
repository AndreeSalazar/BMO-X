# FastOS Workspace — Folder Overview

> This file clarifies the role of each top-level folder to prevent
> confusion between similarly-named crates.

## Production code (built by `build_uefi.ps1`)

| Folder             | What it is                                            | Status |
| ------------------ | ----------------------------------------------------- | ------ |
| `bootloader/`      | UEFI bootloader (loads kernel.elf)                    | Built every flash |
| `boot_protocol/`   | Shared `BootInfo` struct (bootloader <-> kernel)      | Built as dep |
| `kernel/`          | The actual OS kernel                                  | Built every flash |
| `bmo_usb/`         | USB drivers library (xHCI, HID, audio)                | Built as dep |
| `bmofs/`           | BMO-FS CLI + lib for creating the USB ramdisk image   | Built every flash |

## In-kernel submodules (inside `kernel/src/`)

| Folder                       | What it is                                       |
| ---------------------------- | ------------------------------------------------ |
| `kernel/src/boot/`           | Boot phase orchestration (Phase 0-5)             |
| `kernel/src/arch/`           | x86_64 architecture (GDT, IDT, paging, ACPI)     |
| `kernel/src/drivers/`        | Hardware drivers (GOP, PCI, NVMe, USB, etc.)     |
| `kernel/src/sched/`          | Thread + process scheduler                       |
| `kernel/src/alloc/`          | Heap allocator                                   |
| `kernel/src/bef/`            | Binary EXchange Format devourer (PE/ELF loader)  |
| `kernel/src/barex/`          | BareX compat layer (FAKE_DLLS, shim, shader)     |
| `kernel/src/bmo_abi/`        | Application Binary Interface (Ring 3 API)        |
| `kernel/src/lang/`           | Languages (BMOasm, NEXO, C, C++, Java, Python)   |
| `kernel/src/lang/nexo/`      | **NEXO language compiler** (in-kernel)           |
| `kernel/src/desktop/`        | Welcome screen, render, input, commands          |
| `kernel/src/diag/`           | Diagnostics (overlay, telemetry, fault log)      |
| `kernel/src/security/`       | Security subsystem (ByteDefender, Restaurer)     |

## Future userland (NOT built into the kernel image)

| Folder            | What it is                                           | When used |
| ----------------- | ---------------------------------------------------- | --------- |
| `nexo_ring3/`     | BSF shader loader for **Ring 3** userland            | When Ring 3 process loading lands |
| `nexo-sh-tool/`   | Host-side CLI: HLSL/GLSL/WGSL -> BSF (uses naga)     | When shaders are authored |

## Name collision warning

Two different things have been called `nexo` historically:

1. `kernel/src/lang/nexo/` — the **NEXO language compiler** (in-kernel)
2. `nexo_ring3/` (formerly `nexo/`) — the **Ring 3 BSF loader** (userland)

These are now clearly separated. The kernel compiler is `kernel::lang::nexo`
and the userland loader is the `nexo_ring3` crate.

## How to navigate

- "What boots the kernel?" -> `bootloader/`
- "What is the kernel made of?" -> `kernel/src/`
- "What's the BMO ABI for Ring 3?" -> `kernel/src/bmo_abi/`
- "Where's the ÑEXO language?" -> `kernel/src/lang/nexo/`
- "What runs in Ring 3 later?" -> `nexo_ring3/` and `nexo-sh-tool/`
- "Where's the BMO-FS for the USB stick?" -> `bmofs/`
