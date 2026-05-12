# FastOS Security Model Specification
**Capa:** Ecosistema
**Prioridad:** ALTA
**Depende de:** `FastOS_Syscall_Table_Spec.md`, `FastOS_Memory_Manager_Spec.md`
**Inspiración:** Zircon (Fuchsia), seL4 (Capability-based Security).

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Windows basa su seguridad en un sistema bizantino de Listas de Control de Acceso (ACLs), Security Descriptors (SIDs), y Tokens de privilegios (ej. `SeDebugPrivilege`). Linux se apoya históricamente en el modelo Discrecional de Usuarios y Grupos (UID/GID = Root vs User).
- **Qué conservamos:** El estricto control de acceso para que un proceso en Ring 3 jamás toque memoria que no es suya sin pasar por el Kernel.
- **Qué tiramos:** Las ACLs, el concepto de "Usuarios" de UNIX y el Bloatware de permisos. BMO es un sistema de propósito único y alto rendimiento. Implementamos pura **Seguridad Basada en Capacidades** (*Capability-based Security*).

---

## FASE 2: Diseño BMO Nativo

En FastOS, la identidad del que ejecuta el programa no importa. Lo que importa es: ¿Tienes el "Token de Acceso"? En BMO, ese token se llama `BmoHandle`. **Si no tienes el Handle de un recurso, el recurso físicamente no existe para ti.**

### 1. The Handle Table (La Tabla de Capacidades)
Como definimos en la `FastOS_Syscall_Table_Spec.md`, los file descriptors crudos no existen.
```rust
// bmo_security/capabilities.rs

/// Un Handle es una Capacidad fuerte, no un simple int.
#[repr(C)]
pub struct BmoHandle {
    internal_id: u64,
    rights: HandleRights, // Bitmask: Lectura, Escritura, Transferencia, Ejecución
}

/// Cada proceso BEF tiene una tabla aislada de Handles inyectada en su BmoProcessEnv
pub struct BmoHandleTable {
    handles: Vec<Option<KernelResourcePointer>>,
}
```

### 2. Los Niveles de Confianza (Trust Levels)
FastOS define 4 estratos estáticos. La aplicación se encasilla en uno de ellos durante el booteo (`sys_spawn_bef`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// Ring 0 absoluto. El Kernel mismo. Sin restricciones.
    Kernel = 0,
    
    /// Ring 3 Privilegiado. Componentes clave firmados por BMO.
    /// Ej: bmo_dwm.bef (Compositor), bmo (Package Manager).
    System = 1,
    
    /// Ring 3 Estándar. Juegos y Apps de confianza (Descargados y Firmados).
    User = 2,
    
    /// Ring 3 Aislado. Apps sin firma digital o sospechosas.
    Sandbox = 3, 
}
```

---

## FASE 3: Implementación (El Filtro de Syscalls)

El control de seguridad no está esparcido por todo el Kernel. Ocurre exactamente en un solo lugar: El **Syscall Dispatcher** (Ver DOC-11). Antes de enrutar una Syscall, el Dispatcher comprueba el `TrustLevel` del hilo llamador.

```rust
// Modificación al Syscall Dispatcher de Ring 0
pub extern "C" fn bmo_syscall_dispatcher_rust(regs: &mut SavedRegs) {
    let current_process = get_current_process();
    let trust = current_process.trust_level;

    // Validación transversal de Handles (Capacidades)
    // Si la syscall requiere un Handle (Ej: sys_write), el Kernel valida que
    // el Handle de Ring 3 exista en la BmoHandleTable y tenga permisos WRITE.
    if requires_handle(regs.rax) {
        validate_handle(current_process, regs.rdi)?; // RDI contiene el BmoHandle
    }

    let result = match regs.rax {
        // ... (Syscalls de VFS y Procesos, permitidas para todos según Capability) ...
        
        0x40 => { // SYS_GSP_COMMAND
            // RESTRICCIÓN DE SEGURIDAD ABSOLUTA
            if trust == TrustLevel::Sandbox {
                return Err(BmoError::AccessDeniedSecurityPolicy);
            }
            sys_gsp_command_async(regs.rdi as *const u8, regs.rsi as usize)
        },
        _ => Err(BmoError::InvalidSyscall),
    };

    regs.rax = result.unwrap_or_else(|e| e as u64); 
}
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Syscall_Table_Spec.md`:** La validación de Handles ocurre directamente en el Handler de Ring 0, asegurando matemáticamente (Zero-Day proof) que un proceso no puede falsificar un Descriptor de Archivo.
- **Conexión con `FastOS_Window_Compositor.md`:** Al requerir nivel `System` o `User` para ejecutar `sys_gsp_command`, BMO garantiza que un malware en un emulador o sandbox (`TrustLevel::Sandbox`) jamás podrá programar directamente el GSP de la RTX 3060 para leer la VRAM de otros programas.
- **Conexión con `FastOS_Package_Manager.md`:** El PM es el juez principal. Durante `bmo install`, si no encuentra una Firma Digital en el `metadata.toml`, advierte al Kernel marcando el `.bef` para que se ejecute forzosamente en `TrustLevel::Sandbox`.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows/Linux:**
Al basar todo en Capacidades (Handles opacos tipados), no hay forma de que una aplicación adivine el ID de un recurso. Y al eliminar los usuarios y ACLs, reducimos la complejidad del Kernel drásticamente. El modelo de 4 Niveles es inflexible y transparente, diseñado no para servidores multi-usuario corporativos, sino para la seguridad perimetral de un OS Bare-Metal dedicado al altísimo rendimiento sin bloatware de seguridad empresarial.
