# FastOS Window Compositor Specification
**Capa:** UserSpace (Ring 3)
**Prioridad:** ALTA
**Depende de:** `FastOS_Standard_Library.md`, `BMO_Graphics_Layer_Spec.md`, `FastOS_Syscall_Table_Spec.md`
**Inspiración:** Protocolo Wayland, Windows DWM (Desktop Window Manager), macOS Quartz.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Windows utiliza un modelo híbrido heredado (GDI + USER32 + DWM) donde el concepto de `HWND` (Handle de Ventana) está incrustado profundamente en Ring 0 (`win32k.sys`). Linux en el pasado dependió de X11, un servidor monolítico gigantesco, pero se está moviendo hacia Wayland (donde el cliente y el compositor se comunican limpiamente).
- **Qué conservamos:** El modelo de Wayland: El cliente (App) es responsable de dibujar sus propios píxeles en su propio buffer, y el Compositor solo mezcla y manda a la pantalla (*Flip*).
- **Qué tiramos:** Servidores X, API WIN32 (`HWND`, `DefWindowProc`, `WNDCLASS`), y cualquier elemento gráfico dentro de Ring 0. El Kernel de FastOS **NO** sabe qué es una ventana, ni una fuente, ni un botón.

---

## FASE 2: Diseño BMO Nativo

El Compositor BMO (`bmo_dwm.bef`) es simplemente otra aplicación ejecutándose en Ring 3, pero es una aplicación *Privilegiada*. 

### Arquitectura Core-Dedicada (SMP)
Dado que el Ryzen 5600X tiene 6 núcleos físicos (12 lógicos), el Compositor se aísla en su propio núcleo dedicado usando afinidad (`cpu_core_id` en el Scheduler). Esto garantiza animaciones bloqueadas matemáticamente a 60/144/240 FPS sin que un juego pesado le robe ciclos de CPU al proceso de pintado de la UI del sistema.

### El Modelo `BmoSurface`
Una aplicación nunca pide dibujar una línea. Pide un `Surface` (una región de memoria de video).

```rust
// bmo_ui/surface.rs
use libbmo::sync::BmoChannel;
use libbmo::sys::BmoHandle;

pub struct BmoSurface {
    pub surface_id: u32,
    pub width: u32,
    pub height: u32,
    // Dos buffers en la VRAM de la RTX 3060 para Double Buffering
    pub front_buffer_handle: BmoHandle, 
    pub back_buffer_handle: BmoHandle,
}

pub struct BmoWindowClient {
    compositor_ipc: BmoChannel, // Canal IPC hacia bmo_dwm.bef
}

impl BmoWindowClient {
    /// Pide al compositor que nos de un área para dibujar
    pub fn create_surface(&self, w: u32, h: u32) -> Result<BmoSurface, Error> {
        let req = IpcMessage::CreateSurface { width: w, height: h };
        self.compositor_ipc.send(&req.as_bytes());
        
        // Esperamos que el compositor nos devuelva los handles de la VRAM
        let mut resp_buf = [0u8; 128];
        self.compositor_ipc.recv(&mut resp_buf);
        parse_surface_response(&resp_buf)
    }

    /// Avisa al Compositor que terminamos de dibujar en nuestro Back Buffer
    pub fn commit_frame(&self, surface_id: u32) {
        let msg = IpcMessage::CommitFrame { surface_id };
        self.compositor_ipc.send(&msg.as_bytes());
    }
}
```

---

## FASE 3: Implementación (El Loop del Compositor)

El Compositor `bmo_dwm.bef` es el maestro de la pantalla. Recoge todos los *Commits* de las apps y ordena al GSP (NVIDIA) fusionarlos usando el hardware 2D/3D.

```rust
// bmo_dwm.bef (Código del Compositor)
use libbmo::gpu::GpuDevice;
use libbmo::sync::BmoChannel;
use bmo_gfx::BmoGfxApiVTable;

fn compositor_main() {
    let gpu = GpuDevice::init_privileged();
    let gfx_api = BmoGfxApiVTable::load(&gpu);
    let app_listener = BmoChannel::open_server("bmo:dwm");

    let mut surfaces: Vec<SurfaceState> = Vec::new();

    loop {
        // 1. Escuchar peticiones de las apps via IPC (sys_recv_msg)
        while let Some(msg) = app_listener.try_recv() {
            match msg {
                IpcMessage::CommitFrame { surface_id } => {
                    // Marcamos que esta app ya tiene un frame fresco
                    mark_surface_dirty(&mut surfaces, surface_id);
                }
                // ... manejar CreateSurface
            }
        }

        // 2. Fusionar todos los surfaces de abajo hacia arriba (Z-Index)
        for surface in surfaces.iter().filter(|s| s.is_dirty) {
            // Utilizamos el hardware de la RTX 3060 para mezclar (Alpha Blending)
            gfx_api.blend_texture(
                surface.back_buffer_handle, // Origen (El buffer recién pintado por la app)
                SCREEN_FRAMEBUFFER,         // Destino (La pantalla)
                surface.x, surface.y
            );
        }

        // 3. Fliping Final a la pantalla (Evita Tearing)
        gfx_api.present_frame();
        
        // 4. Dormir este core hasta el siguiente VSync de la pantalla (ej. 144Hz)
        wait_for_vsync(); 
    }
}
```

---

## FASE 4: Integración con el Stack FastOS

Este documento cristaliza el trabajo de todos los anteriores:
1. **Conexión con la Syscall Table (Pilar 4):** Las apps no le mandan píxeles al compositor (es muy lento). Usan `sys_send_msg` (IPC) a través de `libbmo` para mandarle una notificación instantánea de: "Oye, ya terminé de usar mi bloque de la GPU, puedes mostrarlo".
2. **Conexión con el Scheduler (DOC-03):** Al ejecutar el Compositor en el Core 5 del Ryzen, y usar `sys_yield` / `wait_for_vsync`, evitamos que un juego al 100% en el Core 0 congele la animación de las ventanas.
3. **Conexión con la Capa Gráfica (`BMO_Graphics_Layer_Spec.md`):** El Compositor es el único que invoca la función final `present_frame()` (El equivalente GSP al *SwapBuffers*). Las apps hijas solo pintan texturas.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
Le quitamos el trabajo gráfico al Kernel. En Windows, si el driver de video crashea en `win32k.sys`, toda la PC tira una Pantalla Azul (BSOD). En FastOS, el Compositor es un programa `.bef` normal de espacio de usuario. Si la UI crashea, solo el proceso `bmo_dwm.bef` muere; el Kernel sigue intacto, se reinicia el compositor en milisegundos, y las aplicaciones de fondo (como un servidor web o una descarga) ni siquiera lo notan. La combinación de *Wayland Philosophy + Rust IPC + Dedicated CPU Core* crea un entorno de usuario inmune al lag visual.
