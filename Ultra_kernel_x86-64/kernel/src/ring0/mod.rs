//! Ring 0 -- el suelo sobre el que se apoya todo lo demas.
//!
//! ## Como esta repartido, y por que asi
//!
//! Esto eran **24 archivos sueltos** en un solo directorio. No es que fuera
//! monolitico --cada uno hace lo suyo-- pero un listado plano de 24 nombres no
//! dice de que va el sistema, y hay que leerlos todos para saber cual importa.
//! Las carpetas son las familias que ya existian sin estar dibujadas:
//!
//! | | Que vive ahi |
//! |---|---|
//! | `obj/` | **Los objetos que Ring 3 puede tener.** Cada uno es un `KIND_` con sus operaciones. Si algo se pide con una capability, esta aqui. |
//! | `task/` | Procesos: admitir, planificar, lanzar desde disco. |
//! | `fsys/` | Almacenamiento LOGICO: de sectores a archivos. El disco como hardware es `dev/disk.rs`. |
//! | `plat/` | La plataforma bajo el sistema: fallos, trampas, reloj, reinicio, cerrojos. |
//! | `dev/` | Hardware de verdad: PCI, AHCI, xHCI, framebuffer. |
//! | `mm/`, `cpu/`, `cpu_vendor/`, `svc/`, `core/` | Memoria, CPU, servicios y arranque. Ya estaban. |
//!
//! En la raiz quedan **cuatro** archivos a proposito, y son los que hay que
//! leer primero: `syscall.rs` (la superficie congelada -- las tres puertas),
//! `cabina.rs` y `uconsole.rs` (lo que el sistema confiesa de si mismo), y
//! este `mod.rs`.
//!
//! La frontera que mas se nota: **`obj/` es lo que Ring 3 puede pedir; `dev/`
//! es lo que solo Ring 0 toca.** Un archivo que cruce esa linea esta en el
//! sitio equivocado.

/// Los objetos que Ring 3 puede tener, uno por `KIND_`. Un nombre es
/// adivinable; un handle concedido no.
pub mod obj {
    pub mod file;
    /// LA PUERTA DE ESTRATOS para un `Archivo`: leer desde Ring 3 lo que se
    /// escribio en el sistema de ficheros propio. Tramo 1.1.
    mod estratos;
    /// EL ARCHIVO QUE SE ESTA TRAYENDO: la carga por trozos de una ranura.
    /// Salio de `file.rs` por L6a, y el corte se eligio por nombres libres --
    /// es un ciclo de vida, no un camino de datos.
    mod cargando;
    /// `KIND_AUDIO`: el derecho a hacer ruido. Es el CONTRATO, no el driver --
    /// ver la cabecera del modulo.
    pub mod audio;
    pub mod cap;
    pub mod channel;
    pub mod console;
    pub mod directory;
    pub mod endpoint;
    pub mod fb;
    pub mod input;
    /// `KIND_PRESTADO`: un proceso cede un trozo de SU memoria a otro. El
    /// kernel mueve paginas y **no sabe para que** -- el lienzo, el audio y los
    /// bloques grandes entre procesos salen todos de aqui.
    pub mod loan;
    /// `KIND_MEMORIA`: bloques que un proceso PIDE. No es un asignador -- ver
    /// la cabecera del modulo.
    pub mod memory;
    /// `KIND_TAREA`: un hijo que YO lance. Cerrar es una operacion de su
    /// handle, no un poder del que cierra -- paso 3 de `PLAN_DIRECTOR.md`.
    pub mod tarea;
}

/// Procesos: admitirlos, planificarlos, lanzarlos desde el disco.
pub mod task {
    /// **El cierre de cada seccion al aterrizar.** Un `.bex` trae un BLAKE3 por
    /// seccion; esto los comprueba en el momento en que la seccion termina de
    /// caer en la memoria del proceso, no despues sobre un bufer.
    pub mod landing;
    pub mod bex;
    /// **Quien lanzo a quien.** Un pid y nada mas: lo justo para que una app
    /// pueda ofrecerle su superficie al que la puso en pantalla.
    pub mod family;
    pub mod launch;
    /// De donde salio cada proceso, para que pueda leer su propia caja.
    pub mod package;
    pub mod percpu;
    pub mod proc;
    pub mod scheduler;
}

/// Almacenamiento logico: de sectores a ARCHIVOS. El disco como hardware vive
/// en `dev/disk.rs`; la diferencia es la misma que entre un sector y un nombre.
pub mod fsys {
    pub mod estratos;
    pub mod fs;
}

/// La plataforma que sostiene al kernel: fallos, trampas, reloj y cerrojos.
pub mod plat {
    pub mod faults;
    /// El censo de nucleos que da el firmware por ACPI. Es la fuente de los
    /// APIC IDs, y sustituye a la suposicion `0..hilos-1`.
    pub mod madt;
    pub mod reinicio;
    /// Despertar los otros nucleos. **No corre en el arranque**: lo pide la
    /// orden `smp`. Ver la cabecera del modulo para saber por que esto no podia
    /// vivir en `s1_cpu`.
    pub mod smp;
    /// **La interrupcion del disco.** Que el aparato avise en vez de que se le
    /// pregunte: MSI directo al LAPIC, sin IOAPIC de por medio.
    pub mod irq;
    pub mod spin;
    pub mod timer;
    pub mod trap;
}

pub mod core {
    pub mod entry;
    /// El informe del sistema que Ring 3 pide por `TASK_OP_INFO`. Esta aqui y
    /// no en `obj/` porque no es un objeto con handle: son datos que el kernel
    /// ya tiene y contesta sin conceder nada. Leer cuanta RAM hay no es un
    /// privilegio, es una pregunta.
    pub mod report;
    /// El log del kernel GUARDADO, para que Ring 3 pueda leerlo. Mismo
    /// criterio que `informe`: no concede nada, contesta texto. Hace falta
    /// desde que el escritorio es el arranque y el panel del kernel ya no se
    /// pinta -- el relato de como arranco la maquina se estaba perdiendo.
    /// **BOOT TIMELINE** -- where the boot time actually goes.
    ///
    /// A ruler, not an optimisation: the kernel is up in 47 ms and Ring 3 is
    /// painted at 1164, and nobody had ever measured the 1.1 s in between.
    /// Reports the COST of each stage, which is the column that answers "what
    /// do I attack first?".
    pub mod boot_timeline;
    pub mod klog;
    /// **La AUTOPSIA de un fallo de Ring 3.** El klog cuenta el relato entero;
    /// esto guarda el informe COMPLETO de cada muerte -- vector, codigo,
    /// direccion, `rip`, `rsp`, que programa era y lo ultimo que dijo-- para
    /// que se pueda leer despues y mandar. El kernel captura en RAM; quien lo
    /// escribe a disco es Ring 3, que esta vivo. Ver la cabecera del modulo.
    pub mod autopsy;
    /// **THE ROLLING LOG** -- the band of screen rows anything in Ring 0 can
    /// write to. It owns the panel MINUS the band at the bottom, which is
    /// CABINA's; without that split the two erased each other.
    pub mod dashboard;
    /// **THE DESKTOP SUPERVISOR** -- launch it, notice it died, decide about
    /// retrying. The launching happens at boot; the DECIDING does not, which
    /// is why it is no longer inside the file named after the boot phases.
    pub mod desktop;
    pub mod phase;
    /// **Las ordenes del shell de Ring 0**, repartidas por lo que preguntan.
    /// `phase.rs` llevaba 27 dentro y ocupaban dos tercios del fichero del
    /// arranque. Ver la cabecera del modulo.
    pub mod shell;
    /// EL GATO -- el logo, en dos mascaras de 1 bit. Generado por
    /// `docs/arte/gato_a_mascara.py`; hay una copia gemela en el compositor
    /// porque son binarios distintos y cada uno necesita sus bytes dentro.
    pub mod gato;
    pub mod splash;
}
pub mod cpu;
pub mod cpu_vendor;
pub mod dev {
    /// El VOLUMEN del audifono USB, por control transfer. Llega antes que
    /// reproducir nada: ver la cabecera del modulo.
    pub mod uaudio;
    pub mod console;
    pub mod disk;
    pub mod framebuffer;
    pub mod keyboard;
    pub mod pci;
    /// **La tarjeta de red, de momento solo RECONOCIDA.** Encuentra la NIC,
    /// elige su BAR de memoria y le pregunta su MAC y su enlace -- sin
    /// escribirle un byte. Los anillos DMA vienen despues, y sobre esta prueba.
    pub mod net;
    /// El reloj de la placa (CMOS). Lo que significan sus bytes lo decide
    /// `bmo-rtc`; aqui solo se tocan los puertos.
    pub mod clock;
    pub mod usb;
}
pub mod mm;
pub mod svc;

// -- La raiz: lo que hay que leer primero --------------------------------
/// La superficie congelada. Tres puertas y ni una mas.
pub mod syscall;
/// La caja negra: lo que el sistema confiesa de si mismo.
pub mod cabina;
pub mod uconsole;
