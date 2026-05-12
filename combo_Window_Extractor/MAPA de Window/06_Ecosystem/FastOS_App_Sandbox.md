# FastOS App Sandbox Specification
**Capa:** Ecosistema
**Prioridad:** MEDIA
**Depende de:** `FastOS_Security_Model.md`, `FastOS_Memory_Manager_Spec.md`
**Inspiración:** Android Permissions, Fuchsia Sandbox (Capabilities), sin la burocracia de los *Manifests* inmensos.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Windows corre los programas con el nivel de confianza completo del usuario logueado. Si bajas un `.exe` con virus y lo abres, tiene acceso a leer todo tu Disco C:, tus contraseñas y tu registro. Para mitigar esto, Windows inventó *AppContainer* y Linux tiene *cgroups/namespaces* (Docker), que son complejos y requieren capas extra de virtualización.
- **Qué conservamos:** La idea de una "cárcel" perimetral (aislamiento) donde una aplicación no verificada puede correr sin poder lastimar al host.
- **Qué tiramos:** Los sistemas de *Namespaces* masivos, Docker-like containers, y emulaciones de red. El Sandbox de FastOS es puramente a nivel de Memory Manager y filtrado de Syscalls.

---

## FASE 2: Diseño BMO Nativo

El Sandbox en FastOS no es una máquina virtual, es simplemente una etiqueta (`TrustLevel::Sandbox`) asignada a una estructura `BmoProcess` y aplicada con guante de hierro por Ring 0.

Cualquier app instalada vía `.bpkg` que **carezca de una Firma Digital válida** es etiquetada como Sandbox por defecto durante el proceso `bmo install`.

### Restricciones Estrictas de Sandbox
1. **Gráficos (Hardware Level):** La syscall `sys_gsp_command` está prohibida. El malware no puede reprogramar la RTX 3060.
2. **Memoria:** `sys_mmap` denegará cualquier petición de `MemType::GpuVRAM`. 
3. **VFS (Chroot Nativo):** Un proceso Sandbox solo recibe un `BmoHandle` a su propio directorio (ej. `/system/apps/juego_dudoso/`). Las llamadas a `sys_open` con rutas absolutas como `/system/` devuelven `AccessDenied`.
4. **IPC:** Puede hacer `sys_send_msg` hacia el Compositor (para dibujar la ventana), pero no puede conectarse vía IPC a otras aplicaciones de Ring 3, a menos que la otra aplicación haya expuesto un canal IPC público como "Abierto".

---

## FASE 3: Implementación (Aislamiento Puro en Hardware)

La verdadera cárcel no es lógica, es física. El Memory Manager (`FastOS_Memory_Manager_Spec.md`) y el hardware x86-64 son los verdaderos guardianes del Sandbox.

```rust
// bmo_kernel/process_loader.rs

pub fn spawn_sandbox_process(app_bef: &[u8]) -> Result<u64, Error> {
    // 1. Crear una Tabla de Páginas (PML4) ABSOLUTAMENTE NUEVA
    let new_pml4 = memory_manager::create_empty_page_table();
    
    // 2. Mapear el Upper Half (Kernel Space) idéntico para las syscalls
    memory_manager::map_kernel_space(&mut new_pml4);
    
    // 3. Crear el proceso con la etiqueta Sandbox
    let mut process = BmoProcess {
        trust_level: TrustLevel::Sandbox,
        pml4_table: new_pml4,
        handle_table: BmoHandleTable::new_empty(), // NO HEREDA NINGÚN HANDLE
        // ...
    };
    
    // 4. El VFS inyecta SOLAMENTE el handle del directorio local de la app
    let local_dir = vfs::get_local_app_dir(app_id);
    process.handle_table.insert(local_dir);
    
    // 5. El Kernel inyecta el handle pre-aprobado para hablar con el Compositor
    let dwm_ipc = ipc::get_public_channel("bmo:dwm");
    process.handle_table.insert(dwm_ipc);
    
    // 6. Lanzar la app (Scheduler)
    scheduler::enqueue(process);
    
    Ok(process.id)
}
```

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `FastOS_Memory_Manager_Spec.md`:** El hardware x86-64 se encarga de la seguridad. Al tener su propio `CR3` (Page Table), es físicamente imposible que un proceso Sandbox lea la RAM de otro programa. No hay instrucción en ensamblador que lo permita sin colapsar en un *Page Fault*.
- **Conexión con `FastOS_Security_Model.md`:** Ejemplifica la aplicación de `TrustLevel::Sandbox`. Dado que la `handle_table` nace vacía y solo se inyecta su propio directorio y la conexión al DWM, el Capability-based security impide de base ataques globales.
- **Conexión con `FastOS_Package_Manager.md`:** El Gestor de Paquetes es el notario que revisa el `.bpkg` y activa el flag Sandbox en el archivo maestro del disco.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
La seguridad de Windows falla porque por defecto las aplicaciones heredan los permisos del usuario administrador. FastOS utiliza el enfoque *Zero-Trust*: un programa arranca desnudo. No tiene Handles para tocar el hardware gráfico, no tiene Handles para tocar tus archivos personales y está confinado en un universo matemático aislado por las Tablas de Paginación del hardware de Intel/AMD. Para que una aplicación haga algo útil, debe pedirlo explícitamente vía Syscalls a Ring 0, y Ring 0 validará matemáticamente si tiene la capacidad (`BmoHandle`) para hacerlo.
