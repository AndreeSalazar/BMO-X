# FastOS / BMO

FastOS es un sistema operativo propio en Rust para bare metal. BMO significa
**Bare Metal Orchestrator**: el núcleo que organiza hardware, memoria, entrada,
gráficos, procesos y las futuras APIs de compatibilidad.

El objetivo actual es simple y honesto: **arrancar bien, pintar bien y responder
bien usando UEFI GOP/framebuffer**. No hay driver oficial de fabricante, no hay
firmware de GPU funcional en el camino de arranque y no se debe depender de una
GPU concreta para que el OS funcione.

---

## Estado real del proyecto

### Funciona / es el camino principal

- Boot UEFI propio.
- Kernel `no_std` en Rust, Ring 0.
- BootInfo compartido entre bootloader y kernel.
- Framebuffer por UEFI GOP.
- Consola/framebuffer inicial.
- Pantalla de bienvenida.
- Escritorio Ring 0 por GOP.
- Entrada PS/2 básica para teclado y ratón.
- GDT, IDT, TSS y MSR de syscalls inicializados.
- Page allocator inicial.
- Enumeración PCI por ACPI MCFG/ECAM.
- Estructura base de BMO ABI, BEF, scheduler, syscall y BareX.

### No es objetivo activo ahora

- Driver propietario/oficial de GPU.
- Driver acelerado real para una GPU concreta.
- Firmware de GPU como requisito de arranque.
- Aceleración 3D por GPU.
- WDDM real.
- CUDA, Vulkan o DirectX nativo sobre hardware real.

La carpeta `kernel/src/drivers/gpu/fastgpu/` queda como experimento/legado de
investigación. No forma parte del camino estable de arranque. El camino gráfico
oficial de FastOS por ahora es:

```text
UEFI GOP -> framebuffer -> desktop Ring 0 -> futura capa BareX software/GOP
```

---

## Arquitectura objetivo

```text
Bootloader UEFI
  -> carga kernel ELF
  -> entrega BootInfo + framebuffer GOP

Kernel Ring 0
  -> serial, GDT/TSS, IDT, syscalls
  -> memoria, ACPI/PCI, timers
  -> framebuffer GOP
  -> welcome + desktop Ring 0

BMO ABI
  -> contrato estable para servicios del OS
  -> base para syscalls, handles, memoria, strings, tiempo y sincronización

BareX
  -> API gráfica/audio/input/net propia
  -> descendiente conceptual de DirectX, pero diseñada para FastOS
  -> primero backend GOP/software; después backends acelerados si existen

Compatibilidad futura
  -> ideas de Windows: Win32/PE/thunks cuando sean útiles
  -> ideas de Linux: syscalls simples, VFS, drivers pequeños
  -> ideas de macOS: compositor pulido, experiencia visual consistente
```

---

## Decisión de GPU

FastOS debe funcionar aunque no exista ningún driver de GPU dedicado. Por eso la
decisión técnica actual es:

1. **GOP primero**: todo lo visual debe poder dibujarse en framebuffer.
2. **BareX no depende de un fabricante**: BareX debe tener backend software/GOP antes de
   cualquier backend acelerado.
3. **GPU real después**: si algún día se implementa un driver acelerado, será un
   backend opcional, no el cimiento del sistema.
4. **Sin firmware obligatorio**: el arranque no debe requerir blobs privados ni
   payloads de GPU.

Esto hace que BMO/FastOS sea más fácil de probar, depurar y mantener.

---

## BareX: API descendiente de DirectX

BareX es la API moderna de FastOS. La idea es tomar lo mejor de DirectX, pero sin
copiar su dependencia de Windows ni de WDDM.

Capas deseadas:

- `barex::graphics`: device, queue, swapchain, buffers, textures, fences,
  root-signature/PSO y comandos.
- `barex::audio`: mixer, voces, formatos, latencia, backends.
- `barex::input`: teclado, ratón, HID, gamepad, eventos.
- `barex::net`: sockets, DNS, HTTP, QUIC/TLS a futuro.
- `barex::shader`: IR propia y traducción futura.
- `barex::compat`: thunks para Win32/PE/DirectX cuando el núcleo ya sea estable.

Orden correcto para BareX graphics:

1. Backend GOP/software: rectángulos, blit, texto, cursor, ventanas.
2. Swapchain simple sobre framebuffer.
3. Compositor estable con input.
4. API pública limpia de `barex::graphics`.
5. Compatibilidad tipo DirectDraw/DXGI/D3D12 encima de BareX.
6. Backend acelerado opcional sólo cuando haya un driver real.

---

## BMOasm: lenguaje tipo Python en español

BMOasm será el lenguaje propio para crear servicios, pruebas, herramientas y
apps pequeñas de FastOS sin escribir todo en Rust o ensamblador. Aunque se llame
`asm`, la dirección correcta no es hacerlo como ensamblador crudo, sino como un
lenguaje **alto nivel, indentado, UTF-8 y compilable a BMO IR/BEF**.

Objetivo de sintaxis:

- Parecida a Python: bloques por indentación, lectura simple y poco ruido.
- En español: palabras clave claras para aprender y escribir más natural.
- UTF-8 real: identificadores con `ñ`, acentos y nombres humanos.
- Determinista: sin magia pesada en Ring 0; lo crítico se valida antes de correr.
- Compilación futura a BEF/BMO ABI, no ejecución directa dentro del boot path.

Ejemplo deseado:

```python
función saludar(nombre):
    si nombre == nulo:
        devolver "Hola desde FastOS"

    devolver "Hola, " + nombre

función principal():
    año = 2026
    dueño = "André"

    para intento en rango(0, 3):
        imprimir(saludar(dueño))

    si año >= 2026:
        imprimir("BMO está vivo")
```

Palabras clave candidatas:

- `función`, `devolver`, `si`, `sino`, `mientras`, `para`, `en`
- `intentar`, `capturar`, `finalmente`
- `usar`, `desde`, `como`
- `verdadero`, `falso`, `nulo`
- `romper`, `continuar`, `pasar`

Reglas iniciales recomendadas:

1. Lexer UTF-8 con identificadores Unicode: `año`, `niño`, `señal`, `tamaño`.
2. Parser por indentación, estilo Python, sin llaves obligatorias.
3. Tipos opcionales al principio; inferencia simple después.
4. Runtime pequeño encima de BMO ABI: `imprimir`, tiempo, memoria, archivos BEF.
5. Modo kernel restringido: nada de memoria arbitraria sin permiso explícito.
6. Modo usuario primero: BMOasm debe servir para probar syscalls y BareX sin
   arriesgar el arranque.

Ruta práctica:

```text
BMOasm fuente UTF-8
  -> lexer/parser español
  -> AST
  -> BMO IR simple
  -> BEF ejecutable o bytecode verificado
  -> syscalls/BareX por BMO ABI
```

---

## Limpieza y enfoque

Para que el proyecto avance, hay que separar tres grupos:

### Núcleo estable

Debe compilar y arrancar siempre:

- `bootloader/`
- `boot_protocol/`
- `kernel/src/main.rs`
- `kernel/src/arch/`
- `kernel/src/desktop/`
- `kernel/src/fb.rs`
- `kernel/src/console.rs`
- `kernel/src/drivers/gop.rs`
- `kernel/src/drivers/serial.rs`
- `kernel/src/drivers/pci.rs`
- `kernel/src/fs/ramdisk.rs` cuando se use de verdad

### API en construcción

Se conserva, pero no debe bloquear el arranque:

- `kernel/src/barex/`
- `kernel/src/bef/`
- `kernel/src/sched/`
- `kernel/src/syscall/`
- `kernel/src/sandbox/`

### Investigación / legado

No debe ser requisito del boot path:

- `kernel/src/drivers/gpu/fastgpu/` queda fuera del build activo
- scripts de firmware/payload GPU
- material extraído de Windows en `combo_Window_Extractor/`
- documentos antiguos que prometen drivers GPU como si ya fueran funcionales

---

## Próximos ataques recomendados

### 1. Estabilizar boot + escritorio GOP

- Mantener `Run -> Desktop` sin depender de Ring 3.
- Quitar mensajes falsos de driver GPU en UI y scripts.
- Hacer que el desktop tenga una ruta clara de salida/reinicio.
- Reducir parpadeos, bloqueos de input y dependencias de hardware específico.

### 2. Convertir el escritorio actual en backend BareX GOP

- Extraer primitivas actuales de framebuffer como backend `BareXGopBackend`.
- Mantener API pequeña: fill, blit, text, present, poll input.
- No crear abstracciones grandes hasta que se usen.

### 3. Completar syscalls mínimas útiles

- `DebugPrint`
- `ClockGetTime`
- `NanoSleep`
- `FbInfo`
- `FbFill`
- `FbBlit`
- `KeyPoll`
- `MousePoll`

### 4. Ring 3 sólo cuando el scheduler pueda sostenerlo

El compositor Ring 3 no debe prometerse hasta que existan:

- stacks seguros por thread,
- transición syscall/sysret validada,
- dispatcher real,
- scheduler que pueda volver al kernel sin congelar la UI.

### 5. Compatibilidad Windows/Linux/macOS por capas, no mezclada en el kernel

- Win32/PE/DirectX: capa `barex::compat`, no boot path.
- Linux-like: syscalls simples y VFS limpio.
- macOS-like: compositor visual y UX pulida.

---

## Build

El flujo esperado es usar el script UEFI del repo:

```powershell
.\build_uefi.cmd
```

`build_uefi.cmd` llama a `build_uefi.ps1` con `ExecutionPolicy Bypass` sólo para
esa ejecución, así no necesitas cambiar la política global de Windows.

Requisitos habituales:

- Rust nightly según `rust-toolchain.toml`.
- Target bare metal configurado por `.cargo/config.toml`.
- Firmware UEFI con Secure Boot desactivado durante pruebas.
- GOP disponible en firmware.

---

## Principio del proyecto

FastOS no debe intentar ser Windows, Linux o macOS completos desde el primer día.
Debe tomar ideas buenas de los tres y convertirlas en un OS propio:

- pequeño,
- arrancable,
- depurable,
- gráfico por GOP,
- con BMO ABI estable,
- con BareX como API moderna,
- y con compatibilidad futura encima, no dentro del núcleo crítico.
