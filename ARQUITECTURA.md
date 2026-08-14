# BMO-X -- Technical Reference

The full technical document. For the short version, see [README.md](README.md).

A bare-metal system written in Rust, with a capability-based kernel and a
**frozen surface of two syscalls**. Graphics through UEFI GOP (framebuffer).
No proprietary driver dependencies.

**Status:** boots on real hardware and reaches the top -- full Ring 0, a desktop
in Ring 3, and **three in-house languages executing on silicon** (BMO C, BMO
COBOL, BMO Ada), compiled by the in-house toolchain.

**Test bench:** MSI A320M PRO MAX / AMD Ryzen 5 5600X (Zen 3). No QEMU.
Boot chain: unified UEFI (`BOOTX64.EFI` with the stages embedded) -> `s1_cpu` ->
`s2_mem` -> kernel.

**The number that sums it up:** BMO-X occupies **5.4 MiB of 14.8 GiB** of RAM
on the test machine.

**ABI surface:** `INVOKE` + `WAIT` (frozen, two doors) + Capability Engine in
Ring 0.

---

## The three states

Everything below is labelled with one of three states. Confusing them is how a
project loses track of itself -- and how it starts lying to its own author.

| | State | Meaning |
|:--:|---|---|
| 🟢 | **Runs on metal** | Seen on the Ryzen, with a photo or a CABINA telemetry line |
| 🟡 | **Written, never executed** | Compiles, links, passes its tests -- and no CPU has run it |
| ⚪ | **Design only** | Documented, not built |

**Only 🟢 counts as done.** The list of what is waiting for a boot lives in
[AVANCES.md](AVANCES.md).

---

## Layout -- multi-arch from day one

BMO splits into a **CPU-agnostic core** and a **per-CPU kernel tree**.

```
BMO/
+-- Ultra_kernel_x86-64/      x86-64 kernel: unified UEFI shim + 2 stages + Ring 0
|   +-- uefi_chain/           UEFI shim: embeds s1/s2/kernel, boots without reading disk
|   +-- faggin/               boot stages: s1_cpu, s2_mem, serial_shared
|   +-- boot_context/         handoff contract shim->s1->s2->kernel (magic + version)
|   +-- kernel/               Ring 0: Capability Engine, scheduler, mm, syscall, UI
+-- Ultra_userspace/          Ring 3 side, also x86-64 (sibling workspace)
+-- platform/                 CPU-AGNOSTIC CORE: bmo-abi, bmo-rt, drivers, services
|   +-- abi/                  bmo-abi (surface, capability, handle, BEF/BEX), bmo-rt
|   +-- shared/               bmo-hal, bmo-channel, bmo-hash, hw-profile
|   +-- drivers/              xhci, ahci, nvme, fat32, estratos, input, uhid, gpu
|   +-- services/             cabina-core
+-- toolchain/                build-time: language frontends -> BEF -> linker -> BEX
|   +-- lang/                 C, C++, COBOL, Ada -> BEF
|   +-- forge/                shared pipeline: bmo-lower, sem-asm, bmo-verify
|   +-- tools/                build-time generators, estratos-fmt
+-- Ultra_kernel_aarch64/     (planned) same structure, ARM chain
```

`platform/` is the part of BMO that is **genuinely CPU-agnostic** -- the BEF
format, the syscall surface, the lock-free channel, version control and the
language frontends work identically on any CPU.

To port to another architecture: duplicate `Ultra_kernel_x86-64/` as
`Ultra_kernel_<arch>/` and rewrite the **two stages** (`faggin/s1_cpu`,
`faggin/s2_mem` -- the only CPU-specific code) plus the inline assembly in the
kernel's `_start`.

---

## Architecture

```
+------------------------------------------------------------+
|                    Ring 3 -- Applications                   |
|  +----------+ +----------+ +----------+ +----------+       |
|  |Compositor| |  COBOL   | |    C     | |   Ada    |       |
|  | gui.bex  | | programs | | programs | | programs |       |
|  +----+-----+ +----+-----+ +----+-----+ +----+-----+       |
|       +------------+-------+----+------------+             |
|                            |  SYSCALL / SYSRET             |
+----------------------------+-------------------------------+
|                    Ring 0 -- Kernel                         |
|  +----------+ +----------+ +----------+ +----------+       |
|  |Capability| |Scheduler | |  Memory  | | ESTRATOS |       |
|  |  Engine  | |preemptive| | demand pg| | + FAT32  |       |
|  +----------+ +----------+ +----------+ +----------+       |
|  +----------+ +----------+ +----------+ +----------+       |
|  |  CABINA  | |ByteDefend| | TimeBack | | BEF Load |       |
|  |telemetry | |          | |          | | + verify |       |
|  +----------+ +----------+ +----------+ +----------+       |
|  +----------+ +----------+ +----------+ +----------+       |
|  |   APIC   | | ACPI/PCI | | MTRR/PAT | |xHCI/AHCI |       |
|  +----------+ +----------+ +----------+ +----------+       |
+------------------------------------------------------------+
|                       Hardware                             |
|    AMD Ryzen 5 5600X (Zen 3) | UEFI GOP | COM1 Serial      |
+------------------------------------------------------------+
```

---

## The frozen surface and subsyscalls

**Subsyscall** (a BMO term): an operation that travels *inside* a frozen
syscall, directed at a capability. The kernel exposes **two doors** --
`CORE_SYSCALL_COUNT = 2` in `syscalls/surface/puertas.rs`:

| # | Door | Role |
|---|---|---|
| 0x00 | `INVOKE(cap, operation, a0..a3)` | Do this NOW -- the only service door |
| 0x01 | *(reserved -- see below)* | |
| 0x02 | `WAIT(waitable, seq, timeout)` | Wake me WHEN |

### Why it went from three to two (2026-08-10)

`CHANNEL_KICK(cap, seq)` held the `1`. It resolved a handle, checked it was a
channel, and notified its consumer: **an operation on a handle**, which is the
definition of `INVOKE`. It had its own number because of how it was born, not
because of what it did. Today it is `CHANNEL_OP_KICK` (0x03) and it sits under
`RIGHT_WRITE`, not READ, because **notifying is writing**. Nothing was lost:
nobody was calling it.

[!] **The `1` is RESERVED, not recycled.** An old binary that calls it has to
fail *saying so*; recycling the number would make it do something nobody asked
for, without failing anywhere. `name(1)` returns `None` on purpose, and the
test `la_superficie_son_dos_puertas_y_el_uno_esta_reservado` guards it.

### And it does not go down to one

`WAIT` cannot be expressed with `INVOKE`, because the only thing it does is
**not give the turn back**. A synchronous call would have to answer *"not
yet"* and let the program ask again -- that is, burn its turn asking, which is
exactly what `WAIT` exists to avoid.

Everything else is a **subsyscall**: the pair `(handle kind x operation)`,
resolved by the Capability Engine. The system grows by adding *kinds* and
*operations* -- **never** a new door.

### Registered subsyscalls

| Kind | Operation | # | State |
|---|---|---|---|
| Task (`CURRENT_TASK`) | `GET_PID` | 0x01 | stable |
| Task | `GET_TID` | 0x02 | stable |
| Task | `YIELD` | 0x03 | stable |
| Task | `EXIT` | 0x04 | stable |
| Task | `CHANNEL_OPEN` | 0x05 | stable |
| Task | `CONSOLE_WRITE` | 0x06 | stable -- routes to `KIND_CONSOLE` if the process has one |
| Task | `ENDPOINT_CREATE` / `CONNECT` | 0x07 / 0x08 | *bootstrap* -- no name service yet |
| Task | `FRAMEBUFFER_CLAIM` | 0x09 | stable -- exclusive |
| Task | `INPUT_CLAIM` | 0x0A | stable -- exclusive, mouse **and** keyboard |
| Task | `RUTA` / `EJECUTAR` | 0x0B / 0x0C | stable -- launch from Ring 3, with signature gate |
| Task | `CONSOLA_CREAR` | 0x0D | stable |
| Task | `DIR_ABRIR` | 0x0E | stable |
| Task | `CONSOLE_READ` | 0x0F | stable -- the partner of `CONSOLE_WRITE` |
| Channel | `GET_SEQ` / `GET_INDEX` | 0x01 / 0x02 | stable |
| Framebuffer | `BASE` / `DIMS` / `STRIDE` / `BYTES` | 0x01-0x04 | stable |
| Input | `PUNTERO` / `EVENTOS` | 0x01 / 0x02 | stable |
| Input | `TECLA` / `MODIFICADORES` | 0x03 / 0x04 | stable |
| Console | `LEER` / `PERDIDOS` | 0x01 / 0x02 | stable |
| Console | `ESCRIBIR` / `HAY_HIJO` | 0x03 / 0x04 | stable |
| Directory | `SIGUIENTE` / `NOMBRE` | 0x01 / 0x02 | stable |

**Five kinds, zero new doors.** Everything added -- the screen, input, a console
with both directions, directories, launching programs -- fits inside `INVOKE`.
That is the proof the frozen ABI holds: the system grew and the boundary did
not move by a single number.

### Contract rules

1. **Single source of truth** -- the numbers live in `platform/abi/bmo-abi`
   (`syscalls/surface.rs`); the kernel mirrors them and `build.ps1` has a drift
   guard that breaks the build if they diverge.
2. **Lifecycle** -- a subsyscall may be born as *bootstrap* on Task (e.g.
   `CONSOLE_WRITE`) and mature into an operation on a dedicated capability.
   Being born is easy; **the door never changes**.
3. **By value first** -- arguments travel in registers. Large payloads go through
   a BMO Channel; the subsyscall carries the control.
4. **RPC to Ring 3** -- the Endpoint design
   (`platform/abi/bmo-abi/src/ENDPOINT_RPC.md`) extends `INVOKE` to Ring 3
   servers without touching the surface.

### Empirical proof (real hardware, 2026-07-22)

The first Ring 3 program lived and died through **9 calls on 1 single door**
(8x `INVOKE-CONSOLE_WRITE` + 1x `INVOKE-EXIT`). Surface intact.

---

## 🟢 Runs on real hardware

**Boot and CPU**
- **Unified UEFI boot** -> `BOOTX64.EFI` with `s1_cpu` + `s2_mem` + kernel
  embedded -> GOP 1920x1080. Zero dependence on the firmware's FAT reader
- **GDT + TSS** Ring 0 / Ring 3 with IST1, and a 256-entry **IDT**
  (#GP/#PF/#UD/#NM/#MF/#XM/#DE/#DF)
- **AMD Zen 3**: CPUID, errata, TSC calibration
- **XSAVE per task** -- with its root cause paid for: `XSAVE` *merges* the
  header, it does not *store* it (see [BITACORA.md](BITACORA.md), ep. 14)

**Kernel**
- **Two frozen syscalls** (`INVOKE` + `WAIT`) -- everything else is a
  subsyscall. `CHANNEL_KICK` was the third until 2026-08-10; it is now
  `CHANNEL_OP_KICK`, and the number `1` stays **reserved**, never recycled
- **Capability Engine**: 16 processes x 64 slots, handles with generation
  counters against use-after-free; `revoke_all` on death
- **Preemptive scheduler** by LAPIC timer, with real Ring 0 <-> Ring 3 switching
  (`iretq` -> CPL3 -> `INVOKE` -> CPL0 -> `EXIT` -> reap)
- **Fault isolation**: a fault in CPL3 kills the task and BMO carries on
- **Page allocator**: a bitmap. 512 KiB in `.bss`, 4 KiB frames, one spinlock
  and a hint. Covers exactly the physmap (16 GiB)
- **4-level VMM** (PML4/PDPT/PD/PT). No demand paging and no CoW: a mapping
  exists or it does not

**Drivers, all written from scratch**
- **USB keyboard** (xHCI + HID): es-latam / es-espana / us layouts switchable
  live, dead keys, real AltGr, readline-style editing, key repeat, LEDs, history
- **AHCI/SATA disk** + **GPT** + **FAT32**: the kernel reads and mounts its own
  disk. The data volume mounts **for writing**; the boot volume, never
- **ESTRATOS mounted**, superblock read and **the signature verified before
  execution**

**Capabilities in practice**
- **The screen, input, console, directories and files are capabilities**
  (`KIND_FRAMEBUFFER` / `INPUT` / `CONSOLE` / `DIRECTORIO` / `ARCHIVO`): Ring 3
  paints with `mov` and the kernel steps aside
- **RPC endpoint** (`KIND_ENDPOINT` + `KIND_REPLY`): two Ring 3 processes
  talking through the kernel, without touching the two syscalls
- **Compositor in Ring 3**, loaded from `sys/gui.bex` -- changing the desktop
  does not recompile the kernel

**Languages**
- **Two in-house languages on silicon**: BMO C and BMO COBOL (exact decimal,
  file I/O, `OCCURS`, level 88). Both appear by name in the metal-test
  documents; **Ada does not, and that is why it is not here** -- see the yellow
  section
- **Runtime `PICTURE` editing**, photographed: `$12,345.67`, `*****0.45` and
  `  120.00CR` aligned. The whole chain -- COBOL source -> parser -> codegen -> BEF
  -> real CPU -- produces a bank's statement line
- **COBOL file I/O**: `batch.bex` reads transactions, totals in cents and writes
  the close to disk
- **Input in BMO C** (`getchar` / `scanf`)

**Observability**
- **CABINA**: telemetry that RECORDS at the instant of the event (IRQ-safe), it
  does not poll -- modules push their event even before a framebuffer exists

---

## 🟡 Written, never executed on a CPU

- **ESTRATOS writes** -- the transaction state machine exists and is tested;
  **nobody has wired it to the device**. The data window says so in red, and it
  has to
- **SMP** -- the code to wake the APs **exists** in `s1_cpu` (trampoline,
  INIT+SIPI, percpu) and **nothing calls it**. Deliberately last: the day a
  second core runs, every `static mut` in the kernel is a race. The trampoline
  is 10%; auditing shared state is the other 90%
- **Framebuffer write-combining** (PAT) -- pending, and it will show: today every
  pixel is an uncached write
- **USB mouse** -- enumerates and delivers pointer and buttons, but the shared
  event ring fix (BITACORA ep. 18) is waiting on a photo
- **Desktop focus** -- F12 opens the ESTRATOS data console, **Alt+Tab** walks the
  MRU stack, three modes (`normal` / `fixed` / `follows pointer`). The policy
  lives in `bmo_input::foco` -- where it can be TESTED -- and the compositor only
  paints what was decided
- **Native BEF** -- format, validation, sections, imports/exports done;
  relocations and TLS still evolving
- **ByteDefender** -- BEF headers only, no heuristics
- **TimeBack** -- the API exists; capture and rollback do nothing yet
- **Ada** -- frontend of 1.608 lines with one example (`examples/1-basico`).
  It rejects an invented statement with a line and a reason, which is more than
  some of its siblings could do. But **it has never run on a CPU**: it does not
  appear in a single metal-test document, and `subtype D is Integer range
  1 .. 31;` does not parse yet. Its gap is one of *scope*, not debt. The plan
  is ZFP (not Ravenscar) with ACATS as the conformance matrix, and the expensive
  part is already paid: Annex F copied COBOL's PICTURE, so the exact decimal
  exists
- **C++** -- minimal frontend (~900 lines), cheap on top of honest C when the
  time comes

---

## ⚪ Designed, not built

- **NVMe** -- there is a folder and it is unused. The NVMe in this machine holds
  the owner's Windows; the kernel asks for the controller **BY TYPE**, never
  "the first one found in the scan"
- **Networking and audio** -- no network stack; audio is `beep()` only
- **I/O APIC**, **EDF scheduler**
- **ESTRATOS garbage collector** -- the policy is written: the owner decides,
  named strata are never released, amber at 70%, a concrete proposal at 85%,
  read-only at 95% rather than lose data
- **Memory capability** -- a process receives its image and 64 KiB of stack and
  cannot ask for more. Unlocks two things at once: any language with a GC, and
  the **shared surfaces** real windows need
- **BMO GPU** -- RDNA4 skeleton, no driver

---

## Deliberately out of scope

Stated plainly, because promising compatibility that does not exist is exactly
what sank the previous project.

- **Vulkan / GPU** -- another project the size of this one. And the reason it
  stopped is the architecture in miniature: BMO-X squeezes hardware using the
  documentation each manufacturer publishes. AMD publishes its GPU ISA. NVIDIA
  does not. This machine has an NVIDIA card
- **Wine / Win32 / Windows applications** -- thirty years of work for
  compatibility this goal does not use
- **A complete libc, POSIX personality, "compiling the world"** -- a full
  preprocessor, headers, separate compilation and a libc are years of work
- **Windows with shared surfaces** -- today `KIND_FRAMEBUFFER` is exclusive: one
  process owns the screen

These are decisions **of this phase**, not renunciations. The day games come
back, the unlock is already listed: memory capability -> shared surfaces ->
windows.

---

## Memory allocator

**It is a bitmap.** One, flat, with a lock.

```
alloc_frame() / alloc_frames_contig(n) / free_frame()      ring0/mm/phys.rs
        |
        v
  BITMAP: [u64; 65536]        512 KiB in `.bss`, one bit per 4 KiB frame
  SpinLock + HINT             the hint is where the last search stopped
        |
        v
  Coverage = PHYSMAP_SIZE (16 GiB), and that bound is not decoration:
  `zero_frame` and every page-table write go through `phys_to_virt`, so a
  frame the physmap cannot reach must never be handed out
```

Its source of truth is the `BootContext` memory map **after** `s2_mem` carved
out its page-table pool, plus the kernel's own reservations: low memory, the
kernel image, the faggin/UEFI stages, the BootContext page, the GOP
framebuffer, the LAPIC/IOAPIC/HPET window and the reserved Ring 3 ranges.

### The weakness it has, said out loud

`alloc_frames_contig(n)` is a **linear scan** of the whole bitmap looking for a
free run. Its callers are the ones that ask for the big blocks: the
compositor's double buffer (~8 MB = 2048 frames) and DOOM's `Z_Zone` (12 MiB =
3072). With fragmented RAM every large request walks 16 GiB of bitmap.

That is the real, measured shape of the problem -- not lock contention.

### LLFree -- an IDEA, not a feature

[!] Until 2026-08-14 this section claimed a buddy allocator with per-CPU
pagesets and an opt-in `--features alloc-llfree`, complete with a "204-line
adapter + 2199-line crate" that "boots clean on the Ryzen 5600X with identical
behaviour -- no crashes, no regressions, +25 KB".

**None of it existed.** No feature, no adapter, no buddy, no pagesets --
`grep -rn "llfree\|buddy\|pageset" --include=*.rs` returns nothing. The worst
part was not the missing code: it was a **fabricated hardware measurement** in
the document people read first.

What LLFree (Wrenger et al., USENIX ATC '23) actually offers, and what of it is
worth anything here today:

* **Not** its headline. LLFree solves *contention between cores*, and AXION
  does not hand heavy work to other cores yet. Integrating it now would be
  optimising a bottleneck that does not exist.
* **Yes** its shape: hierarchical counters instead of a flat bitmap answer
  *"is there a free run of 2^k under this subtree?"* in O(log n) instead of
  O(n). That is the fix for `alloc_frames_contig` above, and it has nothing to
  do with SMP.
* **Yes** its philosophy: the allocator state **is** the data structure, so it
  can be rebuilt by scanning instead of journalled. That is already the shape
  of ESTRATOS (writing *is* committing) and of the DIRECTOR (it finds out by
  *looking*, nobody sends it a message).

So: no crate, no feature flag. A design note, and it says so.

---

## Boot path

**Not a single file is read from firmware.** `BOOTX64.EFI` carries both stages
and the kernel inside (`include_bytes!`), and the kernel is copied to its
address **after** `ExitBootServices` -- the Linux EFI stub pattern. It is the
answer to a motherboard that never attached a FAT driver (BITACORA ep. 1).

```
UEFI Firmware
  -> BOOTX64.EFI (uefi_chain: s1_cpu + s2_mem + kernel embedded)
    1. Query GOP (1920x1080), memory map, RSDP
    2. ExitBootServices          <- firmware's favours end here
    3. Copy the stages and kernel to their addresses and jump
  -> s1_cpu @0x100000   CPU: cli + mask the PIC BEFORE touching the GDT
                       (firmware hands over with interrupts ON -- ep. 2)
  -> s2_mem @0x200000   memory: map, physmap, handoff verified by magic
  -> kernel  @0x400000  ring0::core::entry::_start -> phase::main(ctx)
    1. Validate BootContext (magic + version). If it does not match, SAY SO in red
    2. xsave::init()      <- before anything that could fault: the area is fixed
                            and only this CPU knows its size
    3. percpu + scheduler + mm (phys, vmm) + channel + services + syscall
    4. faults::init()     <- the on-screen report ARMED before anything can
                            enter Ring 3
    5. timer::init()      <- LAPIC tick: the scheduler becomes preemptive
    6. PCI -> xHCI (keyboard and mouse) -> AHCI -> GPT -> FAT32 -> ESTRATOS
    7. lanzar::ruta("sys/gui.bex")   <- the compositor, from DISK
  -> Ring 3: the desktop. If it fails to start, the machine stays in the kernel
    shell and CABINA says why
```

### Inside the kernel

```
Ultra_kernel_x86-64/kernel/src/ring0/
  core/    entry.rs (_start), phase.rs (boot by phases), informe.rs, splash, font
  cpu/     GDT/IDT/TSS, XSAVE; cpu_vendor/ Zen 3: CPUID, caches, TSC, errata
  mm/      phys (frames), vmm (4 levels), 16 GiB physmap
  task/    preemptive scheduler, percpu, proc, the program registry
  obj/     the capabilities: channel, input, framebuffer, console, archivo, endpoint
  dev/     pci, usb (xHCI), disk (AHCI), console/serial, framebuffer, keyboard
  fsys/    fat32, estratos + the identity gate and the write window
  svc/     Ring 0 services registered in estuary 0
  plat/    faults, timer (LAPIC)
  cabina.rs   telemetry that records at the instant of the event
```

---

## Build

The script lives at `Ultra_kernel_x86-64/build.ps1`.

```powershell
# Compile and validate, touching no disk (this is the default)
.\build.ps1 -BuildOnly

# Ring 0 to the boot volume, programs to the data volume
.\build.ps1 -Flash -Drive A -Data A -Yes
```

**The two flags are separate on purpose**: `-Flash` touches the boot ESP and
`-Data` touches the program volume. Sharing a flag would invite writing to one
when you meant the other.

And **nothing is written outside the project tree without three locks**: it
cannot be the system disk, it must be FAT/FAT32, and the full phrase with the
drive letter has to be typed (`-Yes` skips it, and that is on whoever types it).

Everything copied is verified by **SHA-256 at the destination**: a half-copied
`.bex` does not fail at boot, it fails at BEX admission -- and that message sends
you looking for the bug in the compiler instead of in the cable.

What gets deployed:

```
EFI\BOOT\    BOOTX64.EFI (with the stages and kernel inside) + BMO-MANIFEST.TXT
sys\         gui.bex          the compositor
cobol\ c\ ada\                the example programs, by language
datos\       the .txt files those programs read
```

**Requirements**
- Rust **nightly** (userspace compiles to `x86_64-unknown-none` with its own
  linker script)
- UEFI with **Secure Boot disabled**
- The BMO disk mounted with a drive letter

The script, in order: validates the **syscall contract** (drift guard) -> s1_cpu
-> s2_mem -> **Ring 3 compositor** (`bex-link` translates the ELF to `.bex` and
fixes the addresses) -> the COBOL, Ada and C examples with the in-house frontends
-> kernel -> `uefi_chain` which embeds everything -> staging -> verified deployment.

---

## Hardware

- **CPU**: AMD Ryzen 5 5600X (Zen 3, Family 19h, 6C/12T)
- **RAM**: any size (detected from the UEFI memory map)
- **GPU**: UEFI GOP framebuffer (1920x1080 BGR, backbuffer)
- **Serial**: COM1 115200 baud (debug output)
- **PCI**: enumeration via I/O ports (0xCF8/0xCFC)
- **ACPI**: RSDP/XSDT/MCFG/FADT
- **Disk**: in-house AHCI/SATA (Kingston SA400S37480G, 447 GiB)

> ⚠ The NVMe in this machine holds the owner's Windows. The kernel asks for the
> controller **by type**, never "the first one in the scan". This is not
> paranoia -- it is the difference between a bug and a destroyed disk.

---

## Roadmap

1. **Wire ESTRATOS writes to the device.** The only thing separating "a store
   you can read" from "a store". The transaction is written and tested; what is
   missing is the real `write` and `FLUSH CACHE`
2. ~~**COMP-3 (packed decimal)** in COBOL.~~ ✅ **done 2026-08-03** -- a
   `COMP-3` field is stored as nibbles, exactly as wide as its PICTURE says.
   What remains is the other half: **binary records**, so the compiler can read
   the packed bytes of a file as they come instead of one text number per line
3. **Range checks in Ada.** Without them it is Ada syntax with C safety, which
   is the worst of both
4. **Memory capability.** Unlocks GC languages and shared surfaces at once
5. **Framebuffer write-combining** (PAT). Cheap, and felt on every pixel
6. **Ada toward ACATS** as a conformance matrix -- the standard has its own test
   suite, and it is the honest way to measure how much Ada is really there
7. **Surfaces and windows.** Wayland in miniature, on top of item 4
8. **SMP last.** The trampoline is 10%; auditing shared state is 90%

---

## Principles

- **GOP first** -- everything visual through the framebuffer, no proprietary GPU
  drivers
- **Ring 0 + Ring 3 only** -- no Rings 1/2 (modern x86-64)
- **Serial debug** -- COM1 as the primary diagnostic channel
- **Preemptive** -- LAPIC timer scheduler with real ring switching, proven on
  hardware
- **Modular** -- each subsystem independent
- **Debuggable from the first byte** -- CABINA records before there is a screen
  to record onto
- **Reject with a reason** -- a compiler that quietly accepts what it cannot do
  is worse than one that refuses out loud

---

## The three musketeers

Named by the author, and the names stuck:

| Musketeer | Component | Line |
|---|---|---|
| **Athos** | CABINA | *"I saw what happened."* |
| **Porthos** | TimeBack | *"I undo it."* |
| **Aramis** | ByteDefender | *"I stop it happening."* |

---

Built from scratch in Lima, Peru, by **Eddi Salazar**.
Licensed under [Techne License v2.0](LICENSE.txt).
