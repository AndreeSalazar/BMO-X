# 🎮 ROADMAP — De FastOS/BMO a DOOM y StarCraft

> **Estado actual (post-S19):** Ring 0 + Ring 3 funcionando, compositor
> Hyprland/Win11 con teclado + ratón + sonido + RAMdisk + file I/O.
> **Distancia honesta a juegos reales** documentada abajo.

---

## 🟢 Lo que SÍ tenemos (suficiente para juegos *propios* hechos en BMO)

| Capa                     | Estado | Cómo se usa desde Ring 3 |
|--------------------------|--------|--------------------------|
| Ring 0 protected         | ✅     | GDT, IDT, syscall MSR    |
| Ring 3 user processes    | ✅     | `iretq` desde `user_init` |
| Framebuffer 1920×1080    | ✅     | `FbInfo 0x60`, `FbFill 0x61`, `FbText 0x62`, `FbBlit 0x64` |
| Teclado PS/2 polling     | ✅     | `KeyPoll 0x70`           |
| Ratón PS/2 con paquetes  | ✅     | `MousePoll 0x71`         |
| PC speaker (beep)        | ✅     | `Beep 0x80`              |
| Reloj de alta resolución | ✅     | `ClockGetTime 0x50` (rdtsc) + `NanoSleep 0x51` |
| RAMdisk de assets        | ✅     | `FileOpen 0x20`/`Read 0x21`/`Close 0x23`/`Size 0x25` |
| Debug por serial         | ✅     | `DebugPrint 0xF0`        |
| ProcessExit              | ✅     | `0x00`                   |
| BMO Simple emit (subset) | ✅     | `barex::bmoasm::Emitter` (mov_reg_imm64, syscall, ret, nop) |

Con esto se puede escribir **un juego nativo BMO** (tipo Snake, Tetris,
Breakout, un mini-shooter 2D) directamente desde `desktop/compositor.rs`
o desde un .bmo cargado por `bef::loader`. No hace falta nada más.

---

## 🟡 DOOM (1993) — Lo que falta

**Resumen:** DOOM es un .exe DOS ~700 KB que renderiza 320×200×256 a
software, lee `DOOM1.WAD` (4 MB shareware), usa Adlib/SB16 para sonido
y mouse opcional. Hay puertos modernos (chocolate-doom, Crispy Doom).

### Camino realista (≈4–6 sesiones)

1. **Decisión clave:** NO portamos el .exe original. Portamos **Chocolate Doom 
   en Rust no_std** (≈40k líneas C que se pueden traducir o linkar como
   ELF estático).

2. **C runtime mínimo** (`crt0` BMO) — ya tenemos `alloc` (heap), faltan:
   - `malloc`/`free`/`calloc`/`realloc` → wrapper sobre `alloc::vec`.
   - `memcpy`/`memset`/`memcmp` → trivial.
   - `printf`/`puts` → vía `DebugPrint 0xF0`.
   - `fopen`/`fread`/`fclose`/`fseek` → mapean a syscalls 0x20-0x25.
   - `time`/`gettimeofday` → vía `ClockGetTime 0x50`.
   - `exit` → `ProcessExit 0x00`.

3. **Loader ELF / BEF nativo** — `bef::loader::native::load` está
   stub. Hay que:
   - Mapear secciones `Code`/`RoData`/`Data`/`Bss` en memoria Ring 3
     con permisos correctos.
   - Aplicar las 3 relocations BEF (`Abs64`/`Rel32`/`Got64`).
   - Resolver imports a thunks de C runtime BMO.
   - Saltar a `entry_point`.

4. **WAD loader** — coloca `doom1.wad` en `kernel/src/fs/assets/`,
   declara en `ramdisk.rs::RAMDISK_FILES`. Listo.

5. **Audio mejorado** — Doom suena bien con sólo PC speaker, pero ideal:
   - Driver USB Audio Class (UAC2) → ya hay esqueleto en `drivers/usb/audio_class.rs`.
   - Mixer en `barex/audio/mixer/` listo, sólo falta cablear.

6. **Compilar Chocolate Doom como BEF**:
   ```
   $ rustc --target x86_64-unknown-bmo doom-rs/src/main.rs --emit=bef
   $ cp doom.bef kernel/src/fs/assets/
   $ recompilar kernel
   $ fastos> spawn doom
   ```

**Esfuerzo estimado:** ~30k líneas Rust o port C→Rust + 1k líneas de
kernel (loader BEF + crt0 BMO).

---

## 🔴 StarCraft (1998) — Lo que falta

**Resumen:** StarCraft es un .exe Win32 PE32 ~12 MB que usa:
- **DirectDraw 2** (2D scaled blit)
- **DirectSound 3D** (audio)
- **DirectInput** (mouse/keyboard)
- **DPlay** (multiplayer LAN)
- **Win32 API**: ≈400 funciones (CreateFile, ReadFile, RegOpenKeyEx,
  CreateThread, WaitForSingleObject, GetTickCount, LoadCursor, etc.)
- **MMX**, ~4 MB de instalación + ~600 MB de CD-ROM
- **CD-ROM access** via DeviceIoControl SCSI passthrough

### Camino realista (≈ 12+ sesiones)

1. **PE loader completo** — ya hay esqueleto en `bef/loader/pe.rs`
   con 75 thunks Win32 mapeados (`pe_thunks::THUNK_TABLE`). Falta:
   - Localizar `IMAGE_DIRECTORY_ENTRY_IMPORT` real (hoy heurística).
   - Reescribir el IAT (Import Address Table) con direcciones reales
     de los thunks BMO.
   - Soportar TLS callbacks, SEH unwind, manifest XML.
   - Carga de DLLs (StarCraft trae `storm.dll`, `battle.snp`, ...).

2. **DirectDraw → BareX graphics**:
   - `IDirectDraw7::CreateSurface` → `barex::graphics::BxTexture`.
   - `IDirectDrawSurface7::Blt` → `BxCmdList::copy_texture`.
   - `IDirectDrawSurface7::Lock`/`Unlock` → mapping CPU del recurso.

3. **DirectSound → BareX audio**:
   - `IDirectSound8::CreateSoundBuffer` → `barex::audio::voice::BxVoice`.
   - `IDirectSoundBuffer::Play` → `BxVoice::start`.
   - El mixer en `barex/audio/mixer/` ya lo hace; cablear thunks.

4. **DirectInput → BareX input**:
   - `IDirectInputDevice8::GetDeviceState` → `barex::input::ring::poll`.
   - Trivial gracias a `barex::input::keymap` ya hecho.

5. **Win32 API surface (400+ funciones)**:
   - Las 75 ya stub-eadas en `pe_thunks` son la base.
   - Falta: registry (HKEY_LOCAL_MACHINE → `barex::fs::registry`),
     critical sections, fibers, console, GDI básico (StarCraft usa
     poco GDI, casi todo es DD).

6. **CD-ROM access**:
   - Driver SATA AHCI (`drivers/ahci.rs`) ya lee SATA.
   - Falta SCSI passthrough / DeviceIoControl IOCTL_SCSI_PASS_THROUGH.
   - Alternativa: ripear el CD a archivo `.iso` en el RAMdisk, simular
     CD-ROM virtual.

7. **DPlay multiplayer** — opcional, sólo si se quiere LAN. Ignorable
   para el primer play single-player.

8. **Compatibilidad MMX**:
   - El CPU Ryzen 5 5600X soporta MMX nativo, ya está en `barex::abi::compat`.
   - Setear correctamente CR4.OSFXSR y manejar contexto SSE/MMX en
     ring switch (TODO en `arch/cpu.rs`).

**Esfuerzo estimado:** 50-100k líneas (sólo Win32 surface + DirectX 7).
Wine tardó **20+ años** y aún no ejecuta StarCraft sin fallos. Para
FastOS realista: **portar la lógica de StarCraft** (openra-style) en
lugar de devorar el binario de Blizzard.

---

## 📊 Comparativa de viabilidad

| Juego          | Esfuerzo | Estrategia recomendada                            |
|----------------|----------|---------------------------------------------------|
| Snake/Tetris   | 1 sesión | Escribir en bmoasm directo                        |
| Pacman/Pong    | 2 sesiones | Compositor con sprites + sonido                  |
| **DOOM**       | 4-6 sesiones | Port Rust + WAD en RAMdisk + crt0 BMO       |
| Quake / Quake2 | 8-12 sesiones | Soft renderer + sound mixer                |
| StarCraft      | 30+ sesiones | Win32 + DirectX (o port lógica como OpenRA) |
| Half-Life      | 50+ sesiones | OpenGL stub + Win32 + DirectX               |

---

## 🚀 Próxima sesión sugerida (S20)

**Objetivo:** Cerrar el ciclo "compilar BEF → cargar desde RAMdisk → ejecutar Ring 3".

1. Implementar `bef::loader::native::load` completo (relocs + mapping).
2. Añadir `crt0_bmo.rs` con malloc/free/printf/file ops.
3. Crear un `samples/snake.bef` compilado fuera del kernel.
4. Comando shell `spawn <name>` que lee `name.bef` del RAMdisk, llama
   al loader y hace `iretq` al entry.
5. Snake como **primer juego nativo BMO** ejecutándose en Ring 3.

Una vez hecho eso, DOOM es "cuestión de tiempo + portar Chocolate Doom".
