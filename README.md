<p align="center">
  <img src="docs/arte/bmo-x-gato.jpg" alt="BMO-X -- BMO METAKERNEL" width="320">
</p>

# BMO-X

**A bare-metal orchestrator that boots on real hardware and runs COBOL, C, Ada
and INTI -- compiled by its own toolchain. No LLVM. No GCC. No QEMU.**

![status](https://img.shields.io/badge/boots_on-real_hardware-2ea043)
![cpu](https://img.shields.io/badge/verified_on-Ryzen_5_5600X-2ea043)
![ram](https://img.shields.io/badge/RAM_footprint-5.4_MiB-1f6feb)
![syscalls](https://img.shields.io/badge/syscalls-2_frozen-1f6feb)
![languages](https://img.shields.io/badge/native_languages-COBOL_-_C_-_Ada_-_INTI-8957e5)
![inti](https://img.shields.io/badge/system_language-INTI-f0883e)
![license](https://img.shields.io/badge/license-Apache_2.0-d29922)

Written from scratch in Rust -- the boot chain, the kernel, the drivers, the
filesystem, and **four native compilers**. It boots on an AMD Ryzen 5 5600X and
occupies **5.4 MiB of 14.8 GiB of RAM**.

**1.792 commits - 1.204 files - 17 April to 5 September 2026 - one developer.**

<details>
<summary><b>En espanol</b> -- que es esto, en un parrafo</summary>

BMO-X no es un sistema operativo de proposito general y no aspira a serlo. Un
sistema operativo **multiplexa**: le miente a cada programa diciendole que esta
solo en la maquina. BMO-X **orquesta**: cada programa sabe su parte, su entrada
y lo que cuesta. Por eso la frontera entera con el sistema son **dos syscalls**
--`INVOKE` y `WAIT`-- y no trescientas.

Esta escrito desde cero en Rust, en Lima, por una persona: la cadena de
arranque, el kernel, los drivers, el sistema de ficheros y cuatro compiladores
nativos (C, COBOL, Ada e INTI) que no usan LLVM ni GCC. Arranca en un AMD Ryzen
5 5600X **de verdad**, no en QEMU.

La documentacion tecnica esta en espanol --`BITACORA.md`, `ARQUITECTURA.md`,
`AVANCES.md` y todo `docs/`-- y este README en ingles a proposito: es la puerta
de entrada, y la conversacion de fuera ocurre en ingles. Lo que el sistema
IMPRIME en pantalla es espanol sin acentos, y eso tiene su propio motivo tecnico
(ver `CONTRIBUTING.md`).

</details>

---

## The one sentence

**Authority is functional, never inherited.** What a process may do comes from
the handles it holds -- not from who launched it, what it is called, or which
user is logged in. There is no `root` to inherit from and no ambient permission
to escalate into.

The teller's process does not *fail a check* on a large transfer. **The
operation does not exist for it.**

Everything below is a consequence of that sentence, and the four other clauses
that follow from it are in **[ARQUITECTURA.md](ARQUITECTURA.md)** with the
mechanism that implements each one -- because a principle with no mechanism
under it is a slogan.

---

## Read this first: the three states

| State | Meaning |
|---|---|
| 🟢 **Runs on metal** | Seen working on the real Ryzen, with a photo or a telemetry line |
| 🟡 **Written, never executed** | Compiles, links, passes its tests -- and no CPU has ever run it |
| ⚪ **Design only** | Documented, not built |

**Only 🟢 counts as done.** Nothing is green because it *should* work.

---

## 🟢 What the silicon has actually done

| | |
|---|---|
| Boots UEFI -> Ring 0 -> Ring 3 | kernel up at **47 ms**, first composed frame at **1.210 ms** |
| Runs its own compilers' output | COBOL, C and Ada binaries, launched from disk |
| Decimal arithmetic that is exact | a bank batch totalling `$1,135.00` from a file it read |
| USB keyboard and mouse | xHCI + HID written here, no BIOS help |
| Disk | AHCI, FAT32 read *and* write, plus ESTRATOS, its own copy-on-write volume |
| 12 cores | SMP bring-up, `12 of 12` |
| Ring 3 isolation | a fault kills the task; the kernel takes the screen back and prints its last four lines |
| Traps its own undefined behaviour | INTI `llano` on the Ryzen: overflow, divide-by-zero and bad conversion all caught **in metal** |
| Plays DOOM | full width with the status bar, 1600x1000 scaled x5, **27 fps** -- every pixel expanded and blitted by the CPU, no GPU |
| 12 cores doing real work | a kernel-side workload measured at **11,52x** over one core |
| Explains its own crashes in Spanish | a page fault inside the framebuffer prints `ESCRIBIA EN LA PANTALLA QUE YA NO ES SUYA -- fila 231`, not just an address |

### Watch it boot

<!-- ===================================================================
     EL VIDEO DE ARRANQUE VA AQUI.

     Como se pone, en GitHub y sin equivocarse:

       1. Abre una issue nueva en el repo (no hace falta enviarla).
       2. Arrastra el .mp4 dentro de la caja de texto.
       3. GitHub lo sube y te devuelve una URL que empieza por
          https://github.com/user-attachments/assets/...
       4. Pega esa url SOLA, en su propia linea, justo debajo de este
          comentario. GitHub la convierte en un reproductor dentro del
          README -- el enlace de abajo se queda como respaldo.
       5. Cierra la issue sin enviarla. El fichero ya vive en GitHub.

     [!] NO funciona ![](docs/evidencia/15.mp4): eso no reproduce nada,
     sale un enlace roto. Y un <video src="..."> tampoco: GitHub lo
     filtra. El enlace de abajo SI funciona --la pagina del fichero en
     GitHub lleva reproductor-- pero saca al lector del README, que es
     justo lo que no quieres el dia que publiques.
     =================================================================== -->

![BMO-X booting](docs/evidencia/15-arranque.gif)

A short edited piece: title card, then the machine coming up, from the
firmware's own boot picker to BMO-X. **[Same thing as video, with sound and at
full size](https://github.com/AndreeSalazar/BMO-X/blob/main/docs/evidencia/15.mp4)**
(7 s, 848x480).

The unbroken take for the sceptic is a different recording and is
**[described here](docs/evidencia/)** -- it has not been shot yet.

> **Booted by the firmware, not by a loader.** The motherboard's own boot picker
> lists `BMO-X` next to `Windows Boot Manager` -- no Ventoy, no GRUB, no
> hypervisor, no QEMU. A system launched from inside a multi-boot loader can be
> dismissed as a payload; one the firmware itself boots cannot.
>
> The stills, the test bench and what each one proves:
> **[docs/evidencia/](docs/evidencia/)**.

**The rescue that makes the rest safe**: `Ctrl+Alt+ESC` returns keyboard *and*
screen to the kernel from any program, checked at the one point in Ring 0 every
key passes through. A program that holds both cannot keep the machine hostage --
whether the cause is malice or a missing `if`.

Photographs, telemetry and the exact dates: **[AVANCES.md](AVANCES.md)**.

---

## Try it yourself

### 1. Build it. This touches no disk.

```powershell
.\bmo.ps1
```

That is the whole thing: seventeen compatibility rules, the ASCII sweep, the
syscall contract across kernel/ABI/userland, four compilers, the kernel, the
desktop, and thirty programs. About a minute, and **it cannot write to any
drive** -- deploying is a separate command on purpose. Add `-Rapido` to skip the
test bench while you iterate.

### 2. Put it on a USB stick and boot it.

```powershell
.\desplegar.ps1 -Arranque E -Datos E
```

**Those two letters are Windows drive letters of the stick you are writing to.**
Nothing else. They are separate because BMO-X deploys two different things:

| flag | what lands there | what it must be |
|---|---|---|
| `-Arranque` | `EFI\BOOT\BOOTX64.EFI` -- **the entire operating system, one file** | a **FAT32** partition the firmware can boot (an ESP) |
| `-Datos` | `BMO-DATA\` -- the desktop and the programs | any FAT32 volume BMO-X can read |

On a normal USB stick with one FAT32 partition **both are the same letter**, and
that is the usual case. They stay separate because they are two different risks:
`-Arranque` decides what the firmware runs, `-Datos` only decides what is on the
menu. Getting the first wrong and the second right is how you spend an evening
testing yesterday's kernel without noticing.

> [!] **Check the letter twice.** Whatever is on that drive gets overwritten.
> `.\bmo.ps1` with no flags is always safe; `.\desplegar.ps1` is the one that
> writes. That split is the only reason it exists.
>
> The script refuses an NTFS volume rather than reporting a false success, so
> pointing it at a Ventoy payload partition fails loudly instead of quietly.

### 2b. Or skip the script entirely -- copy two things onto a stick.

**You do not need Windows or PowerShell for this part.** The build leaves a
folder that *is* the layout of a bootable stick, so any OS that can copy files
can deploy BMO-X:

```text
Ultra_kernel_x86-64/staging/
    EFI/BOOT/BOOTX64.EFI      <- the whole operating system
    EFI/BOOT/BMO-MANIFEST.TXT <- sizes and SHA-256, for checking
    BMO-DATA/                 <- the desktop and the programs
```

1. Format a USB stick as **FAT32** (not exFAT, not NTFS -- UEFI reads FAT).
2. Copy **the contents of `staging/`** to the root of the stick, so that the
   stick ends up with `EFI\BOOT\BOOTX64.EFI` and `BMO-DATA\` at its top level.
3. That is the whole deployment. There is no bootloader to install, no
   `grub.cfg`, no initrd, no kernel command line, and nothing to configure.

**Why it is that simple**: UEFI firmware looks for `EFI\BOOT\BOOTX64.EFI` on any
FAT volume and runs it. BMO-X *is* that file -- boot chain, kernel and drivers
in one binary -- so there is nothing left for a bootloader to do. Ventoy, GRUB
and rEFInd all exist to choose between things and hand off; here there is
nothing to hand off to.

That also means the firmware's own boot menu lists it directly, next to
`Windows Boot Manager`, which is what photo 1 in
**[docs/evidencia/](docs/evidencia/)** shows.

### 3. Turn Secure Boot off first.

**This is the one that will waste your evening otherwise.** `BOOTX64.EFI` is not
signed by a key your firmware trusts, so a machine with Secure Boot enabled --
which is most machines shipped in the last decade -- **refuses to load it** and
says something like `Security Violation`. That is not a bug in BMO-X and there
is nothing to debug: the firmware never ran a single instruction of it.

It is in the BIOS setup, usually under *Boot* or *Security*, and it is one
toggle. Turn it back on afterwards if you want -- Windows does not care either
way as long as it was not installed in a mode you are changing.

> Signing the boot chain properly is on the roadmap and blocked behind the same
> thing everything else is: **cryptography**. Until then, "turn it off" is the
> honest instruction and not an oversight. See
> **[SEGURIDAD_MAESTRO.md](docs/maestro/SEGURIDAD_MAESTRO.md)** section 4.3 for
> why measured boot is *refused* rather than pending.

### 4. Reboot and pick it.

Open the firmware's boot menu -- it is a key pressed right after power-on, and
which key depends on the board: **F11** on MSI, **F12** on Gigabyte and Lenovo,
**F8** on ASUS, **F9** on HP, **Esc** on many others. If you miss it, the BIOS
setup itself (**Del** or **F2**) has a boot-order page that does the same thing.

Pick `BMO-X` and you are in. Click the
DOOM icon, or type `info`, `cpu`, `mem`, `ls` in the launcher. `Ctrl+Alt+ESC`
always takes the machine back from whatever is running.

### What actually gets written

Measured from a real build, not estimated:

| | bytes | what it is |
|---|---:|---|
| `BOOTX64.EFI` | **1.218.048** | boot chain + 2 stages + the Ring 0 kernel. **This one file is the OS** |
| `sys/d.bex` | 623.448 | the desktop and compositor -- Ring 3, loaded from disk |
| 28 more programs | 271.264 | C, COBOL, Ada and INTI examples, all compiled here |
| `apps/doom.bex` | 912.512 | optional, GPL, built only if the port is present |
| `apps/doom1.wad` | 4.196.020 | id Software's shareware data, not ours |

**The operating system is 1,16 MiB.** Not the kernel -- the boot chain, the
kernel, the drivers, the filesystems, USB, AHCI and the capability engine, in a
single file the firmware loads. It uses **5,4 MiB of RAM** once running.

### Why it is that small, and it is not cleverness

It is the same rule that shows up everywhere else in this repository:

```text
   hardware   ->  PROFILED   named exactly; swapping it swaps a table
   software   ->  CONTRACT   names nobody, so it works for everyone
```

`amdgpu` is millions of lines because it supports fifteen years of cards:
runtime block discovery, dozens of firmware sets, a register map per generation.
**That is the price of being generic, and it is not paid here.** BMO-X carries
one CPU profile, one xHCI, one AHCI. Add no libc, no dynamic linker, no LLVM
runtime and no POSIX layer, and 1,16 MiB is just what is left.

The cost is written down and it is real: **this has been verified on exactly one
machine** -- an AMD Ryzen 5 5600X on an MSI A320M PRO MAX. On yours it may not
boot at all.

> **If it does not boot, that is the single most useful thing you can send me.**
> Photograph the screen where it stops, with your CPU and motherboard, and open
> an issue. A panic from unknown hardware is worth more than a pull request.
> See **[CONTRIBUTING.md](CONTRIBUTING.md)**.

### And one command that only reads

```powershell
.\limpiar.ps1
```

Says what the build trees weigh before you delete anything. Cargo leaves about
5,7 GB across four workspaces. It deletes nothing unless you add `-Borrar`.

---

## Why it is built this way

Five decisions carry the whole design. Each is stated with what it costs,
because a trade-off with only one side written down is advertising.

**Two syscalls, frozen.** `INVOKE` and `WAIT`. Everything else -- opening a file,
reading the mouse, claiming the screen -- is an *operation* on a capability. The
API grows inside the pair (object kind, operation); the ABI does not move. The
proof it works is a number: **the doors went 3 -> 2 while the operations went
22 -> 127.** The system grew and the surface shrank -- and the build prints that
second number on every run (`operaciones kernel<->ABI: 127 comprobadas, ninguna
a mano`), so it is checked, not claimed.
*Cost*: every new ability needs a handle kind to hang from, so there is no quick
way to add "just one syscall".

**Its own compilers.** COBOL, C, Ada and INTI, straight to machine code and
BMO's own `.bex` container. No LLVM, no libc, no linker.
*Cost*: every bug in every language is ours, and a missing corner of C is found
by a program that dies, not by a spec.

**An abstraction may save you work; it may not lie.** An unimplemented feature is
**rejected with a reason, never stubbed to look like it works**. A global the
compiler cannot evaluate is an error rather than a silent zero. When `info`
reports 8.4 MiB, that number comes from the kernel and not from the program
claiming it.
*Cost*: more paths that say no, and more code to say why.

**Writing *is* committing.** ESTRATOS is copy-on-write: the audit trail is not a
logging product bolted on, it is what the filesystem leaves behind. The write
window is **named, per volume**, and the owner's Windows loader is outside every
one of them.
*Cost*: space, and a sealing step that has to be deliberate.

**Survive the fall.** A cat falls, breaks something, and walks away. Not the
absence of failure -- surviving it and being able to say what happened.
*Cost*: the kernel carries autopsy machinery it hopes never to use.

---

<p align="center">
  <img src="docs/arte/inti.png" alt="INTI -- habla con la CPU" width="360">
</p>

## Why INTI exists

**INTI is the system language of BMO-X.** Python's syntax, assembly's control,
and a compiler that will not leave a single operation without a rule. It
compiles straight to `.bex`, it runs on the Ryzen, and its checks reach real
bytes -- not a document.

### C is not dead here. It was demoted.

C still compiles, is still tested, and stays. It is the only way to run the C
that already exists in the world, and that is worth keeping. What changed is
narrower and final: **BMO-X stops writing *new* system code in C.**

The reason is one clause C wrote in 1972 and never revisited. Undefined
behaviour was a *portability* bargain -- dozens of incompatible machines, tiny
compilers, and no honest way to say what a signed overflow meant on all of them
at once. It was the right trade **then**.

Fifty years later the clause had not moved, but what it bought had switched
sides: the compiler stopped reading it as *"anything may happen"* and started
reading it as *"this cannot happen, therefore I may delete the code around it."*
The programmer never agreed to the second reading. He inherited it.

And here the bargain has nothing left to buy: **one machine, one toolchain, and
every source in the tree.** There is no foreign compiler to stay compatible with
and no unknown architecture to hold the door open for. Keeping undefined
behaviour in BMO-X costs everything and purchases nothing.

### The evidence is from this house, not from a paper

The strongest argument for INTI was not argued. It was found:

| what the emulator did | what the silicon actually does |
|---|---|
| `cvttsd2si` **saturated** -- `1e30` gave the largest integer, NaN gave zero | returns the most negative value as a sentinel, for both |
| `imul` **set no flags** | sets `cf` and `of` when the product does not fit |

Both holes sat in the emulator for months while BMO C compiled against them, and
nobody noticed. **They surfaced the week INTI arrived, and the reason is exact:
BMO C never emits a `jo`.** There was no one to ask the question.

On **2026-08-22** the question was put to the real Ryzen 5 5600X. The probe line
`reglas` came back `0x00`: overflow trapped as `1001`, divide-by-zero as `1003`,
bad conversion as `1012`. **The emulator and the metal agree** -- which is the
only way "INTI has no undefined behaviour" stops being a claim about an emulator
we wrote ourselves.

### The cost, stated

The checks cost about **1%**. Three of the four rules that matter reach bytes and
run; the rest are 🟡 or ⚪ and the tracker says which. `INTI PLENO` -- text,
lists, tables, exact decimal -- **is not built**, and the compiler refuses to
emit a signed binary for it rather than emitting one that quietly returns zeros.

> The design is **[INTI_MAESTRO.md](docs/maestro/INTI_MAESTRO.md)**; who executes
> the cut is **[PLAN_EL_SILICIO.md](docs/plan/PLAN_EL_SILICIO.md)**; and
> **[ESTADO.md](toolchain/lang/inti/ESTADO.md)** takes the three claims apart and
> says which of them is paid for.

---

## What it deliberately does not do

No networking stack, no GPU driver, no dynamic linking, no processes talking
over sockets, no package manager, no accounts. Some of those are queued and some
are refused; **[ARQUITECTURA.md](ARQUITECTURA.md)** says which is which and why.

It is also not a Linux, not a hobby OS aiming at POSIX, and not trying to run
anybody else's binaries. A program for BMO-X is compiled for BMO-X.

---

## The second objective: **the day the card arrives, it gets profiled**

The primary objective is banking on BMO-X. This is the one behind it, and it is
stated here because the *shape* of the work is the point, not the hardware.

> **Target: AMD Radeon RX 9060 XT 16GB** (Navi 44 / GFX1200), declared
> 2026-08-02 in `platform/drivers/gpu/rdna4/`. Intended as a **Ring 3** driver
> -- and it would be the **first**: every device driver today is a Rust crate the
> Ring 0 kernel links. That gap is stated, not glossed over.

### Why this is one day of work and not a project

**BMO-X profiles hardware. It does not write generic drivers.** That is the law
of the house, and it is why the estimate is not the one people expect:

```text
   hardware   ->  PROFILE    named exactly; swapping it swaps a table,
                             never an edit to the kernel
   software   ->  CONTRACT   names nobody, and therefore works for everyone
```

`amdgpu` is millions of lines because it supports fifteen years of cards:
runtime block discovery, dozens of firmware sets, a register map per generation,
power management for each. **That is the price of being generic, and it is not
paid here.** What gets written is one device id, one firmware set, one register
map. The same rule the CPU already follows -- *"swapping the CPU is a profile
swap, never a kernel edit"* (`cpu_vendor/profile.rs`).

The profile is also what makes the metal **testable**: it says *this card, this
MAC, this register*, so the answer can be written down before looking and then
compared. A driver that claims "any AMD card" has no experiment that can confirm
or refute it.

** So the arrival of the card is the trigger, and the work is filling in a
profile that is already written and already waiting -- `PROFILE.pci_devices` is
an empty list today, because **a profile refuses to claim a card it has never
met**.

### Why AMD, and it is not a preference

The RTX 3060 came first, and that path was **reverse engineering**. With AMD the
path is open, and that is the whole reason:

| | |
|---|---|
| firmware | published in `linux-firmware`, and **redistributable** |
| the RDNA ISA | **published by AMD** |
| a reference driver | `amdgpu` is open and **can be read** |

That turns the question from *"is it possible"* into *"how much"*.

### Two goals, kept apart on purpose

| | goal A | goal B |
|---|---|---|
| what | **SDMA**, the copy engine | command rings, compute, Vulkan |
| size | *"the size of the AHCI driver"* | the long road |
| why A is small | **the display is not touched**: it inherits the linear framebuffer UEFI left and skips DCN entirely | -- |
| what it buys | the compositor blit -- which at 1600x1000 is DOOM's **entire** deficit | Vulkan in Ring 3 |

Confusing *"make the blit fast"* with *"run Doom Eternal"* is the classic way to
finish neither.

### And the one piece that is honestly not measured

Goal B has a wall: the **PSP**, the security processor that authenticates the
microcode before the GPU will run. A profile shrinks *variants*; it does not
shrink a handshake.

**No number is given for it here, and that is deliberate.** The plan that
describes it says *"do not write a schedule for this until you have looked at
it"*, and the house rule is older than the plan: **a device is asked, never
assumed**. Measuring it costs a day of reading `amdgpu` and counting the steps
-- no hardware, no money -- and until that day the honest answer is *not
measured*, which is not the same as *long*.

The full reasoning: **[PLAN_VULKAN.md](platform/drivers/gpu/rdna4/PLAN_VULKAN.md)**
and **[PLAN_EL_ASISTENTE.md](docs/plan/PLAN_EL_ASISTENTE.md)**.

---

## What is next, and what blocks it

The plans are public because an estimate nobody can check is advertising. Each
row below is **work on top of something that already runs**, except the last one.

| | | blocked by |
|---|---|---|
| 🟡 | **Receive an Ethernet frame** -- the ring is written, the NIC is profiled and its MAC was read on metal | one boot |
| 🟡 | **Give the 12 cores work from Ring 3** -- the door is built (`ATRIL` / `TOCAR`, two operations, a closed catalogue of parts) and the kernel side already measured **11,52x** | one boot: `smp orquesta` has never been executed |
| ⚪ | **A LAN that works and is measured** -- `ping`, files, banking terminals against a local server | the transmit ring, then a TCP stack |
| ⚪ | **Sound** -- volume already reaches the USB headset by control transfer | isochronous transfers in xHCI |
| ⚪ | **A local assistant**, running as a Ring 3 app over your own files | `exp`, and the core door |
| ⛔ | **Anything over the internet** | **cryptography** -- and that is the ceiling |

**The ceiling has a name.** Everything above it is work; cryptography is the one
piece that is an *invention*. X25519, AES-GCM, SHA-256, X.509 -- written wrong it
does not fail, it **works and does not protect**.

And it is the same debt twice: the elliptic curve HTTPS needs is the one a signed
`.bex` needs. **Paying it once collects twice.**

> **[!] This paragraph used to say** *"today `verify_ed25519` says yes to a
> signature of zeros, and nothing calls it yet"*. **That has been false since
> 2026-08-24**, when that function was deleted, and real Ed25519 landed the day
> after. `bmo-firma` verifies against a trust anchor and refuses without one.
>
> The accurate statement is narrower and still uncomfortable: **the machinery
> works and nothing signed ships through it.** Every `.bex` this repository
> produces goes out with `sig_algo = 0` -- integrity, not authorship -- and the
> trust anchor is empty. The gap is no longer the algorithm; it is the key
> ceremony and the anchor, and until those exist a bank cannot use this to prove
> who built a binary.

The full reasoning, with what each piece costs and why:
**[PLAN_EL_PERFIL_TOTAL.md](docs/plan/PLAN_EL_PERFIL_TOTAL.md)** (what this
machine gives without buying anything) and
**[PLAN_EL_ASISTENTE.md](docs/plan/PLAN_EL_ASISTENTE.md)**.

> **On estimates.** A profile here measures between **600 and 1.900 lines** --
> that is four measurements, not an opinion: the Ryzen profile is 952, AHCI is
> 1.103, xHCI is 1.871. Where a number has not been measured, this repository
> says *not measured* rather than guessing. That is not modesty; a guess written
> down becomes a fact three months later.

---

## How to be suspicious of it

The claim worth checking is not "it works" -- it is that the things marked 🟢
were seen on the real machine and the things marked 🟡 say so.

- `.\bmo.ps1` builds everything and runs the guards **without touching any
  disk**: seventeen compatibility rules over 82 cases, sources are ASCII, 1.033
  document citations resolve, no module grew past its ceiling, and the syscall
  contract is compared across the kernel, the ABI and the userland runtime --
  **127 kernel operations and 94 userland ones, none by hand**. Deploying to a
  USB stick is a separate command, `.\desplegar.ps1`, on purpose.
- `.\limpiar.ps1` says what the build trees weigh before you delete them.
- `cargo test --workspace --exclude bmo-kernel` -- **2.433 tests, 0 failures**.
- `cargo test -p bmo-c-front probe_` -- **15 axes in 0,61 s**: the census of what
  the C compiler actually supports, including the rows that are still broken. A
  census that hides its red rows is worth nothing.

**[CONTRIBUTING.md](CONTRIBUTING.md)** explains the bar for a change, and it is
mostly one rule: nothing that compiles and does not do what it says.

---

## Going deeper

This README is the summary on purpose. Everything under it has a document,
because the reason a decision was made is worth more than the decision.

| Document | What is in it |
|---|---|
| ★ **[EL_FUERO.md](EL_FUERO.md)** | **Start here if you want to build on BMO-X.** What the system grants you, what it demands back, and what it deliberately does not grant. Not an SDK -- a charter |
| **[META-KERNEL_HARD.md](META-KERNEL_HARD.md)** | The law of the machine. A rule exists only if it carries the component that demands it and the number it demands |
| **[META-APP_HARD.md](META-APP_HARD.md)** | The law of an app. What BMO-X demands of anything that wants to be one, and what it gives back |
| **[META-SDK_HARD.md](META-SDK_HARD.md)** | The law of **REX**: the nine `<bmo/...>` headers an app is written with, and the two tests that keep a library from becoming a framework |
| **[ARQUITECTURA.md](ARQUITECTURA.md)** | The full technical picture: layout, boot path, the operation table, the allocator, the complete status list |
| **[BITACORA.md](BITACORA.md)** | The build log, episode by episode. **Every bug that cost a day is written down with its root cause** |
| **[AVANCES.md](AVANCES.md)** | What is done, what is waiting for a boot, and the photographs |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | The bar, and what is frozen |
| [Ultra_kernel_x86-64](Ultra_kernel_x86-64/README.md) | Ring 0: boot chain, drivers, filesystems |
| [Ultra_userspace](Ultra_userspace/README.md) | Ring 3: the runtime and the compositor |
| [toolchain](toolchain/README.md) | The four native compilers (C, COBOL, Ada, INTI) and the shared backend |
| [platform](platform/README.md) | The ABI, the `.bex` container, the drivers as crates |
| [docs/](docs/) | The master plans: audio, network, SMP, self-healing, DOOM, RAM |

---

## License

**Apache License 2.0.** Use it, fork it, ship it, sell it. Patent grant
included, and no fee of any kind.

Two things it does *not* hand over, and both are in the licence itself:

- **The name.** Section 6 grants no trademark rights. "BMO-X", "ESTRATOS",
  "BEF", "CABINA" and "INTI" are this project's names -- the code is yours,
  naming your product after this one is not.
- **A warranty.** Sections 7 and 8. It is free, it is not guaranteed, and
  both are true at once.

> **The Base still should not fork -- and that is now a request, not a
> clause.** Two syscalls, BEF, the Ring 0 kernel. Apache 2.0 lets you change
> all three, and the reason not to has not changed: with the Base fixed, one
> audit is worth something to everyone; forked, every audit is worth
> something to one person. That argument now has to stand on its own, which
> is the honest place for it. See **[CONTRIBUTING.md](CONTRIBUTING.md)**.

See **[LICENSE](LICENSE)** and **[NOTICE](NOTICE)**.

---

Built from scratch in Lima, Peru, by **Eddi Salazar**.

- **Repository**: [github.com/AndreeSalazar](https://github.com/AndreeSalazar)
- **Contact**: eddi.salazar.dev@gmail.com
- **Boot report from your own hardware**: see
  **[CONTRIBUTING.md](CONTRIBUTING.md)** -- it is the single most useful thing
  anyone can send, and it settles the licence barter in one email.

<!-- TODO Eddi: el enlace al video del Ryzen arrancando va aqui. Sin el, quien
     lea esto va a asumir QEMU -- que es exactamente lo que el proyecto no es. -->
