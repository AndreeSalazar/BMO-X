//! Ring 0 — el suelo sobre el que se apoya todo lo demás.
//!
//! ## Cómo está repartido, y por qué así
//!
//! Esto eran **24 archivos sueltos** en un solo directorio. No es que fuera
//! monolítico —cada uno hace lo suyo— pero un listado plano de 24 nombres no
//! dice de qué va el sistema, y hay que leerlos todos para saber cuál importa.
//! Las carpetas son las familias que ya existían sin estar dibujadas:
//!
//! | | Qué vive ahí |
//! |---|---|
//! | `obj/` | **Los objetos que Ring 3 puede tener.** Cada uno es un `KIND_` con sus operaciones. Si algo se pide con una capability, está aquí. |
//! | `task/` | Procesos: admitir, planificar, lanzar desde disco. |
//! | `fsys/` | Almacenamiento LÓGICO: de sectores a archivos. El disco como hardware es `dev/disk.rs`. |
//! | `plat/` | La plataforma bajo el sistema: fallos, trampas, reloj, reinicio, cerrojos. |
//! | `dev/` | Hardware de verdad: PCI, AHCI, xHCI, framebuffer. |
//! | `mm/`, `cpu/`, `cpu_vendor/`, `svc/`, `core/` | Memoria, CPU, servicios y arranque. Ya estaban. |
//!
//! En la raíz quedan **cuatro** archivos a propósito, y son los que hay que
//! leer primero: `syscall.rs` (la superficie congelada — las tres puertas),
//! `cabina.rs` y `uconsole.rs` (lo que el sistema confiesa de sí mismo), y
//! este `mod.rs`.
//!
//! La frontera que más se nota: **`obj/` es lo que Ring 3 puede pedir; `dev/`
//! es lo que sólo Ring 0 toca.** Un archivo que cruce esa línea está en el
//! sitio equivocado.

/// Los objetos que Ring 3 puede tener, uno por `KIND_`. Un nombre es
/// adivinable; un handle concedido no.
pub mod obj {
    pub mod archivo;
    pub mod cap;
    pub mod channel;
    pub mod consola;
    pub mod directorio;
    pub mod endpoint;
    pub mod fb;
    pub mod input;
    /// `KIND_PRESTADO`: un proceso cede un trozo de SU memoria a otro. El
    /// kernel mueve páginas y **no sabe para qué** — el lienzo, el audio y los
    /// bloques grandes entre procesos salen todos de aquí.
    pub mod prestamo;
    /// `KIND_MEMORIA`: bloques que un proceso PIDE. No es un asignador — ver
    /// la cabecera del módulo.
    pub mod memoria;
}

/// Procesos: admitirlos, planificarlos, lanzarlos desde el disco.
pub mod task {
    pub mod bex;
    pub mod lanzar;
    pub mod percpu;
    pub mod proc;
    pub mod scheduler;
}

/// Almacenamiento lógico: de sectores a ARCHIVOS. El disco como hardware vive
/// en `dev/disk.rs`; la diferencia es la misma que entre un sector y un nombre.
pub mod fsys {
    pub mod estratos;
    pub mod fs;
}

/// La plataforma que sostiene al kernel: fallos, trampas, reloj y cerrojos.
pub mod plat {
    pub mod faults;
    /// El censo de núcleos que da el firmware por ACPI. Es la fuente de los
    /// APIC IDs, y sustituye a la suposición `0..hilos-1`.
    pub mod madt;
    pub mod reinicio;
    /// Despertar los otros núcleos. **No corre en el arranque**: lo pide la
    /// orden `smp`. Ver la cabecera del módulo para saber por qué esto no podía
    /// vivir en `s1_cpu`.
    pub mod smp;
    pub mod spin;
    pub mod timer;
    pub mod trap;
}

pub mod core {
    pub mod entry;
    /// El informe del sistema que Ring 3 pide por `TASK_OP_INFO`. Está aquí y
    /// no en `obj/` porque no es un objeto con handle: son datos que el kernel
    /// ya tiene y contesta sin conceder nada. Leer cuánta RAM hay no es un
    /// privilegio, es una pregunta.
    pub mod informe;
    /// El log del kernel GUARDADO, para que Ring 3 pueda leerlo. Mismo
    /// criterio que `informe`: no concede nada, contesta texto. Hace falta
    /// desde que el escritorio es el arranque y el panel del kernel ya no se
    /// pinta — el relato de cómo arrancó la máquina se estaba perdiendo.
    pub mod klog;
    pub mod phase;
    /// EL GATO — el logo, en dos mascaras de 1 bit. Generado por
    /// `docs/arte/gato_a_mascara.py`; hay una copia gemela en el compositor
    /// porque son binarios distintos y cada uno necesita sus bytes dentro.
    pub mod gato;
    pub mod splash;
}
pub mod cpu;
pub mod cpu_vendor;
pub mod dev {
    pub mod console;
    pub mod disk;
    pub mod framebuffer;
    pub mod keyboard;
    pub mod pci;
    pub mod usb;
}
pub mod mm;
pub mod svc;

// ── La raíz: lo que hay que leer primero ────────────────────────────────
/// La superficie congelada. Tres puertas y ni una más.
pub mod syscall;
/// La caja negra: lo que el sistema confiesa de sí mismo.
pub mod cabina;
pub mod uconsole;
