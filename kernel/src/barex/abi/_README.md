# `barex/abi/` — BMO ABI (19 sub-carpetas)

> Cimiento absoluto de FastOS. **Reemplaza** al C ABI (cdecl / Win64 / SysV
> AMD64) y a su stdlib. Cada nueva línea de código del kernel debe usar
> tipos BMO; el C ABI sólo aparece en `compat/` para hablar con código
> heredado mientras se digiere.

## Mapa visual

```
                          BmoRuntime  ← runtime.rs  (agregador único)
                           ┌────┴────┐
                           │         │
   Capa cimiento ─────────┘          └─── Capa genérica multi-lenguaje
   (sesiones 3-5, 12 carpetas)            (sesión 7-8, 7 carpetas)

   primitives/    ←─ <stdint.h>          type_system/  ←─ RTTI / class
   memory/        ←─ void* size_t        vtable/       ←─ COM / dyn Trait
   string/        ←─ char* wchar_t       closure/      ←─ (no existe en C)
   handle/        ←─ HANDLE / fd          exception/    ←─ Itanium / SEH
   status/        ←─ HRESULT / errno     reflect/      ←─ Java reflect
   calling/       ←─ Win64 / SysV        lang_bridge/  ←─ (no existe)
   async_io/      ←─ OVERLAPPED / IOCP   marshal/      ←─ JNI / P/Invoke
   time/          ←─ time_t
   compat/        ←─ thunks legacy
   sync/          ←─ stdatomic
   option/        ←─ FFI Option
   result/        ←─ FFI Result
```

## Filosofía

- **Cero sigilo.** Cada tipo BMO es `repr(C)` y de tamaño/alineación
  documentados. Cualquier lenguaje con FFI puede consumirlos sin glue.
- **Cero globals.** No hay `errno`, `GetLastError`, `__cxa_*`. Toda la
  información viaja por valor (`BmoStatus` en RAX:RDX) o por handle.
- **Cero legacy hidden.** Si necesitas hablar con Win32 o POSIX vives en
  `compat/`. El resto del kernel asume BMO puro.
- **Genérico por construcción.** Un `TypeDescriptor` describe cualquier
  cosa de cualquier lenguaje. Un `LangDescriptor` registra el lenguaje
  origen. Un `BmoRuntime` agrega todo. Añadir un nuevo lenguaje (incluso
  uno que aún no existe) es: registrar `LangDescriptor` + opcional
  marshaller. Sin tocar el ABI base.

## Cómo añadir un nuevo lenguaje al ecosistema

1. Asignar un ID en [`lang_bridge/ids.rs`](lang_bridge/ids.rs) (rango
   oficial `0x0000_0020+` o experimental `0x8000_0000+`).
2. Crear un `LangDescriptor` con `name`, `version`, `LangFeatures`.
3. Si el lenguaje tiene boxing/tagged values, implementar `Marshaller` en
   [`marshal/`](marshal/).
4. Si el lenguaje tiene runtime managed (GC), wirearlo en `gc_iface/` (sesión futura).
5. El compilador del lenguaje emite BEF con secciones `TypeMap` /
   `LangBridge` / `VTables`. Ya está: corre nativo en FastOS.

## Cuándo se acabará el C ABI

Cuando todos los thunks de [`bef/loader/pe_thunks.rs`](../../bef/loader/pe_thunks.rs)
y [`bef/loader/elf_thunks.rs`](../../bef/loader/elf_thunks.rs) sean
recompilados como BEF nativos. Hasta entonces, `compat/` aísla la
contaminación.
