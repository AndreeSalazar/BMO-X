# BMO ABI G — GPU ABI (teoría)

## Concepto
Interfaz formal de GPU en el kernel, análoga a BMO ABI para CPU.
Separa la semántica gráfica del hardware, unificando cualquier API
(D3D, Vulkan, OpenGL, GCM, GNM, NVN, Metal) bajo un mismo ABI.

## Flujo
```
API cualquiera → BMO ABI G → BSF validator → GPU (RDNA4+)
```
- No hay drivers de GPU en user space
- El kernel valida cada comando antes de tocar el HW
- BSF = Binary Shader Format, análogo a BEF para CPU

## Beneficios
1. **Unificación total** — cualquier API compila a BMO ABI G
2. **Validación en kernel** — shaders, memoria, accesos, timeouts
3. **Virtualización nativa** — cada proceso tiene GPU address space virtual
4. **Drivers delgados** — HW init + colas DMA, el resto en Rust
5. **Seguridad** — una app no puede ver VRAM de otra ni saturar la GPU
6. **Catálogo completo** — cualquier juego traducible corre en FastOS
7. **Multi-GPU transparente** — el kernel maneja migración y reparto

## Relación con BareX
- BareX (implementación concreta) se elimina como nombre
- BMO ABI G es la interfaz documentada y estable
- La implementación queda interna en el kernel, intercambiable

## Memoria compartida CPU↔GPU
- BMO ABI exporta `buffer_handle` que BMO ABI G importa
- BMO ABI G exporta `fence`/`semaphore` que BMO ABI espera (futex GPU)
- Scheduler ve CPU y GPU como colas de trabajo peer

## Formato BSF (Binary Shader Format)
- Shaders pre-compilados a ISA de GPU (ej. RDNA4)
- Validación estructural antes de enviar al HW
- Equivalente a BEF pero para GPU
