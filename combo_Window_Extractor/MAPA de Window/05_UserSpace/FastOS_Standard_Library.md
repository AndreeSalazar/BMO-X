# FastOS Native Standard Library (`libbmo`) Specification
**Capa:** UserSpace (Ring 3)
**Prioridad:** ALTA
**Depende de:** `FastOS_Rust_Runtime_BEF.md`, `FastOS_Syscall_Table_Spec.md`
**Inspiración:** Rust `std`, `musl libc`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
El ecosistema C tradicional depende de `glibc` en Linux y `MSVCRT` (ahora `UCRT`) en Windows. Son bibliotecas gigantescas que envuelven las llamadas al sistema. Cuando usas Rust puro, su librería estándar (`std`) por debajo llama a estas librerías de C, lo que añade otra capa de sobrecarga.
- **Qué conservamos:** La idea de ofrecer una API amigable (`println!`, `File::open`, `Mutex`) para que los desarrolladores no tengan que escribir *inline assembly* (`syscall`) en sus programas.
- **Qué tiramos:** El intermediario en C. **FastOS NO usa libc**. `libbmo` es una librería estándar puramente en Rust que compila estáticamente dentro de cada ejecutable `.bef` y llama directamente a las instrucciones de hardware x86-64 del Kernel.

---

## FASE 2: Diseño BMO Nativo

`libbmo` expone módulos que emulan la ergonomía de la librería estándar, pero con Cero Coste de Abstracción (Zero-cost abstraction) conectados al *FastOS Syscall Table*.

### Módulo IO (Consola y Archivos)
Envolturas limpias sobre el **Pilar 3: VFS** de la Syscall Table.

```rust
// libbmo/src/io.rs
use crate::sys::{sys_write, sys_open, sys_read, sys_close};

pub struct File {
    handle: u64, // El BmoHandle opaco
}

impl File {
    pub fn open(path: &str) -> Result<Self, io::Error> {
        let handle = sys_open(path.as_ptr(), path.len() as u32)?;
        Ok(File { handle })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, io::Error> {
        sys_read(self.handle, buf.as_mut_ptr(), buf.len())
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = sys_close(self.handle); // Cierre seguro al salir del Scope
    }
}

/// Macro estándar reescrito para enviar a stdout
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        let s = format!($($arg)*);
        let _ = sys_write(1, s.as_ptr(), s.len()); // Handle 1 = Stdout
    };
}
```

### Módulo Process & Threads
Envolturas sobre el **Pilar 1: Procesos**.

```rust
// libbmo/src/process.rs
use crate::sys::{sys_spawn_bef, sys_exit};

pub fn spawn(app_path: &str) -> Result<u64, Error> {
    sys_spawn_bef(app_path.as_ptr())
}

pub fn exit(code: i32) -> ! {
    sys_exit(code)
}
```

### Módulo Sync (IPC nativo)
Envolturas sobre el **Pilar 4: IPC**. BMO abandona sockets para usar paso de mensajes entre procesos a velocidad de RAM.

```rust
// libbmo/src/sync.rs
use crate::sys::{sys_send_msg, sys_recv_msg};

pub struct BmoChannel {
    handle: u64,
}

impl BmoChannel {
    pub fn send(&self, payload: &[u8]) {
        let _ = sys_send_msg(self.handle, payload.as_ptr(), payload.len());
    }
    
    pub fn recv(&self, buffer: &mut [u8]) -> usize {
        sys_recv_msg(self.handle, buffer.as_mut_ptr(), buffer.len()).unwrap_or(0)
    }
}
```

### Módulo GPU (Zero-overhead GSP)
El arma secreta de BMO. Expone de manera segura el **Pilar 5** a Ring 3.

```rust
// libbmo/src/gpu.rs
use crate::sys::sys_gsp_command;

pub struct GpuDevice;

impl GpuDevice {
    /// Inyecta un comando RPC directo al Falcon de la RTX 3060
    pub fn submit_rpc(&self, rpc_payload: &[u8]) -> Result<u64, Error> {
        // La syscall devuelve el ID del ticket asíncrono
        sys_gsp_command(rpc_payload.as_ptr(), rpc_payload.len())
    }
}
```

---

## FASE 3: Implementación (Bajo Nivel a Alto Nivel)

El verdadero valor de `libbmo` es ser una barrera de seguridad (*Safe Rust*). El módulo base `libbmo::sys` contiene los mapeos de ensamblador inseguros, mientras que la API que usan los desarrolladores (como `File::open`) es 100% segura e idiomática.

```rust
// libbmo/src/sys.rs (Ejemplo del backend real)
#[inline(always)]
pub fn sys_write(handle: u64, buf: *const u8, len: usize) -> Result<usize, ()> {
    let result: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 0x22,     // SYS_WRITE
            in("rdi") handle,   // Arg 1
            in("rsi") buf,      // Arg 2
            in("rdx") len,      // Arg 3
            out("rcx") _,       // Clobbered por x86-64
            out("r11") _,       // Clobbered
            lateout("rax") result,
            options(nostack, preserves_flags)
        );
    }
    // Conversión de retorno C a Result Rust...
    Ok(result as usize)
}
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Syscall_Table_Spec.md`:** `libbmo` es la cara pública, envolviendo cada uno de los 5 Pilares definidos en el Kernel.
- **Conexión con `FastOS_Rust_Runtime_BEF.md`:** Usa el Global Allocator inicializado por `_start` para habilitar `Vec`, `Box`, `String` en el módulo `alloc`.
- **Conexión con la Experiencia del Desarrollador:** Los ingenieros de juegos solo escribirán `use libbmo::gpu::GpuDevice;` y tendrán control total del hardware sin necesidad de tocar Ring 0 o lidiar con DLLs perdidos.

---

## Conclusión

**Qué aprendimos y mejoramos:**
Evitamos el mayor error histórico de los sistemas operativos C: el infierno de las dependencias dinámicas (DLL Hell / `.so` Hell). `libbmo` no es un archivo externo, es una caja de herramientas en Rust (`libbmo.rlib`) que el BEF Linker incrusta en tu ejecutable. Esto garantiza la filosofía de BMO: 1 aplicación = 1 archivo `.bef` absolutamente auto-contenido, con la barrera de seguridad garantizada matemáticamente por el compilador de Rust, sin sacrificar un solo ciclo de reloj gracias a la compilación estática.
