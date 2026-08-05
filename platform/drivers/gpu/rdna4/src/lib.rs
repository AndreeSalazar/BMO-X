//! BMO GPU driver profile — AMD RDNA 4.
//!
//! Reserved slot: the bench has no RDNA 4 card yet. This crate holds the
//! profile contract so that, the day the GPU arrives, bringing it up is a
//! profile fill-in — the same professional-profile rule the CPU follows
//! (`cpu_vendor/profile.rs` in the kernel: swapping hardware is a profile
//! swap, never a kernel edit).
//!
//! Like every BMO driver, this runs in **Ring 3** as a BEX server behind
//! a BMO Channel estuary. It will hold DEVICE + DISPLAY capabilities for
//! its own MMIO ranges only. The Ring 0 kernel never gains GPU code.
//!
//! ═══════════════════════════════════════════════════════════════════════
//! EL PLAN DE ARRANQUE, escrito el 2026-08-02 para no reconstruirlo dentro
//! de seis meses
//! ═══════════════════════════════════════════════════════════════════════
//!
//! ## ★ Regla 1: NO SE TOCA EL DISPLAY
//!
//! Y es lo que convierte esto de "un año" en "un driver del tamaño del de
//! AHCI". El firmware UEFI **ya programó el motor de pantalla** y dejó un
//! framebuffer lineal a 1920×1080 (GOP). Si se deja como está, **se salta
//! DCN entero** — el controlador de display, que es la parte más grande y
//! peor documentada de `amdgpu`, y la única que de verdad no tiene
//! documentación pública de registros.
//!
//! O sea: **no hay modeset, no hay scanout propio, no hay EDID**. Se hereda
//! lo que el firmware dejó puesto y se le escribe encima. Es lo que ya hace
//! el compositor hoy con el CPU.
//!
//! ## ★ Regla 2: UN motor, no la GPU entera
//!
//! De todo lo que trae una GPU moderna hace falta **uno**: el motor de copia
//! (**SDMA**). Nada de sombreadores, nada de pipeline 3D, nada de vídeo,
//! nada de gestión de energía — el firmware deja un estado que funciona.
//!
//! El trabajo real, en orden:
//!
//! ```text
//!   1. Enumerar PCI y mapear los BAR        -> YA SE HACE (xHCI, AHCI)
//!   2. Cargar el firmware por el PSP        -> blobs de linux-firmware
//!   3. Montar el anillo SDMA + su timbre    -> ES LA FORMA DE xHCI
//!   4. Encolar copias de rectangulo
//! ```
//!
//! El punto 3 es el argumento entero de por qué esto es alcanzable: BMO-X ya
//! tiene **un driver de anillo de comandos con timbre y anillo de eventos**
//! (`bmo-xhci`), con sus bugs ya peleados en metal. El modelo mental
//! transfiere entero; cambian los paquetes, no la forma.
//!
//! ## ★ Regla 3: se enchufa en UN sitio
//!
//! El compositor tiene desde hoy la costura hecha: `bmo_userland::Volcador`.
//! El backend de GPU es **una variante más** de ese enum y **una
//! implementación más** de `Pantalla::volcar`. No hay un segundo compositor,
//! ni un `gui_GPU.bex`: eso sería bifurcar y pagar cada arreglo dos veces.
//!
//! ## ★ Regla 4: PRIMERO EL NÚMERO
//!
//! El comando `perf` de la caja Ejecutar dice KiB por fotograma y peor caso.
//! La caja de sucio ya recorta casi todo el volcado, así que **la respuesta
//! puede perfectamente ser que una GPU no compra nada**. Ese número se mira
//! antes de gastar un sol, y se vuelve a mirar después para saber si sirvió.
//!
//! ## Por qué AMD y no Nvidia
//!
//! No es ideología: es que los blobs de firmware de AMD **existen, están
//! publicados y son redistribuibles**, y hay un driver abierto de referencia
//! que se puede leer. Ésa es la diferencia práctica, y es la que decidió una
//! pelea perdida antes con una RTX 3060.
//!
//! ## ⚠ Y la OTRA meta, que no es esta — ver `PLAN_VULKAN.md`
//!
//! Todo lo de arriba es **acelerar el compositor con SDMA**: un motor, sin
//! sombreadores, sin 3D. Es del tamaño del driver de AHCI y es alcanzable.
//!
//! **Correr juegos de Vulkan es otra meta y es un proyecto de años.** Vive en
//! `PLAN_VULKAN.md`, en esta misma carpeta, con sus piezas contadas y con la
//! ruta que casi nadie considera primero: **Vulkan por software**, que borra
//! el muro del PSP y el compilador de ISA de golpe.
//!
//! Separarlas es el punto: confundir "acelerar el volcado" con "correr Doom
//! Eternal" es la forma clasica de no terminar ninguna de las dos.
//!
//! ## Objetivo declarado
//!
//! **RX 9060 XT 16GB** (Navi 44 / GFX1200). Soporte en kernel desde 6.11 y
//! firmware en `linux-firmware`. Es lo más nuevo de los candidatos —
//! deliberadamente, porque el perfil tiene que seguir siendo el perfil
//! dentro de tres años, y la parte que se va a usar (SDMA) es de las más
//! estables del stack de AMD entre generaciones.
//!
//! ⚠️ **Comprobar antes de comprar** (ley 11: se pregunta, no se supone):
//! que la SKU concreta tenga su firmware en `linux-firmware` y sus cabeceras
//! de registros publicadas. Alternativas más maduras si eso fallara: RDNA 3
//! (Navi 33, RX 7600) o RDNA 2 (Navi 23, RX 6600) — el trabajo de SDMA es
//! prácticamente el mismo y llevan más años de rodaje.

#![no_std]

/// GPU profile descriptor, mirroring the CPU profile philosophy.
pub struct GpuProfile {
    pub vendor: &'static str,
    pub microarch: &'static str,
    /// PCI vendor id (AMD).
    pub pci_vendor: u16,
    /// PCI device ids this profile claims. Empty until the exact card
    /// (Navi 4x SKU) is on the bench.
    pub pci_devices: &'static [u16],
}

pub static PROFILE: GpuProfile = GpuProfile {
    vendor: "AMD",
    microarch: "RDNA 4 (Navi 4x)",
    pci_vendor: 0x1002,
    pci_devices: &[],
};

/// Whether the profile can claim `device_id`. Always false until the
/// SKU list is filled in with real hardware.
pub fn claims(vendor: u16, device: u16) -> bool {
    vendor == PROFILE.pci_vendor && PROFILE.pci_devices.contains(&device)
}
