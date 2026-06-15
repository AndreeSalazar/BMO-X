# Legacy Archive

Archivos y módulos que fueron parte del kernel activo pero fueron retirados del build.
No compilan ni se invocan en el boot path.

## Contenido

### `shell.rs` (10 KB)
- **Qué era**: Shell interactivo en Ring 0 con comandos (help, cpuinfo, pci, meminfo, etc.)
- **Por qué se archivó**: Nunca se llamó desde `main.rs`. El flujo de boot va directo a `desktop::welcome::run()`.
- **Fecha de retiro**: 2026-06-15
- **Dependencias rotas**: Ninguna. No era importado por ningún otro módulo.

### `gpu/fastgpu/` (21 archivos, 50 KB)
- **Qué era**: Driver experimental de GPU NVIDIA (RTX 3060) con GSP firmware, falcon coprocessor, SEC2 engine.
- **Por qué se archivó**: Excluido del build intencionalmente. El backend gráfico estable es UEFI GOP/framebuffer.
- **Fecha de retiro**: 2026-06-15
- **Dependencias rotas**: Ninguna. `drivers/mod.rs` no declaraba `pub mod gpu;`.

## Restauración

Para restaurar cualquier módulo archivado:
1. Copiar de vuelta a `kernel/src/`
2. Re-agregar la declaración `mod` en el módulo padre
3. Verificar que no haya conflictos de imports
4. Ejecutar `cargo check` para validar
