//! Cargar un `.bex` de disco y admitirlo como proceso. **Sin pintar nada.**
//!
//! ## Por qué esto es un módulo y no una función del shell
//!
//! Esta lógica —buscar en ESTRATOS, caer a FAT32, comprobar la firma, admitir—
//! vivía dentro de `shell_run`, entrelazada con las filas que pintaba en el
//! panel. Mientras el único que lanzaba programas era el shell de Ring 0, eso
//! daba igual.
//!
//! Ya no lo es. La caja de Ring 3 lanza por `TASK_OP_EJECUTAR`, y un proceso
//! Ring 3 **no tiene panel donde pintar filas**: la pantalla es suya. Copiar la
//! lógica habría sido tener dos gates de firma que se separan en cuanto alguien
//! toque uno — y el gate de firma es exactamente lo que no puede tener dos
//! versiones.
//!
//! Así que aquí está una sola vez, muda, y devuelve un informe. El shell lo
//! convierte en filas; el syscall lo convierte en un código de error. Ninguno
//! de los dos decide nada sobre la firma: eso se decide aquí.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::ring0::fsys::estratos as est;

/// Por qué no se lanzó. Cada uno manda a hacer algo distinto — que es la razón
/// de que sean variantes y no un booleano.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fallo {
    /// No se dijo qué lanzar.
    RutaVacia,
    /// No quedan huecos de proceso.
    SinHueco,
    /// Otro lanzamiento está usando el buffer de imagen ahora mismo.
    Ocupado,
    /// El archivo no está, o el nombre no cabe en 8.3.
    NoSeEncuentra(&'static str),
    /// Está en ESTRATOS pero la lectura falló.
    NoSePudoLeer,
    /// La firma NO cuadra con el contenido. El gate lo rechaza.
    FirmaMala,
    /// El nodo de ESTRATOS no lleva `:firma`. Sin firma no hay ejecución.
    SinFirma,
    /// El `.bex` no pasó la admisión (BEX inválido, sin memoria...).
    NoAdmitido,
}

impl Fallo {
    /// Una línea corta, en el idioma del sistema. La usan el shell y CABINA.
    pub fn motivo(self) -> &'static str {
        match self {
            Fallo::RutaVacia => "no dijiste que lanzar",
            Fallo::SinHueco => "no quedan huecos de proceso",
            Fallo::Ocupado => "hay otro lanzamiento en curso",
            Fallo::NoSeEncuentra(e) => e,
            Fallo::NoSePudoLeer => "esta en ESTRATOS pero no se pudo leer",
            Fallo::FirmaMala => "la firma NO cuadra: ejecucion rechazada",
            Fallo::SinFirma => "sin firma no hay ejecucion",
            Fallo::NoAdmitido => "el .bex no paso la admision",
        }
    }

    /// Código que ve Ring 3. No es el motivo entero —un proceso no necesita
    /// saber de FAT32— pero distingue las tres cosas que le hacen cambiar de
    /// conducta: no existe, no se permite, no cupo.
    pub fn codigo(self) -> u32 {
        match self {
            Fallo::RutaVacia | Fallo::NoSeEncuentra(_) => ERROR_NO_ESTA,
            Fallo::FirmaMala | Fallo::SinFirma => ERROR_GATE,
            Fallo::SinHueco | Fallo::Ocupado => ERROR_OCUPADO,
            Fallo::NoSePudoLeer | Fallo::NoAdmitido => ERROR_NO_ADMITIDO,
        }
    }
}

pub const ERROR_NO_ESTA: u32 = 20;
pub const ERROR_GATE: u32 = 21;
pub const ERROR_OCUPADO: u32 = 22;
pub const ERROR_NO_ADMITIDO: u32 = 23;

/// Todo lo que se supo del intento, salga bien o mal.
///
/// El origen y el tamaño se rellenan aunque el gate rechace después: el shell
/// los pintaba ANTES de comprobar la firma y esa información sigue siendo útil
/// cuando el rechazo llega. Un informe que sólo habla cuando todo va bien no
/// sirve para depurar nada.
pub struct Informe {
    pub origen: &'static str,
    pub bytes: usize,
    /// `None` = no se llegó a leer el archivo.
    pub firma: Option<est::Firma>,
    /// Pid del proceso admitido. Hace falta para encauzar su salida a la
    /// consola de quien lo lanzó — el tid identifica al hilo, no al proceso.
    pub pid: Option<u32>,
    pub res: Result<u32, Fallo>,
}

/// Tope de una imagen `.bex`. **256 KiB.**
///
/// Eran 64 KiB, elegidos cuando el `.bex` más grande del sistema eran cinco de
/// COBOL. El compositor había llegado a **61.6 KiB** —el 94% del tope— y una
/// sola línea nueva lo pasó a 82 KiB de golpe: con `lto` y un `match` grande,
/// LLVM cruza un umbral de inlining y el binario da un salto de veinte KiB.
/// A partir de ahí el escritorio no cargaba y la máquina se quedaba en el
/// panel del kernel, sin decir por qué salvo una línea de "no cabe".
///
/// El coste es RAM, no tamaño de la imagen EFI: esto es `.bss`, y `.bss` no
/// viaja en el fichero — lo pone a cero `entry.rs` al arrancar. 192 KiB más de
/// RAM en una máquina con 16 GiB, a cambio de un margen de 3× sobre el binario
/// de Ring 3 más grande que existe.
const MAX_BEX: usize = 256 * 1024;
static mut IMAGE: [u8; MAX_BEX] = [0u8; MAX_BEX];

/// El buffer de imagen es UNO y estático: un `.bex` son varios KiB y la pila
/// del kernel son 64 KiB para todo.
///
/// ★ Antes tenía un solo usuario (el shell) y por eso no hacía falta guardarlo.
/// Ahora tiene dos —el shell y cualquier proceso Ring 3 que llame a
/// `EJECUTAR`— y entre ellos hay preempción: el timer puede quitarle el turno
/// al shell con el buffer medio lleno. Dos lanzamientos solapados se pisarían
/// la imagen y admitirían un binario mezclado, que es la clase de fallo que no
/// se reproduce nunca. Se rechaza el segundo y se dice por qué.
static EN_USO: AtomicBool = AtomicBool::new(false);

/// Carga y admite. No pinta, no bloquea, no reintenta.
pub fn ruta(path: &str) -> Informe {
    let vacio = |f: Fallo| Informe { origen: "", bytes: 0, firma: None, pid: None, res: Err(f) };

    let path = path.trim();
    if path.is_empty() {
        return vacio(Fallo::RutaVacia);
    }
    if !crate::ring0::task::proc::has_room() {
        return vacio(Fallo::SinHueco);
    }
    if EN_USO.swap(true, Ordering::Acquire) {
        return vacio(Fallo::Ocupado);
    }
    // ── CR3 del kernel mientras dure ──
    //
    // Leer el disco es tocar MMIO del AHCI (`0xFC680000` en esta placa), y ese
    // rango está mapeado en el PML4 del kernel y NO en el de una tarea de
    // usuario. Mientras el único que llamaba aquí era el shell —tarea de
    // kernel— no se notaba. Desde que la caja de Ring 3 lanza por
    // `OP_EJECUTAR`, esto se recorre **desde dentro de un SYSCALL**, y en un
    // SYSCALL el CR3 sigue siendo el del llamante: el cambio de CR3 solo ocurre
    // en un cambio de contexto y aquí todavía no ha habido ninguno. Daba
    // `#PF(0)` con `cr2 = 0xFC680320`.
    //
    // Es la MISMA mina que ya se pisó con el xHCI en `usb::poll_ascii`, con
    // otro periférico. La regla no es "el framebuffer necesita CR3 de kernel":
    // es **cualquier dirección del rango identidad alto tocada desde un
    // syscall**. Cada capability nueva que llegue a hardware vuelve aquí.
    //
    // Se envuelve la carga ENTERA y no cada lectura de sector: un `.bex` son
    // varios KiB y cambiar el CR3 por sector serían cientos de vaciados de TLB
    // para leer un archivo. La mitad alta —physmap, pilas, imagen del kernel—
    // está mapeada igual en los dos espacios, así que todo lo que hace
    // `con_buffer` (leer, verificar la firma, mapear el proceso nuevo) es
    // seguro bajo el CR3 del kernel.
    use crate::ring0::mm::vmm;
    let kpml4 = vmm::kernel_pml4();
    let previo = vmm::read_cr3();
    let cambiado = kpml4 != 0 && previo != kpml4;
    if cambiado {
        vmm::switch_to(kpml4);
    }
    let informe = con_buffer(path);
    // Se devuelve SIEMPRE y por un solo camino: volver a Ring 3 con el CR3 del
    // kernel puesto sería mucho peor que el fallo original — la tarea seguiría
    // corriendo con el espacio de direcciones de otro.
    if cambiado {
        vmm::switch_to(previo);
    }
    EN_USO.store(false, Ordering::Release);
    informe
}

/// El cuerpo, ya con el buffer tomado. Separado para que el `EN_USO` se suelte
/// por un solo camino pase lo que pase.
fn con_buffer(path: &str) -> Informe {
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(IMAGE) };

    // ESTRATOS primero: es el sistema de ficheros propio y el ÚNICO donde un
    // binario puede traer su firma pegada. Si no está ahí, se cae a FAT32, que
    // sigue siendo de donde arranca la máquina.
    let nodo_est = if est::is_mounted() { est::abrir(path) } else { None };

    let (origen, n, veredicto) = if let Some(nd) = nodo_est {
        let leidos = match est::leer(&nd, buf) {
            Some(v) => v,
            None => {
                return Informe {
                    origen: "ESTRATOS",
                    bytes: 0,
                    firma: None,
                    pid: None,
                    res: Err(Fallo::NoSePudoLeer),
                }
            }
        };
        let v = est::firma(&nd, &buf[..leidos]);
        ("ESTRATOS", leidos, Some(v))
    } else {
        match crate::ring0::fsys::fs::load(path, buf) {
            Ok(v) => ("FAT32", v, None),
            Err(e) => {
                return Informe {
                    origen: "FAT32",
                    bytes: 0,
                    firma: None,
                    pid: None,
                    res: Err(Fallo::NoSeEncuentra(e.name())),
                }
            }
        }
    };

    // ── El gate: sin firma buena no hay ejecución ──
    //
    // §7 del diseño de ESTRATOS: `abrir(nodo, EJECUTAR)` comprueba `:firma` y
    // si no cuadra NO entrega un handle ejecutable. Se aplica antes de admitir
    // nada, que es el único momento en que sirve de algo.
    //
    // FAT32 queda fuera a propósito y no por pereza: no tiene atributos con
    // nombre, así que un binario de ahí no PUEDE traer su firma pegada. La
    // asimetría es del formato, no del gate.
    if let Some(v) = veredicto {
        let fallo = match v {
            est::Firma::Cuadra => None,
            est::Firma::NoCuadra => Some(Fallo::FirmaMala),
            est::Firma::Ausente => Some(Fallo::SinFirma),
        };
        if let Some(f) = fallo {
            crate::ring0::cabina::fault("estratos", f.motivo(), n as u64);
            return Informe { origen, bytes: n, firma: veredicto, pid: None, res: Err(f) };
        }
    }

    // El nombre del proceso es el último componente de la ruta: es lo que se
    // reconoce en el log, no la ruta entera.
    let nombre = match path.as_bytes().iter().rposition(|&c| c == b'/' || c == b'\\') {
        Some(i) => &path[i + 1..],
        None => path,
    };

    let (res, pid) = match crate::ring0::task::proc::admit_from_disk(nombre, &buf[..n]) {
        Some((tid, pid)) => (Ok(tid), Some(pid)),
        None => (Err(Fallo::NoAdmitido), None),
    };
    Informe { origen, bytes: n, firma: veredicto, pid, res }
}
