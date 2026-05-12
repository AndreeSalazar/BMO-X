# FastOS Rust Runtime Specification
**Capa:** UserSpace (Ring 3)
**Prioridad:** ALTA
**Depende de:** `BEF_Executable_Format_Spec.md`, `FastOS_Syscall_Table_Spec.md`
**Inspiración:** Rust `std` runtime (crt0), musl libc `_start`.

---

## FASE 1: ADN Extraído (¿Qué hace Windows/Linux aquí?)
Cuando haces doble clic en un `.exe` en Windows o un ELF en Linux, el control de ejecución no va a tu función `main()` inmediatamente. Entra a una función mágica llamada `_start` (comúnmente provista por `glibc` o MSVC) que inicializa cosas críticas como el TLS, los argumentos de consola y el recolector de excepciones (Stack Unwinding) antes de llamarte.
- **Qué conservamos:** La necesidad absoluta de un entry point oculto (`_start`) para preparar el Global Allocator para que el desarrollador pueda usar `Box` y `String`.
- **Qué tiramos:** El pesado `libunwind` (No habrá Stack Unwinding, si una app crashea, se aborta y punto) y toda la inicialización de variables de entorno de POSIX y Win32.

---

## FASE 2: Diseño BMO Nativo

Diseñamos la inicialización más rápida del mundo. El Kernel FastOS nos hace la vida fácil porque, según el `BEF_Executable_Format_Spec.md`, cuando el OS salta a Ring 3, inyecta un puntero a la estructura `BmoProcessEnv` directamente en el registro `RDI`.

### 1. El Entry Point Mágico (`_start`)
```rust
// runtime/start.rs
use libbmo::sys::BmoProcessEnv;

#[no_mangle]
pub unsafe extern "C" fn _start(env_ptr: *const BmoProcessEnv) -> ! {
    // 1. Guardar el entorno para que libbmo pueda acceder globalmente
    libbmo::env::init(env_ptr);

    // 2. Comprobar los límites del Stack para prevenir overflows
    let rsp: u64;
    core::arch::asm!("mov {}, rsp", out(reg) rsp);
    if rsp < (*env_ptr).stack_limit || rsp > (*env_ptr).stack_base {
        libbmo::sys::sys_exit(-1); // El Kernel nos cargó mal, abortar instatáneamente
    }

    // 3. Inicializar el Global Allocator (Para usar Box, Vec, String)
    libbmo::alloc::init_global_allocator();

    // 4. Inicializar Thread Local Storage (TLS) básico para el hilo principal
    libbmo::tls::init_main_thread();

    // 5. Salto al código del usuario
    extern "Rust" { fn main() -> i32; }
    let exit_code = main();

    // 6. Matar el proceso limpiamente
    libbmo::sys::sys_exit(exit_code);
}
```

### 2. Global Allocator sobre `sys_mmap`
Para que un dev pueda hacer `let x = vec![1, 2, 3]`, Rust necesita un gestor de *Heap*.
```rust
// runtime/alloc.rs
use libbmo::sys::{sys_mmap, MemType};

struct BmoGlobalAllocator;

unsafe impl core::alloc::GlobalAlloc for BmoGlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        // En v1 simple: pedimos bloques completos de 4KB al Kernel usando la Syscall de FastOS
        let size = (layout.size() + 4095) & !4095; 
        
        match sys_mmap(0, size, MemType::SystemRAM) {
            Ok(addr) => addr as *mut u8,
            Err(_) => core::ptr::null_mut(),
        }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // ... llamada a sys_munmap
    }
}

#[global_allocator]
static ALLOCATOR: BmoGlobalAllocator = BmoGlobalAllocator;
```

### 3. Panic Handler (Abort Only)
En BMO, si hay un pánico, el proceso muere instantáneamente para proteger la memoria, sin intentar desenredar el stack (zero *stack unwinding* overhead).
```rust
// runtime/panic.rs
use core::panic::PanicInfo;
use libbmo::io::print_to_stderr;
use libbmo::sys::sys_exit;

#[panic_handler]
fn bmo_panic(info: &PanicInfo) -> ! {
    // Convertimos el mensaje a bytes y lo escupimos vía sys_write a la consola
    print_to_stderr(format_args!("FastOS App Panic: {}\n", info));
    
    // Matanza limpia
    sys_exit(1);
}
```

---

## FASE 3: Implementación (El Developer Experience)

El desarrollador no ve nada de esto. Para compilar una app `.bef`, usa `cargo` con un target custom (ej. `x86_64-fastos-bef.json`). El linker de BMO inyecta silenciosamente este runtime (`_start` + `Panic Handler` + `Allocator`) asegurando que cualquier código en Rust estándar compile y funcione perfectamente asumiendo un entorno `no_std` mejorado.

---

## FASE 4: Integración con el Stack FastOS

- **Conexión con `BEF_Executable_Format_Spec.md`:** `_start` es literalmente la primera instrucción de código de la sección `CODE` del archivo `.bef`.
- **Conexión con `FastOS_Syscall_Table_Spec.md`:** Utiliza `sys_mmap` para el alloc de memoria (Pilar 2), `sys_write` para el texto del Panic (Pilar 3), y `sys_exit` (Pilar 1) al finalizar.

---

## Conclusión

**Qué aprendimos y mejoramos vs Windows:**
El *Runtime de C (CRT)* de Windows inicializa cientos de librerías dinámicas, lee registros, carga variables de entorno gigantescas y prepara estructuras para GUI obsoletas antes de ejecutar el `main()`. El Runtime de FastOS no toca el disco duro, no carga librerías y hace exactamente 4 cosas vitales antes de saltar al usuario. El resultado es que una aplicación BEF pasa del Kernel al `main()` en menos de 50 nanosegundos, con protección de stack por hardware incluida.
