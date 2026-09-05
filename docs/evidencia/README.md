# Evidence -- BMO-X on real hardware

Photographs and a recording of BMO-X running on the test bench. **Not screen
captures**: a camera pointed at a physical monitor, because the claim is *"this
is not an emulator"* and a screen capture cannot prove that.

**Test bench:** MSI A320M PRO MAX - AMD Ryzen 5 5600X (Zen 3) - Kingston
SA400S37480G SATA - 1920x1080 UEFI GOP.

---

## 1. It is a real boot entry on real firmware

![UEFI boot menu](01-uefi-bmo-x-arranque.jpg)

The motherboard's own boot picker lists `BMO-X (SATA3: KINGSTON SA400S37480G)`
next to `Windows Boot Manager` on the same machine. No virtual machine, no
hypervisor -- the firmware is choosing between two operating systems on physical
disks.

## 2. The kernel enumerates real hardware

![Kernel log](02-kernel-log-usb-enumerado.jpg)

- `[ring0] mem 14 GiB physmap listos` -- physical memory map built
- `[ring0] scheduler preemptivo + capabilities armados` -- scheduler and
  Capability Engine up
- `[usb] xHC pci 0x3:0x0.0x0 mmio=0xFC6A0000` -- a real xHCI controller at a real
  PCI address with its real MMIO window; then a second at `0x2B`
- `[uhid] ... class=0x3 sub=0x1 proto=0x2 -> lo tomo` -- HID descriptors parsed and
  the interface claimed
- `[usb] teclado USB listo` / `[usb] mouse USB listo`

Those PCI addresses and descriptor values are what the hardware answered.

## 3. A desktop, in Ring 3, loaded from disk

![Desktop](03-escritorio-ring3-compositor.jpg)

`gui.bex> escritorio pintado`. The compositor is **not in the kernel** -- it is a
`.bex` loaded from the data volume, painting directly into a framebuffer it
holds as a capability. Changing the desktop does not recompile Ring 0.

![Launcher](04-ventana-ejecutar.jpg)

The launcher: `ruta de un .bex y Enter. info / cpu / mem / ls / lee / reboot.`
This is what makes it an interactive system rather than a boot log.

## 4. The system reports on itself

![System info](05-info-sistema-zen3.jpg)

```
uarch      Zen 3 (Vermeer)  familia 19h/21h
nucleos    6 fisicos / 12 hilos
tsc        3.70 GHz (medido)
total      14.8 GiB   3886100 marcos de 4 KiB
usada      5.4 MiB    [--------------------] 0%
kernel     2.1 MiB    en 0x400000
ranuras    4 en uso de 64
programas  17 lanzados
disco      listo
datos      montado para escritura
```

**5.4 MiB of 14.8 GiB -- 0%.** The TSC frequency is *measured*, not read from a
table. The kernel is 2.1 MiB at a known address.

---

## The three languages, on silicon

This is the part that matters. Each program below was compiled by BMO-X's own
toolchain -- no GCC, no LLVM -- and executed on the Ryzen.

### C -- arithmetic, strings, hex

![C running](06-c-aritmetica-cadenas-hex.jpg)

```
holac.bex> hola desde C en el Ryzen
holac.bex> suma 1..10 = 55
holac.bex> 42-100=-58   100/7=14   100%7=2
holac.bex> cadena=viva hex=beef
           origen  FAT32     leido  12.00 KiB
           firma   FAT32 no puede llevar firma (sin atributos)
holac.bex> C termino ok
```

Signed arithmetic, integer division and modulo (`idiv` with correct sign
extension), `%s` and `%x`. Loaded from a real FAT32 file.

And note the `firma` line: the loader **says why** it cannot verify the
signature instead of skipping silently. FAT32 has no named attributes, so it
cannot carry `:firma` next to the binary. That single line is the reason
ESTRATOS exists.

### COBOL -- interactive, exact decimals

![COBOL calculator](07-cobol-calculadora-accept.jpg)

```
run apps/calc.bex
calculadora COBOL - importes con dos decimales
primer importe:  5
segundo importe: 90
suma:   95.00
resta: -85.00
```

`ACCEPT` reading from the terminal that launched it, in a process that does not
hold the keyboard capability and does not need it.

### COBOL -- a banking batch that writes a file and reads it back

![COBOL batch](08-cobol-batch-escribe-y-relee.jpg)

```
run apps/batch.bex
BATCH DE CIERRE - BANCO BMO
total del dia:
 $1,135.00
cierre escrito en apps/cierre.txt
lee apps/cierre.txt
 1135.00
```

**This is the whole thesis in one screen.** It reads transactions from disk,
totals them in integer cents, formats the total through an edited `PICTURE`
(`$1,135.00` -- currency sign, thousands separator, two decimals, all emitted as
instructions), **writes the close to a file**, and then the file is read back to
show `1135.00` really landed on disk.

Read -> compute -> write a report. That is what banking software is.

### Ada -- the same exact decimal

![Ada](09-ada-cierre-decimal-exacto.jpg)

```
run apps/cierre.bex
CIERRE EN ADA - BANCO BMO
total de tres cuotas:
59.97
tras la devolucion:
39.98
```

**`19.99 x 3 = 59.97`, and `59.97 - 19.99 = 39.98`.** Exact, in integer scale,
no floating point anywhere. The same number COBOL produces, for the same reason:
Ada's Annex F copied COBOL's `PICTURE` rules in 1985, so the decimal work was
already paid for.

Three languages, one binary format, one machine.

---

## Fault recovery, recorded as it happened

![CABINA](10-cabina-revocacion-de-pantalla.jpg)

```
CABINA  eventos=50  perdidos=0
48 INFO  ring3: primer CONSOLE_WRITE: userspace habla
49 INFO  fb: pantalla cedida a Ring 3
50 INFO  input: raton cedido a Ring 3
51 INFO  consola: consola creada para Ring 3
52 WARN  fb: el dueno de la pantalla MURIO: se vuelve al panel del kernel
53 INFO  ring3: proceso termino por su cuenta (EXIT) =2
54 INFO  ring3: proceso termino por su cuenta (EXIT) =6
55 INFO  ring3: proceso termino por su cuenta (EXIT) =3
56 INFO  ring3: proceso termino por su cuenta (EXIT) =4
57 INFO  usb: puerto: ENCHUFADO, nada que adoptar
```

**Line 52 is the most valuable line in this folder.** A Ring 3 process that
owned the framebuffer died, and the kernel **took the screen back** instead of
leaving a dead display. That is capability revocation (`revoke_all` on death)
firing on real hardware, with CABINA recording it at the instant, at WARN level.

`perdidos=0` -- CABINA lost zero events. `ENCHUFADO, nada que adoptar` -- hot-plug
detected and answered *with a reason* rather than in silence.

A system that only ever reports good news is not reporting anything.

---

## 11. And one that shows a bug, on purpose

![DOOM with unpainted columns](11-doom-columnas-sin-pintar.jpg)

2026-09-04. DOOM at 1600x1000 with the status bar and the full view width --
and vertical bands where the **title screen from an earlier frame is still
showing through**. Those columns were never painted: a wall segment that enters
`R_AddLine` with `x1 > x2` is dropped in silence, and what stays on screen is
whatever was there before.

The cause was found in the compiler, not in DOOM: `(angle + ANG90) >> 19` was
not being truncated to 32 bits, so one carry escaped and an index into
`viewangletox[4096]` came out as **9215** -- 5.119 integers past the end of the
table. `9215 = 1023 + 8192`, and `8192 * 2^19` is exactly `2^32`.

It is here because the rule of this folder cuts both ways: a folder that only
ever shows good news is not showing anything.

---

## How this folder works

These are **progress markers, one batch per milestone**. When something starts
running on the Ryzen, it gets photographed and lands here with a name that says
what it shows. Nothing is added because it should work.

### Video: the rule, corrected

This section used to say **"deliberately no video in the repository"**. The rule
was sound and only half of it was: git never forgets a blob, so an *unbounded*
video is a permanent cost -- but a short clip is not. So it becomes a **ceiling**
instead of a ban:

> A clip **under 5 MB** may live here. Anything longer goes on the channel and
> gets linked from this file.

### `15.mp4` -- the boot, edited, with an intro

**2026-08-18. 7,3 s, 848x480, with sound.** An edited presentation piece: the
machine powering up, the firmware's own boot picker, and BMO-X coming up. It has
a title card, which is why it is *this* recording and not the evidence take
described below -- the two are different jobs and the table says which is which.

Committed on 2026-09-05 under the ceiling above, at 1,67 MB. `15-arranque.gif`
(2,1 MB, 480 px, 10 fps, no sound) is the same seven seconds as an animated GIF,
and it exists for one reason: **GitHub does not play a repository `.mp4` inside a
README.** A markdown link opens the file's own page; a GIF plays where it sits.
So the GIF is the one in the README and the MP4 is the one worth watching. Before that it sat
in this folder for two and a half weeks **uncommitted and cited by nobody**,
which is the state this caption exists to end: a recording without a caption is
evidence of nothing.

> [!] **What it is not.** Seven seconds with a title card is a *teaser*, and at
> 848x480 nobody can read a kernel log line off it. It is the right file for the
> top of the README and **the wrong one for the sceptic** -- who is the person
> the next section is written for, and whose take has not been shot yet.

### The two recordings are different jobs -- do not merge them

|  | THE EVIDENCE TAKE | THE PRESENTATION |
|---|---|---|
| for | the sceptic who thinks the stills are faked | somebody who has never heard of this |
| shape | one continuous take, **no cuts** | intro, title, narration, edits |
| content | power on -> BMO-X -> `batch.bex` -> hold ten seconds on the output | the boot, and what is happening while it happens |
| lives | here, or linked here | the README, at the top |

The rules below apply to **the evidence take only**. An intro on that one
destroys the very thing it exists to prove -- a cut is what people suspect. On
the presentation an intro is not a flaw; it is the point.

- Camera on the physical screen, never a screen capture
- One take, no cuts
- Phone quality is fine; **steadiness matters more than resolution**
- No intro, no music, no narration

### What the boot proves, and it is worth saying out loud

The firmware's own picker lists `BMO-X` next to `Windows Boot Manager` -- see
photo 1. **BMO-X is not booted through Ventoy, GRUB, or any other loader**: it
is a UEFI boot entry on its own partition, chosen by the motherboard.

That distinction is not decoration. A system launched from inside a multi-boot
loader can always be waved away as a payload. One the firmware itself boots
cannot.
