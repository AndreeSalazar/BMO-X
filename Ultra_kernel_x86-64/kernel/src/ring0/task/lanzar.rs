//! Cargar un `.bex` de disco y admitirlo como proceso. **Sin pintar nada.**
//!
//! ## Por que esto es un modulo y no una funcion del shell
//!
//! Esta logica --buscar en ESTRATOS, caer a FAT32, comprobar la firma, admitir--
//! vivia dentro de `shell_run`, entrelazada con las filas que pintaba en el
//! panel. Mientras el unico que lanzaba programas era el shell de Ring 0, eso
//! daba igual.
//!
//! Ya no lo es. La caja de Ring 3 lanza por `TASK_OP_EJECUTAR`, y un proceso
//! Ring 3 **no tiene panel donde pintar filas**: la pantalla es suya. Copiar la
//! logica habria sido tener dos gates de firma que se separan en cuanto alguien
//! toque uno -- y el gate de firma es exactamente lo que no puede tener dos
//! versiones.
//!
//! Asi que aqui esta una sola vez, muda, y devuelve un informe. El shell lo
//! convierte en filas; el syscall lo convierte en un codigo de error. Ninguno
//! de los dos decide nada sobre la firma: eso se decide aqui.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::ring0::fsys::estratos as est;

/// Por que no se lanzo. Cada uno manda a hacer algo distinto -- que es la razon
/// de que sean variantes y no un booleano.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fallo {
    /// No se dijo que lanzar.
    RutaVacia,
    /// No quedan huecos de proceso.
    SinHueco,
    /// Otro lanzamiento esta usando el buffer de imagen ahora mismo.
    Ocupado,
    /// El archivo no esta, o el nombre no cabe en 8.3.
    NoSeEncuentra(&'static str),
    /// Esta en ESTRATOS pero la lectura fallo.
    NoSePudoLeer,
    /// La firma NO cuadra con el contenido. El gate lo rechaza.
    FirmaMala,
    /// El nodo de ESTRATOS no lleva `:firma`. Sin firma no hay ejecucion.
    SinFirma,
    /// El `.bex` no paso la admision (BEX invalido, sin memoria...).
    NoAdmitido,
}

impl Fallo {
    /// Una linea corta, en el idioma del sistema. La usan el shell y CABINA.
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

    /// Codigo que ve Ring 3. No es el motivo entero --un proceso no necesita
    /// saber de FAT32-- pero distingue las tres cosas que le hacen cambiar de
    /// conducta: no existe, no se permite, no cupo.
    pub fn codigo(self) -> u32 {
        match self {
            Fallo::RutaVacia | Fallo::NoSeEncuentra(_) => ERROR_NOT_THERE,
            Fallo::FirmaMala | Fallo::SinFirma => ERROR_GATE,
            Fallo::SinHueco | Fallo::Ocupado => ERROR_BUSY,
            Fallo::NoSePudoLeer | Fallo::NoAdmitido => ERROR_NO_ADMITIDO,
        }
    }
}

pub const ERROR_NOT_THERE: u32 = 20;
pub const ERROR_GATE: u32 = 21;
pub const ERROR_BUSY: u32 = 22;
pub const ERROR_NO_ADMITIDO: u32 = 23;

/// Todo lo que se supo del intento, salga bien o mal.
///
/// El origen y el tamano se rellenan aunque el gate rechace despues: el shell
/// los pintaba ANTES de comprobar la firma y esa informacion sigue siendo util
/// cuando el rechazo llega. Un informe que solo habla cuando todo va bien no
/// sirve para depurar nada.
pub struct Informe {
    pub origen: &'static str,
    pub bytes: usize,
    /// `None` = no se llego a leer el archivo.
    pub firma: Option<est::Firma>,
    /// Pid del proceso admitido. Hace falta para encauzar su salida a la
    /// consola de quien lo lanzo -- el tid identifica al hilo, no al proceso.
    pub pid: Option<u32>,
    pub res: Result<u32, Fallo>,
}

/// Tope de una imagen `.bex`. **4 MiB.**
///
/// Historia, porque explica por que este numero sube a saltos y no poco a poco:
/// eran 64 KiB cuando el `.bex` mas grande eran cinco de COBOL. El compositor
/// llego a **61.6 KiB** --el 94% del tope-- y **una sola linea nueva lo paso a 82
/// KiB de golpe**: con `lto` y un `match` grande, LLVM cruza un umbral de
/// inlining y el binario da un salto de veinte KiB. A partir de ahi el
/// escritorio no cargaba y la maquina se quedaba en el panel del kernel.
///
/// Por eso 256 KiB tampoco valia, ni 1 MiB: el compositor va por **164 KiB** con
/// tres ventanas, y lo que viene --superficies, tiling, una barra de estado-- es
/// mas. Un tope que se roza es un tope que un dia se cruza sin avisar, y el
/// aviso llega en forma de maquina que no arranca al escritorio.
///
/// ** EL NUMERO LO PONE DOOM, Y ESO ES DELIBERADO (2026-08-09).**
///
/// El nucleo de DOOM compilado por BMO C mide **1.299.512 bytes** -- las 56.465
/// lineas en una sola unidad de traduccion, porque aqui no hay enlazado. Con el
/// tope en 1 MiB **no cabia por 248.936 bytes**.
///
/// La regla que se aplica: **el programa ajeno manda sobre el tope, no al
/// reves.** DOOM es de 1993 y es el codigo mas apretado que se va a portar aqui
/// en mucho tiempo; si no cabe, el que esta mal medido es el bufer. Cuatro MiB
/// dejan un margen de 3x sobre esa imagen, que es lo que hace falta para que el
/// backend de plataforma y los datos que vengan detras no obliguen a volver a
/// tocar este numero dentro de una semana.
///
/// **El coste es RAM del kernel y nada mas.** Esto es `.bss`: no viaja en la
/// imagen EFI --el `.bin` no la lleva-- y el cargador UEFI ya pone a cero el
/// hueco entero del kernel, que son **16 MiB reservados en `0x400000`**. Con un
/// kernel de ~2,1 MiB, cuatro mas siguen dejando diez libres. En una maquina de
/// 14.8 GiB, el 0,027% de la RAM.
///
/// * Lo que este numero **no** arregla, dicho para que nadie lo suponga: el
/// bufer sigue siendo **uno y estatico**, asi que dos lanzamientos a la vez se
/// siguen serializando con `EN_USO`. Y sigue siendo una **pagina de rebote**:
/// el disco escribe aqui y luego se copia al espacio del proceso. Lo que borra
/// ese coste es DMA directo al bufer del llamante, que esta en la hoja de ruta
/// y es otra conversacion. A 4 MiB esa copia ya se nota, asi que la
/// conversacion se acerca.
const MAX_BEX: usize = 4 * 1024 * 1024;
static mut IMAGE: [u8; MAX_BEX] = [0u8; MAX_BEX];

/// El buffer de imagen es UNO y estatico: un `.bex` son varios KiB y la pila
/// del kernel son 64 KiB para todo.
///
/// * Antes tenia un solo usuario (el shell) y por eso no hacia falta guardarlo.
/// Ahora tiene dos --el shell y cualquier proceso Ring 3 que llame a
/// `EJECUTAR`-- y entre ellos hay preempcion: el timer puede quitarle el turno
/// al shell con el buffer medio lleno. Dos lanzamientos solapados se pisarian
/// la imagen y admitirian un binario mezclado, que es la clase de fallo que no
/// se reproduce nunca. Se rechaza el segundo y se dice por que.
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
    // -- CR3 del kernel mientras dure --
    //
    // Leer el disco es tocar MMIO del AHCI (`0xFC680000` en esta placa), y ese
    // rango esta mapeado en el PML4 del kernel y NO en el de una tarea de
    // usuario. Mientras el unico que llamaba aqui era el shell --tarea de
    // kernel-- no se notaba. Desde que la caja de Ring 3 lanza por
    // `OP_EJECUTAR`, esto se recorre **desde dentro de un SYSCALL**, y en un
    // SYSCALL el CR3 sigue siendo el del llamante: el cambio de CR3 solo ocurre
    // en un cambio de contexto y aqui todavia no ha habido ninguno. Daba
    // `#PF(0)` con `cr2 = 0xFC680320`.
    //
    // Es la MISMA mina que ya se piso con el xHCI en `usb::poll_ascii`, con
    // otro periferico. La regla no es "el framebuffer necesita CR3 de kernel":
    // es **cualquier direccion del rango identidad alto tocada desde un
    // syscall**. Cada capability nueva que llegue a hardware vuelve aqui.
    //
    // Se envuelve la carga ENTERA y no cada lectura de sector: un `.bex` son
    // varios KiB y cambiar el CR3 por sector serian cientos de vaciados de TLB
    // para leer un archivo. La mitad alta --physmap, pilas, imagen del kernel--
    // esta mapeada igual en los dos espacios, asi que todo lo que hace
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
    // kernel puesto seria mucho peor que el fallo original -- la tarea seguiria
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

    // ESTRATOS primero: es el sistema de ficheros propio y el UNICO donde un
    // binario puede traer su firma pegada. Si no esta ahi, se cae a FAT32, que
    // sigue siendo de donde arranca la maquina.
    let nodo_est = if est::is_mounted() { est::open(path) } else { None };

    let (origen, n, veredicto) = if let Some(nd) = nodo_est {
        let leidos = match est::read(&nd, buf) {
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
                // * EL MOTIVO, AL KLOG. `Fallo::NoSeEncuentra` ya lo lleva
                // dentro, pero por la puerta solo cabe un codigo y Ring 3 lo
                // pinta todo como *"no esta: revisa la ruta"* -- un mensaje que
                // te manda a mirar la ruta cuando la ruta es perfecta.
                //
                // Paso de verdad el 2026-08-07: `c/read.bex` SALIA EN `ls` y
                // `run` decia que no estaba. El motivo real era otro --la imagen
                // pesa 1,1 MB y `MAX_BEX` es 1 MiB, asi que no cabia en el
                // bufer-- y no habia forma de saberlo desde fuera.
                //
                // Arreglar el codigo de error es tocar el ABI; escribir el
                // motivo donde ya se mira, no. F11 lo cuenta.
                crate::ring0::core::phase::dashboard_log("[lanzar] NO se pudo cargar la imagen");
                crate::ring0::cabina::warn("lanzar", e.name(), 0);
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

    // -- El gate: sin firma buena no hay ejecucion --
    //
    // section 7 del diseno de ESTRATOS: `open(nodo, EJECUTAR)` comprueba `:firma` y
    // si no cuadra NO entrega un handle ejecutable. Se aplica antes de admitir
    // nada, que es el unico momento en que sirve de algo.
    //
    // FAT32 queda fuera a proposito y no por pereza: no tiene atributos con
    // nombre, asi que un binario de ahi no PUEDE traer su firma pegada. La
    // asimetria es del formato, no del gate.
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

    // El nombre del proceso es el ultimo componente de la ruta: es lo que se
    // reconoce en el log, no la ruta entera.
    let name = match path.as_bytes().iter().rposition(|&c| c == b'/' || c == b'\\') {
        Some(i) => &path[i + 1..],
        None => path,
    };

    let (res, pid) = match crate::ring0::task::proc::admit_from_disk(name, &buf[..n]) {
        Some((tid, pid)) => (Ok(tid), Some(pid)),
        None => (Err(Fallo::NoAdmitido), None),
    };
    // * De donde salio, para que pueda leer su propia caja con
    // `TASK_OP_MI_PAQUETE`. Se apunta **despues** de que la admision haya ido
    // bien: recordar la ruta de un proceso que no llego a existir dejaria
    // basura que solo se limpia cuando ese pid se reutilice.
    if let Some(pid) = pid {
        crate::ring0::task::paquete::recordar(pid, path);
    }
    Informe { origen, bytes: n, firma: veredicto, pid, res }
}
