# FastOS — Ultra Userspace (Ring 3)

The Ring 3 side of FastOS. Companion to `../Ultra_kernel/`.

This workspace contains everything that runs **after** the kernel has
finished its phases and has booted into userland: services, drivers
that live in userland, and the desktop applications.

## Boot relationship

```
  UEFI firmware
       │
       ▼
  uefi_chain.efi  (5 layers, ../Ultra_kernel/uefi_chain)
       │
       ▼
  stage1_arch → stage2_mm → stage3_dev  (../Ultra_kernel/stageN)
       │
       ▼
  bmo-kernel (Ring 0 base, ../Ultra_kernel/kernel)
       │
       ▼  (future: ELF / .bmo loader)
  ┌────────────────────────────────────────────────────────────┐
  │  Ultra_userspace  (this workspace) — Ring 3                 │
  │                                                            │
  │   bmo-service-gui     window manager / compositor          │
  │   bmo-service-input   keyboard + mouse multiplexer         │
  │   bmo-driver-keyboard keyboard userland driver             │
  │   bmo-driver-mouse    mouse userland driver                │
  │   bmo-app-launcher    desktop shell                        │
  │   bmo-app-terminal    terminal emulator (PTY)               │
  │   bmo-userland        shared crate (syscalls, panic)       │
  └────────────────────────────────────────────────────────────┘
```

## Crates

| Crate                  | Role                                                |
|------------------------|-----------------------------------------------------|
| `bmo-userland`         | Shared: panic handler, syscall bindings, allocator  |
| `services/gui`         | Window manager / compositor (server)                 |
| `services/input`       | Input multiplexer (server)                          |
| `drivers/keyboard`     | Userland keyboard driver (PS/2 or USB HID)          |
| `drivers/mouse`        | Userland mouse driver                                |
| `apps/launcher`        | Desktop shell                                        |
| `apps/terminal`        | Terminal emulator (PTY-backed)                       |

## Current state

All crates are **stubs** that compile and link but do nothing useful.
The Ring 3 base depends on infrastructure that doesn't exist yet:

- ELF / `.bmo` loader in `bmo-kernel` (currently the kernel just halts)
- A real syscall ABI (currently `arch::syscall` returns stub for everything)
- A page-table manager for user-space mappings (`mm::vmm` is a stub)
- A process / scheduler (`proc` is a stub)
- IPC mechanism (`channel` is a stub)

Each of these will be implemented in upcoming phases, **top-down**:
1. Real ELF loader in the kernel
2. Page-table mapping for ring3 (CR3 switching, ISTs for syscalls)
3. `syscall` instruction entry that calls into ring3 gateways
4. Shared memory pages for IPC (one page per process pair)
5. Window manager that can draw to a backbuffer page shared with each app

## Build

```powershell
cd C:\Users\andre\Documents\FastOS\Ultra_userspace
cargo build --release
```

Note: this workspace only builds the userland libraries. The actual
**binaries** (the launcher and terminal `.bmo` files) will be built
by a future `build.ps1` that produces the final bootable image.

## Shared ABI

All crates in this workspace depend on the local `bmo-userland`
crate, which re-exports `boot_context::BootContext` so userland has
the same struct as the kernel for handoff. The kernel will pass
`BootContext` to ring3 at process spawn time (future phase).
