crates_Personal/ (38 crates, todos compilan)
│
├── 🔵 BOOT
│   └── bootloader/          → BOOTX64.EFI (v0.5.0, pre-carga módulos)
│
├── 🔴 RING 0 (kernel.elf = 191.8 KB)
│   ├── cpu-vendor-profile/  → AMD Zen3 init
│   ├── boot_protocol/       → BootInfo v3 + ModuleEntry
│   ├── nvram-log/           → Crash diagnostics
│   └── bmo-hal-defs/        → HalServices struct
│
├── 🟢 RING 3 MÓDULOS (.elf)
│   ├── mod_bmo_core/        → desktop, WM, UI (411 KB)
│   ├── mod_timeback/        → versioning (12 KB)
│   └── mod_cabina/          → telemetry (12 KB)
│
├── 🟡 RING 3 LIBRERÍAS
│   ├── bmo_core/            → 103 archivos, engine desktop
│   ├── bmo_abi/             → 74 archivos, ABI/BEF/runtime
│   ├── timeback/            → 16 archivos, git-like versioning
│   ├── byte_defender/       → 9 archivos, security scanner
│   ├── userland_ring3/      → 10 archivos, C runtime + syscall wrappers
│   ├── goblin/              → ELF/PE/MachO parser
│   └── scroll/ + plain/     → binary parsing utilities
│
├── 🟠 RING 3 DRIVERS
│   ├── bmo_ahci/            → AHCI SATA storage
│   ├── bmo_fat32/           → FAT32/exFAT filesystem
│   ├── bmo_xhci/            → USB XHCI controller
│   ├── bmo_uhid/            → USB HID keyboard/mouse
│   ├── bmo_input/           → PS/2 keyboard/mouse HAL
│   ├── bmo_audio/           → PC speaker
│   ├── bmo_nvme/            → NVMe SSD driver ← NUEVO
│   └── bmo_net/             → e1000 NIC + smoltcp ← NUEVO
│
├── 🟣 CABINA (telemetry)
│   ├── cabina/core/          → types
│   ├── cabina/daemon/        → ring buffer, serial
│   └── cabina/panels/        → HUD overlay
│
└── ⚪ LENGUAJES (offline)
    ├── C, COBOL, C++ compilers
    └── Semantic_ASM/