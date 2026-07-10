# HAL Services & BMO Channel — Ring 0 ↔ Ring 3 Contract

This document describes the two complementary mechanisms FastOS uses to
let Ring 3 modules consume Ring 0 services without touching hardware
directly.

---

## 1. HalServices (function-pointer table)

A single `HalServices` struct is built by Ring 0 during boot and
**passed to module 0's entry function** as a `*const HalServices`
argument. The struct contains ~120 function pointers covering every
service a module might need.

The full struct lives in
`crates_Personal/shared/hal_defs/src/lib.rs`. Modules see the same
type, so no cross-ring `#![no_std]` glue is needed.

### Categories

| Category | Service | Notes |
|----------|---------|-------|
| **Console** | `serial_write`, `serial_write_u64` | COM1 115200 baud, timeout-guarded |
| **Watchdog** | `pet_fch_watchdog`, `watchdog_disarm` | Currently pet via timer ISR |
| **Framebuffer** | `backbuffer_ptr`, `backbuffer_stride`, `framebuffer_present`, `framebuffer_put_pixel` | GOP framebuffer + 8 MiB backbuffer |
| **Memory** | `alloc_pages_contiguous`, `free_pages`, `phys_to_pt`, `HIGH_MEM_BASE`, `read_cr3`, `write_cr3`, `create_user_page_table` | Ring 3 can build its own page tables |
| **Heap** | `heap_alloc`, `heap_free` | Slab-backed |
| **CPU** | `rdtsc`, `tsc_per_sec`, `busy_wait_ms`, `halt` | AMD Zen 3 invariant TSC |
| **Boot info** | `fb_addr`, `fb_width`, `fb_height`, `fb_stride`, `fb_pixel_format`, `boot_info` | Read-only snapshot |
| **Logging** | `write_boot_stage` | NVRAM-backed crash trail |
| **Tasks** | `task_alloc`, `task_free`, `task_set_current`, `task_block_on`, `task_wake_on` | Each task gets an 8 KiB kernel stack |
| **Scheduler** | `schedule`, `yield_now` | Cooperative, preemptive with APIC |
| **Audio** | `audio_init`, `audio_play`, `audio_play_logon_chime`, `audio_beep`, `audio_set_volume` | Real mixer (PC speaker backend) |
| **Input** | `input_init`, `input_poll(&mut [InputEvent])` | Drains BMO Channel into unified event stream |
| **Storage** | `storage_read_sectors`, `storage_write_sectors`, `storage_port_count`, `storage_port_active` | Safe stub until AHCI lands |
| **Filesystem** | `fs_mount`, `fs_read_file`, `fs_write_file`, `fs_find_subdir` | Safe stub until FAT32 lands |
| **Ring 3 transition** | `ring3_transition` | Naked iretq primitive |

### Stub-vs-real policy

When a driver has not been ported yet, the corresponding function
pointer in the table does **not** silently no-op. It does one of:

- Return a clearly-invalid value (`false`, `None`, `0`)
- Log a one-line warning to serial ("[storage] read_sectors stub")
- Never panic, never crash

This means a Ring 3 module can detect "service unavailable" and show
the user a friendly "Storage not available" UI instead of hanging.

---

## 2. BMO Channel (lock-free ringbuffer IPC)

For event-style traffic (keyboard scancodes arriving every few ms,
mouse packets, power button presses), syscall-style polling is too
expensive. BMO Channel is a lock-free MPSC ringbuffer living in a
single 4 KiB page shared between Ring 0 and Ring 3.

### Page layout

```
┌────────────────── 4096-byte shared page ──────────────────┐
│  magic  │ doorbell │ submit_h/t │ complete_h/t │  pad     │
│  submit_ring[64]  │  complete_ring[64]                    │
└───────────────────────────────────────────────────────────┘
```

- `magic` = `0x424D_4F43` ("BMOC")
- `doorbell` = 1 when Ring 3 has work for Ring 0
- `submit_head` = Ring 3 writes, Ring 0 reads (request ring)
- `complete_head` = Ring 0 writes, Ring 3 reads (response ring)

### Two channels exist

1. **System channel** (1 instance, owned by Ring 0)
   - `kernel/src/ring0/ipc_channel.rs::SYS_CHANNEL`
   - Physical address written to RAM marker `0x9_0160` for Ring 3
   - Ring 0 pushes hardware events here; Ring 3 polls them

2. **User channels** (up to 8 instances, registered by Ring 3)
   - Ring 3 calls `SYS_CHANNEL_REGISTER` to install one
   - Ring 0 processes them on each timer tick + on `SYS_CHANNEL_KICK`
   - Used for Ring 3 → Ring 0 service requests

### Opcodes

**Hardware events (Ring 0 → Ring 3):**

| Opcode | Name | Args (a0, a1, a2) | Meaning |
|--------|------|------------------|---------|
| `0xB000_0002` | `OP_KEY_SCANCODE` | code, pressed, _ | PS/2 keyboard event |
| `0xB000_0010` | `OP_MOUSE_MOVE` | dx, dy, _ | Relative mouse movement |
| `0xB000_0011` | `OP_MOUSE_BUTTON` | buttons, _, _ | Bitmask of left/right/middle |
| `0xB000_0012` | `OP_MOUSE_WHEEL` | dz, _, _ | Scroll wheel (positive = up) |
| `0xB000_0020` | `OP_POWER_BTN` | _, _, _ | ACPI power button (future) |
| `0xB000_0021` | `OP_LID_SWITCH` | _, _, _ | Laptop lid open/close (future) |
| `0xB000_0030` | `OP_BATTERY` | pct, charging, _ | Battery status (future) |
| `0xB000_0040` | `OP_THERMAL` | temp_c * 100, _, _ | CPU temperature (future) |
| `0xB000_0050` | `OP_TIMER_TICK` | rate_hz, _, _ | Periodic heartbeat |
| `0xB000_0060` | `OP_NETWORK_PKT` | len, ptr_lo, ptr_hi | Incoming packet (future) |
| `0xB000_0070` | `OP_STORAGE_CHANGE` | _, _, _ | Storage hotplug (future) |
| `0xB000_0080` | `OP_AUDIO_DONE` | voice_id, _, _ | Voice finished (future) |

**Service requests (Ring 3 → Ring 0, via user channel):**

| Opcode | Name | Args | Returns |
|--------|------|------|---------|
| `0x100` | `SVC_TIME_NOW` | - | tsc |
| `0x101` | `SVC_TIME_NS` | - | nanoseconds since boot |
| `0x102` | `SVC_FB_INFO` | - | (w, h, (stride<<16)\|fmt) |
| `0x103` | `SVC_FB_PRESENT` | - | 0 |
| `0x104` | `SVC_FB_FILL` | x\|y<<16, color, w\|h<<16 | 0 |
| `0x105` | `SVC_FB_TEXT` | (x, y, color, ptr, len) | bytes drawn |
| `0x110` | `SVC_SERIAL` | (ptr, len) | 0 |
| `0x120` | `SVC_BEEP` | (freq_hz, duration_ms) | 0 |
| `0x121` | `SVC_POWER_REBT` | - | (never returns) |
| `0x122` | `SVC_POWER_OFF` | - | (never returns) |
| `0x130` | `SVC_MEM_TOTAL` | - | bytes |
| `0x131` | `SVC_MEM_FREE` | - | bytes |
| `0x132` | `SVC_HEAP_USED` | - | bytes |
| `0x140` | `SVC_CPU_NAME` | (ptr, len) | bytes written |
| `0x141` | `SVC_TSC_FREQ` | - | Hz |
| `0x150` | `SVC_DIAG_BOOT` | - | last boot stage string id |
| `0x1F0` | `SVC_LOG` | (level, ptr, len) | 0 |
| `0x1FF` | `SVC_PING` | - | `0xC0FFEE` |

### Counters (read by `cabina` and tests)

- `kbd_events()` — kbd scancodes pushed since boot
- `mouse_events()` — mouse packets pushed since boot
- `wheel_events()` — wheel packets pushed since boot
- `svc_requests()` — total SVC requests received
- `svc_responses()` — total SVC responses sent

---

## 3. Module entry point

When the UEFI bootloader loads `mod_bmo_core` and other modules, it
calls:

```rust
entry_fn(hal: *const HalServices);
```

The module receives the table and **never** imports Ring 0 modules
directly. All hardware access is mediated.

---

## 4. Quick architectural rules

- **Ring 3 → Ring 0** direction: syscall (fast, ~50 ns) or BMO Channel
  request (eventual, ~1 ms). Use syscalls for tight loops (fb fill,
  beep, serial), BMO Channel for batched or one-shot requests.
- **Ring 0 → Ring 3** direction: BMO Channel events only. The HAL
  does not have a "callback into Ring 3" primitive; userland polls.
- **No module ever touches MMIO directly.** All PCI, ACPI, HPET,
  storage, and network access goes through the HAL or syscalls.
- **The HAL table is read-only at runtime.** Once built at boot, no
  function pointer in it changes. Modules can cache them freely.
