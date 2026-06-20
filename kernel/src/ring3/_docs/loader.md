# Ring 3 — Loader

> El loader carga ELF y BMO bytecode a Ring 3.

## Estructura

```
loader/
├── mod.rs       — load(path) -> Result<Task, LoaderError>
├── elf.rs       — Parser ELF64
├── bmo.rs       — Parser BMO bytecode
├── region.rs    — Map regions in user space
└── symbols.rs   — Symbol resolver (futuro)
```

## API pública

### `load(path: &str) -> Result<TaskId, LoaderError>`
Carga un programa desde BFS en el path dado. Detecta el
formato (ELF magic o BMO magic) y delega al parser
correspondiente.

Devuelve un TaskId que se puede usar con `schedule::spawn`.

## Formatos soportados

### ELF64

Magic: `\x7FELF` (0x7F, 0x45, 0x4C, 0x46).

Sólo soporta:

- ET_EXEC (no ET_DYN en v1.7.4).
- x86_64 (e_machine = 0x3E).
- PHT_LOAD segments.

Pasos:

1. Leer el header ELF.
2. Leer el program header table.
3. Para cada PT_LOAD:
   - Si p_type == PT_LOAD:
     - Alloc región de p_memsz bytes en p_vaddr.
     - Copiar p_filesz bytes del archivo.
     - Zero el resto (p_memsz - p_filesz).
4. Set entry point a e_entry.
5. Crear el task.

### BMO bytecode

Magic: `0xB0A5_CAFE` (4 bytes LE).

Pasos:

1. Leer header (32 bytes).
2. Alloc y copiar .text a 0x10000.
3. Alloc y copiar .data a 0x20000.
4. Set entry = header.entry + 0x10000.

## Memory setup

Para cada ELF, se hace:

1. Crear nuevo PML4 (zero + copiar kernel half).
2. Mapear el stack (8 MB en 0x0040_0000, NX).
3. Mapear las ELF segments (con permisos correctos).
4. Set el page table del Task al nuevo PML4.

## API interna

### `parse_elf(buf: &[u8]) -> Result<ElfImage, ElfError>`
Parsea un ELF64. Valida:

- Magic.
- e_ident[EI_CLASS] = ELFCLASS64.
- e_machine = EM_X86_64.
- e_type = ET_EXEC.

### `parse_bmo(buf: &[u8]) -> Result<BmoImage, BmoError>`
Parsea un BMO. Valida:

- Magic.
- Version = 0x0001.
- entry < text_sz.
- reloc_sz < total_sz.

### `setup_user_memory(pml4: u64, image: &Image) -> Result<(), LoaderError>`
Map las regiones de la image en el PML4 user.

### `create_task(image: &Image, pml4: u64) -> Result<Task, LoaderError>`
Crea el Task struct con:

- context[0] = entry
- stack_ptr = 0x0080_0000 - 16
- page_table = pml4
- priority = 5

## Errores

```rust
pub enum LoaderError {
    FileNotFound,
    InvalidMagic,
    UnsupportedFormat,
    OutOfMemory,
    InvalidElf,
    InvalidBmo,
    BadPermissions,
    BadAddress,
}
```

## Limitaciones v1.7.4

- Sin dynamic linking.
- Sin relocations (sólo ET_EXEC estáticos).
- Sin thread-local storage.
- Sin position-independent executables.
- Sin BMO con relocations (todas las direcciones son absolutas).
- Sin verificación de firmas (cualquier ELF se ejecuta).

## Debugging

Si el ELF falla al cargar, se loguea a serial:

```
[loader] failed to load /init.elf: InvalidElf (e_type=3)
```

Y el kernel panic si es el init.

Si el ELF carga pero crashea en runtime, se loguea:

```
[ring3] page fault at 0x0000_0000_1234, err=4
[ring3] killing pid 1 (SIGSEGV)
```
