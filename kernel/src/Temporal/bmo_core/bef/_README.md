# BEF — Mapa de carpetas + estrategia "devour PE/ELF"

BEF es el formato ejecutable de FastOS. Pero su loader puede **devorar** binarios PE (Windows) y ELF (Linux/Unix), traduciéndolos a representación BEF interna en tiempo de carga.

## 📁 Estructura

```
bef/
├── mod.rs              ← entry point + re-exports
├── _README.md          ← este archivo
│
├── header.rs           BefHeader, BefMagic ("BEF1"), BefFlags, BefArch
├── sections.rs         SectionKind (10 tipos), SectionEntry, SectionTable
├── imports.rs          ImportTable + lazy/eager binding
├── exports.rs          ExportTable (bind por nombre o ordinal)
├── relocations.rs      Relocation universal (3 tipos vs 38 de ELF x86_64)
├── symbols.rs          Symbol con kind/binding/visibility (más simple que ELF)
├── manifest.rs         Manifest TOML con capabilities BEF
├── signing.rs          SectionHash BLAKE3 256-bit + verificación
├── tls.rs              TlsTemplate (un solo blob, vs .tdata + .tbss de ELF)
│
└── loader/
    ├── mod.rs          Detector de formato + dispatcher
    ├── native.rs       BEF nativo
    ├── pe.rs           ⭐ DEVOUR PE (.exe/.dll de Windows)
    └── elf.rs          ⭐ DEVOUR ELF (.elf/.so de Linux)
```

## 🌍 Estrategia "devour"

```
   ┌──────────────────────────────────────────────────────────┐
   │  bef::loader::load(bytes)                                │
   │                                                          │
   │  1) Lee primeros 4 bytes (magic):                        │
   │       "BEF1" → loader::native                            │
   │       "MZ"   → loader::pe       (PE/COFF de Windows)     │
   │       0x7F"ELF" → loader::elf   (ELF de Linux/Unix)      │
   │                                                          │
   │  2) Cada sub-loader produce un único `Image`:            │
   │     - secciones canonicalizadas a SectionKind BEF        │
   │     - imports re-resueltos a fake-DLLs BareX (PE) o      │
   │       libc-shim BMO (ELF)                                │
   │     - relocs aplicadas                                   │
   │     - manifest sintetizado con capabilities mínimas      │
   │     - hashes calculados al vuelo                         │
   │                                                          │
   │  3) `Image` se monta con address space BEF estándar      │
   │     y se ejecuta bajo el sandbox unificado.              │
   └──────────────────────────────────────────────────────────┘
```

## 🔁 Devour PE (Windows .exe / .dll)

Source: `loader/pe.rs`. Lee:
- `IMAGE_DOS_HEADER` (`MZ` magic)
- `IMAGE_NT_HEADERS64` (después del PE offset)
- `IMAGE_SECTION_HEADER` × N
- `IMAGE_IMPORT_DESCRIPTOR` array → re-resuelve a fake DLLs (`d3d12.dll` → BareX, etc.)
- Relocaciones tipo `IMAGE_REL_BASED_DIR64`

Devuelve `Image` con `format = BinaryFormat::PeDevoured`.

## 🐧 Devour ELF (Linux .elf / .so)

Source: `loader/elf.rs`. Lee:
- `Ehdr` (ELF header con `0x7F"ELF"`)
- `Phdr` × N (program headers — segments)
- `Shdr` × N (section headers — opcional)
- `DT_NEEDED` para libs dinámicas → re-resuelve a libc-shim BMO
- Relocaciones `R_X86_64_64`, `R_X86_64_PC32`, `R_X86_64_GLOB_DAT`, `R_X86_64_JUMP_SLOT`

Devuelve `Image` con `format = BinaryFormat::ElfDevoured`.

## 🏆 Comparación con PE y ELF

| Característica | PE (Win) | ELF (Linux) | **BEF** |
|---|---|---|---|
| Magic bytes | `MZ` (1990, DOS) | `0x7F ELF` (1989) | `BEF1` (2026) |
| Tipos de relocation | 16 | 38 (x86_64) | **3** (Abs64, Rel32, Got64) |
| Tipos de sección | 11 | ~20 | **10** (cada uno con propósito claro) |
| Header tamaño | 264 B (DOS+PE) | 64 B | **48 B** |
| Manifiesto integrado | ❌ (manifest XML aparte) | ❌ | ✅ TOML inline |
| Capabilities sandbox | ❌ | ❌ | ✅ |
| Shaders pre-compilados | ❌ | ❌ | ✅ SASS nativo |
| Hash por sección | ❌ | ❌ | ✅ BLAKE3 |
| TLS | `.tls` separada | `.tdata` + `.tbss` | ✅ blob único |
| Lazy import binding | sí (PLT/IAT) | sí (PLT/GOT) | ✅ pero usando `BmoHandle` |
| Compresión | ❌ | parcial (.zdebug) | ✅ por sección con GDeflate |
| Code signing | sí (Authenticode) | parcial (extensión) | ✅ Ed25519 + BLAKE3 |

## 🛡️ Lo que NO hereda

- Sin `IMAGE_RESOURCE_DIRECTORY` (los recursos van como sección `.resources`).
- Sin `.eh_frame` / `.eh_frame_hdr` (BEF usa `.unwind` BMO ABI).
- Sin `.dynstr` / `.dynsym` redundantes (un solo `.symbols`).
- Sin `.note.*` (irrelevante).
- Sin `.gnu.version*` (versionado va en manifest TOML).
- Sin `.comment` (debug info en `.debug_bef`).
- Sin LSB/MSB selector en ELF Ehdr (siempre LE en BEF).
