# Memory API (`ring0::memory`)

> API de gestión de memoria: heap, page allocator, paginación,
> y virtual memory manager (VMM).

## Estructura

```
memory/
├── mod.rs        — init() del Memory API
├── heap.rs       — Bump allocator + free list
├── page_alloc.rs — Bitmap allocator (4 KB frames)
├── paging.rs     — Page table walker
└── vmm.rs        — Virtual Memory Manager (map/unmap regions)
```

## API pública

### `init()`
Inicializa en orden:
1. `page_alloc::init(phys_base, frame_count)` — bitmap en RAM libre.
2. `heap::init(virt_base, size)` — bump allocator en virtual space.
3. `paging::init()` — load CR3 con PML4 actual.
4. `vmm::init()` — registra kernel space y user space.

Tras init, las 4 funciones públicas del módulo funcionan.

### `heap::malloc(size: usize) -> *mut u8`
Aloca `size` bytes en el heap. Usar `core::alloc::GlobalAlloc`
como wrapper si se quiere usar `Box`, `Vec`, etc. En v1.7.4 NO
está conectado a `#[global_allocator]` (intencional: el heap
es opcional y el kernel funciona sin él).

### `heap::free(ptr: *mut u8)`
Libera memoria. Usa free-list para tracking de bloques libres.
Actualmente se usa poco; la mayoría del código es estático.

### `page_alloc::alloc_frame() -> Option<PhysAddr>`
Asigna un frame físico de 4 KB. Devuelve `None` si no hay
frames disponibles.

### `page_alloc::free_frame(addr: PhysAddr)`
Libera el frame en `addr`.

### `page_alloc::total() -> u64`
Devuelve el total de frames disponibles.

### `page_alloc::used() -> u64`
Devuelve los frames en uso.

### `paging::map_page(virt: u64, phys: u64, flags: PageFlags)`
Mapea `virt` → `phys` con `flags` (R/W, NX, PWT, PCD, etc).
Modifica el PML4 actual. Usado por `vmm::map`.

### `paging::unmap_page(virt: u64) -> Option<u64>`
Desmapea `virt`. Devuelve la dirección física previa (o None).

### `paging::translate(virt: u64) -> Option<u64>`
Devuelve la dirección física mapeada en `virt` (o None).

### `vmm::map_region(base: u64, size: usize, flags: PageFlags)`
Mapea una región contigua con páginas de 2 MB (huge pages).
Usado para mapear kernel space, MMIO, etc.

### `vmm::unmap_region(base: u64, size: usize)`
Desmapea una región contigua.

## Memory layout detallado

```
Virtual (ring 0 kernel space):
0xFFFF_8000_0000_0000 ─┐
                       │  kernel image (.text, .rodata, .data, .bss)
0xFFFF_8000_0100_0000 ─┘
0xFFFF_8000_1000_0000 ─── heap (bump allocator, 16 MB)
0xFFFF_8000_2000_0000 ─── page_alloc bitmap
0xFFFF_8000_3000_0000 ─── VMM metadata

Virtual (ring 3 user space):
0x0000_0000_0000_0000 ─── user code
0x0000_0000_0040_0000 ─── user stack
0x0000_0000_0080_0000 ─── user heap

Virtual (MMIO):
0xFFFF_9000_0000_0000 ─── APIC
0xFFFF_A000_0000_0000 ─── ECAM
0xFFFF_B000_0000_0000 ─── GOP framebuffer
```

## Cómo añadir un nuevo allocator

1. Crear `memory/<nombre>.rs`:
   ```rust
   pub fn init() { /* ... */ }
   pub fn alloc(size: usize) -> Option<*mut u8> { /* ... */ }
   pub fn free(ptr: *mut u8) { /* ... */ }
   ```
2. Agregar `pub mod <nombre>;` en `memory/mod.rs`.
3. Llamar `<nombre>::init()` en `memory::init()`.

## Reglas

- **NO** usar `heap::malloc` desde handlers de interrupción.
- **NO** usar `page_alloc::alloc_frame` desde handlers.
- **SÍ** usar `paging::translate` antes de tocar punteros
  que vienen de Ring 3.

## Bug histórico importante

En v1.6.6, `map_kernel_mmio_huge()` para ECAM causa `#PF` en
Ryzen 5 5600X. Por eso `pci::init_ecam` está definido pero
**no se llama** desde `phase2_devices`. El ECAM se descubre
en `acpi.rs` y se guarda en `BootContext.devices.acpi_mcfg_base`,
pero el mapeo real se difiere a v1.8.0.
