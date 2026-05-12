# FastOS Syscall Table & ABI Specification
**Target OS:** FastOS (Bare Metal Orchestrator)
**Architecture:** x86-64 Puro
**Inspiración:** Zircon (Fuchsia), seL4, io_uring. Cero POSIX, cero Win32.

Esta especificación dicta cómo una aplicación en Ring 3 (espacio de usuario) se comunica con el Kernel en Ring 0 utilizando la instrucción de hardware `syscall`. 

---

## 1. FastOS x86-64 Calling Convention (ABI)

FastOS utiliza un sistema de paso de argumentos basado en registros para lograr máxima velocidad, inspirado en SysV pero adaptado a la naturaleza destructiva de la instrucción `syscall`.

### Registro de Llamada:
- **`RAX`**: Número de la Syscall (El ID de la función a llamar).

### Argumentos (Máximo 6):
- **`RDI`** (Arg 1)
- **`RSI`** (Arg 2)
- **`RDX`** (Arg 3)
- **`R10`** (Arg 4) - *Nota: R10 reemplaza a RCX porque RCX es destruido.*
- **`R8`**  (Arg 5)
- **`R9`**  (Arg 6)

### Registros Destruidos (Clobbered):
> [!WARNING]
> La instrucción `syscall` en x86-64 por diseño en hardware altera dos registros:
> - **`RCX`**: Guarda la dirección de retorno (`RIP`) del usuario.
> - **`R11`**: Guarda el estado de los flags (`RFLAGS`).
> **El compilador de aplicaciones BEF debe asumir que RCX y R11 pierden su valor después de un syscall.**

### Retorno:
- **`RAX`**: Contiene el resultado. En FastOS, `RAX` devolverá un `Result<u64, BmoError>`. (Valores negativos indican errores estándar).

---

## 2. Filosofía de Recursos: Zircon-Style Handles

FastOS **NO** utiliza File Descriptors de POSIX (`int fd`). Eso es inseguro y lleva a colisiones de tipo. Todo recurso del Kernel (archivos, memoria de GPU, procesos, canales IPC) se representa como un **`BmoHandle`** fuertemente tipado (un `u64` opaco manejado por la `HandleTable` del `BmoProcessEnv`).

---

## 3. La BMO Core API (Los 5 Pilares)

Windows tiene ~5000 syscalls, Linux ~400. FastOS tiene el mínimo absoluto para mantener seguridad formal y velocidad (estilo seL4).

### Pilar 1: Gestión de Procesos
| ID (`RAX`) | Syscall | Descripción |
| :--- | :--- | :--- |
| `0x01` | `sys_exit(code: i32)` | Termina el proceso BEF actual. |
| `0x02` | `sys_spawn_bef(path_handle: BmoHandle, args: *const u8)` | Lanza un nuevo ejecutable `.bef` y devuelve un handle de proceso. |
| `0x03` | `sys_yield()` | Cede el tiempo de CPU (Scheduler). |

### Pilar 2: Gestión de Memoria (RAM vs VRAM)
> [!IMPORTANT]
> A diferencia de los OS tradicionales, FastOS distingue a nivel de Kernel entre la RAM de la placa madre y la VRAM de la GPU para evitar cache misses fatales.

| ID (`RAX`) | Syscall | Descripción |
| :--- | :--- | :--- |
| `0x10` | `sys_mmap(addr: u64, size: usize, mem_type: MemType)` | Solicita memoria. |

**El `MemType` (Arg 3 en `RDX`):**
- `0x0` = `SystemRAM`: Memoria estándar paginada (Cacheable).
- `0x1` = `GpuVRAM`: Memoria mapeada vía PCIe BAR1 (Uncacheable / Write-Combined), obligatoria para comandos del GSP.

### Pilar 3: VFS (Sistema de Archivos)
Nativo, asíncrono y orientado a objetos, no dependemos de NTFS.
| ID (`RAX`) | Syscall | Descripción |
| :--- | :--- | :--- |
| `0x20` | `sys_open(path: *const u8, flags: u32) -> BmoHandle` | Devuelve un Handle tipado. |
| `0x21` | `sys_read(handle: BmoHandle, buf: *mut u8, len: usize) -> usize` | Lee del VFS. |
| `0x22` | `sys_write(handle: BmoHandle, buf: *const u8, len: usize) -> usize` | Escribe al VFS. |
| `0x23` | `sys_close(handle: BmoHandle)` | Destruye el recurso en el Kernel. |

### Pilar 4: IPC (Message Passing)
Sin sockets de UNIX ni pipes de Win32. Se implementa un paso de mensajes ultra-rápido entre aplicaciones BEF.
| ID (`RAX`) | Syscall | Descripción |
| :--- | :--- | :--- |
| `0x30` | `sys_send_msg(target: BmoHandle, msg: *const u8, len: usize)` | Envía un payload a un canal. |
| `0x31` | `sys_recv_msg(channel: BmoHandle, buf: *mut u8, len: usize)` | Lee del canal IPC. |

### Pilar 5: GSP Graphics (Hardware Asíncrono)
> [!TIP]
> **Inspiración io_uring:** La GPU no responde instantáneamente. `sys_gsp_command` no bloquea el hilo; envía el payload al Ring Buffer DMA de NVIDIA y devuelve un "ticket" (Handle). El app usa polling o interrupciones para saber cuándo la GPU terminó el frame.

| ID (`RAX`) | Syscall | Descripción |
| :--- | :--- | :--- |
| `0x40` | `sys_gsp_command(rpc_payload: *const u8, len: usize) -> BmoHandle` | Somete un comando (ej. `MSG_INIT` o `NV9097_DRAW`) al Falcon y retorna asíncronamente. |

---

## 4. El Dispatcher en el Kernel FastOS (Rust)

Cuando el programa en Ring 3 ejecuta `syscall`, la CPU salta a una dirección física configurada previamente en el registro `MSR_LSTAR`. 
Aquí está el código Rust puro (Inline Assembly) del Trap Handler que salva la vida de los registros antes de entrar a código Rust de alto nivel.

```rust
// syscall_handler.rs (Ring 0)

use core::arch::global_asm;

// 1. EL TRAP HANDLER (Entry Point de Hardware)
global_asm!(r#"
.global syscall_entry_asm
syscall_entry_asm:
    // La instrucción 'syscall' no cambia el stack por defecto.
    // Debemos cambiar de User Stack a Kernel Stack manualmente (SWAPGS).
    swapgs
    mov gs:[USER_RSP_OFFSET], rsp   // Guardamos el stack del usuario
    mov rsp, gs:[KERNEL_RSP_OFFSET] // Cargamos el stack seguro del Kernel

    // Salvamos los registros requeridos para poder retornar al usuario
    push r11  // RFLAGS salvado por hardware
    push rcx  // RIP (Instrucción de retorno al usuario) salvado por hardware
    
    // Salvamos los registros de argumentos por seguridad (C ABI)
    push r9
    push r8
    push r10  // Recordar: R10 es el Arg 4, ya que RCX fue destruido
    push rdx
    push rsi
    push rdi
    push rax  // Número de la syscall

    // Llamamos al dispatcher en Rust de alto nivel
    // RDI, RSI, RDX, R10, R8, R9 ya están en los registros físicos correctos 
    // y cumplen la convención de llamadas C que Rust entiende.
    mov rdi, rsp // Pasamos un puntero a los registros salvados como argumento
    call bmo_syscall_dispatcher_rust

    // Restauramos todo
    pop rax
    pop rdi
    pop rsi
    pop rdx
    pop r10
    pop r8
    pop r9
    pop rcx
    pop r11

    // Regresamos al stack de usuario
    mov rsp, gs:[USER_RSP_OFFSET]
    swapgs

    // Salto cuántico de vuelta a Ring 3
    sysretq
"#);

// 2. EL DISPATCHER DE ALTO NIVEL (Llamado desde ASM)
#[repr(C)]
pub struct SavedRegs {
    pub rax: u64, // Syscall ID
    pub rdi: u64, // Arg 1
    pub rsi: u64, // Arg 2
    pub rdx: u64, // Arg 3
    pub r10: u64, // Arg 4
    pub r8:  u64, // Arg 5
    pub r9:  u64, // Arg 6
    // RCX y R11 están más abajo en el stack
}

#[no_mangle]
pub extern "C" fn bmo_syscall_dispatcher_rust(regs: &mut SavedRegs) {
    let result = match regs.rax {
        0x01 => sys_exit(regs.rdi as i32),
        0x10 => sys_mmap(regs.rdi, regs.rsi as usize, regs.rdx),
        0x20 => sys_open(regs.rdi as *const u8, regs.rsi as u32),
        0x40 => sys_gsp_command_async(regs.rdi as *const u8, regs.rsi as usize),
        _    => Err(BmoError::InvalidSyscall),
    };

    // El resultado se mete en el registro RAX guardado,
    // para que cuando ASM haga 'pop rax', el usuario tenga su respuesta.
    regs.rax = result.unwrap_or_else(|e| e as u64); 
}
```

## Conclusión
La Syscall Table de FastOS es quirúrgica. Abandona 4 décadas de retrocompatibilidad de Microsoft, elimina los riesgos de colisión de File Descriptors usando handles estrictos tipo Zircon, implementa un puente asíncrono (io_uring) directo hacia la NVIDIA RTX 3060, y asegura explícitamente los registros volátiles de x86-64 (`RCX`, `R11`) mediante un Trampoline hiperoptimizado en Rust.
