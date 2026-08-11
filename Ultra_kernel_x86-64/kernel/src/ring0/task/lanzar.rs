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
///
/// == ** 2026-08-10: YA NO ES EL TOPE DEL FICHERO, ES EL DE LO CARGABLE ==
///
/// Escalon 2 de `LA_RAM.md`. Antes esto media **el archivo**, asi que un paquete
/// con un WAD dentro (~5,5 MB) no arrancaba aunque su parte ejecutable fueran
/// 812 KB: se traia la bodega entera para ejecutar el quirofano.
///
/// Ahora el cargador **pregunta al formato que necesita** (`bex::necesita`) y
/// trae solo eso: codigo, datos, relocations y hashes. Los recursos se quedan en
/// el disco hasta que el programa los pida por `TASK_OP_MI_PAQUETE`.
///
/// > **Un `.bex` puede medir lo que quiera. Lo que tiene que caber aqui es lo
/// > que se EJECUTA.**
const MAX_BEX: usize = 4 * 1024 * 1024;

/// El buffer, **alineado a pagina**.
///
/// == Por que la alineacion importa desde el escalon 3 ==
///
/// El HBA ya no escribe en una pagina de rebote: escribe **aqui**, si esto esta
/// seguido en memoria fisica (`disk::tramo_dma` lo comprueba, no lo supone). Un
/// array de bytes suelto tiene alineacion 1, o sea que podria empezar a mitad de
/// pagina -- y entonces el primer tramo mide lo que queda de esa pagina, que
/// puede no llegar ni a un sector. El camino rapido existiria y no se tomaria
/// nunca, sin que nada fallara.
///
/// Alinearlo a 4096 cuesta cero --es `.bss`, no viaja en la imagen-- y convierte
/// "a veces" en "siempre que la memoria fisica acompane".
#[repr(C, align(4096))]
struct Imagen([u8; MAX_BEX]);
static mut IMAGE: Imagen = Imagen([0u8; MAX_BEX]);

/// Lo que se lee de primeras para poder preguntarle al fichero que necesita.
///
/// La cabecera son 48 bytes y la tabla empieza en el 48 --`BefBuilder::build` la
/// pone siempre ahi-- con 48 bytes por seccion y dieciseis como tope: **816
/// bytes** cubren cualquier `.bex` que pueda existir. Dos kilos dejan sitio a
/// que ese contrato crezca sin que esto se quede corto en silencio, y si aun asi
/// no cupiera, `bex::necesita` lo dice con su nombre (`PrologoCorto`) en vez de
/// leer una tabla a medias.
const PROLOGO: usize = 2048;

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
    // ** TODO LO QUE PASE DE AQUI EN ADELANTE ES EL MISMO INTENTO.
    //
    // Cargar un `.bex` emite eventos desde cuatro modulos --`lanzar`, `proc`,
    // `bex`, `disk`-- y hasta hoy salian sueltos: cuatro renglones que contaban
    // una sola historia y que habia que juntar de memoria. Con esto llevan todos
    // el mismo numero, y **juntarlos deja de ser cosa del que mira**.
    //
    // [!] Y si esta funcion se fuera por un camino que nadie penso, el testigo
    // apunta al soltarse que el intento **quedo abierto**. Un lanzamiento que
    // desaparece sin decir como acabo era, hasta hoy, silencio.
    let testigo = crate::ring0::cabina::intento("lanzar");
    let informe = con_buffer(path);
    testigo.cerrar(informe.res.is_ok());
    // Se devuelve SIEMPRE y por un solo camino: volver a Ring 3 con el CR3 del
    // kernel puesto seria mucho peor que el fallo original -- la tarea seguiria
    // corriendo con el espacio de direcciones de otro.
    if cambiado {
        vmm::switch_to(previo);
    }
    EN_USO.store(false, Ordering::Release);
    informe
}

/// **DE DONDE SALEN LOS BYTES DE UNA IMAGEN.** Una pieza, dos implementaciones.
///
/// === El problema que resuelve, contado con lo que habia ===
///
/// `con_buffer` lee el fichero DOS veces --el prologo, y despues lo que haga
/// falta-- y cada una llevaba escrita su propia bifurcacion:
///
/// ```text
///   if let Some(nd) = &nodo_est { est::read(...) }        else { fs::load_prefijo(...) }
///   if let Some(nd) = &nodo_est { est::leer_y_firmar(..) } else { fs::load_prefijo(...) }
/// ```
///
/// Dos ramas por lectura, y **cada lectura nueva las duplica otra vez**. La
/// pieza B --el disco escribiendo seccion por seccion en los marcos del
/// proceso-- no hace dos lecturas: hace una por seccion. Con la bifurcacion
/// escrita a mano en cada punto, eso son diez ramas que tienen que decir lo
/// mismo, y el dia que una deje de decirlo el sintoma sera que ESTRATOS carga
/// programas que FAT32 rechaza, o al reves.
///
/// === Lo que gana ademas ===
///
/// ESTRATOS deja de ser una copia pegada del camino de FAT32. Hoy la asimetria
/// de verdad entre los dos es UNA --solo ESTRATOS puede traer `:firma`, porque
/// FAT32 no tiene atributos con nombre-- y estaba enterrada entre cuatro
/// bifurcaciones que la repetian sin decirlo. Aqui es un solo `Option`.
pub enum Fuente {
    /// El sistema de ficheros propio. Es el primero que se mira: es el unico
    /// donde un binario puede traer su firma pegada.
    Estratos(est::Nodo),
    /// De donde arranca la maquina, y donde vive todo hasta que ESTRATOS crezca.
    /// Lleva su cursor: leer por rangos hacia adelante cuesta UN recorrido de la
    /// cadena, no uno por rango. Ver `bmo_fat32::Cursor`.
    ///
    /// `inicio` es ese mismo cursor **sin usar**, y es lo que hace posible
    /// [`Fuente::rango_suelto`]: una lectura fuera del flujo se lleva una copia
    /// y no toca la del flujo. Cuesta doce bytes.
    Fat32 { cur: bmo_fat32::Cursor, inicio: bmo_fat32::Cursor, tam: u32 },
}

impl Fuente {
    /// Localiza la imagen. ESTRATOS primero; si no esta ahi, FAT32.
    ///
    /// No falla: que no este en ninguno de los dos lo dira la primera lectura,
    /// **con el motivo del sistema de ficheros en la mano** en vez de un "no
    /// existe" que se ha perdido por el camino cual de los dos lo dijo.
    fn abrir(path: &str) -> Self {
        match if est::is_mounted() { est::open(path) } else { None } {
            Some(nd) => Fuente::Estratos(nd),
            None => match crate::ring0::fsys::fs::abrir_rangos(path) {
                Ok((cur, tam)) => Fuente::Fat32 { cur, inicio: cur, tam },
                // No esta en ninguno de los dos. Se devuelve un FAT32 vacio para
                // que el motivo lo de la lectura --que sabe decir POR QUE-- en
                // vez de inventarlo aqui.
                Err(_) => Fuente::Fat32 {
                    cur: bmo_fat32::Cursor::vacio(),
                    inicio: bmo_fat32::Cursor::vacio(),
                    tam: 0,
                },
            },
        }
    }

    /// **UN RANGO DE LA IMAGEN, DONDE SE DIGA.** Devuelve cuantos bytes entraron.
    ///
    /// === Esta es la pieza B, y cabe en una firma ===
    ///
    /// El cargador ya no pide "el fichero". Pide **el trozo que le toca a esta
    /// pagina**, y `dst` es la ventana del marco que acaba de reservar. Los
    /// sectores enteros van del disco ahi **sin pasar por ningun sitio**: no hay
    /// mesa, no hay copia, y no hay un bufer compartido donde una escritura
    /// perdida pueda estropear la imagen de otro.
    ///
    /// [!] Solo avanza. El cargador aterriza las secciones en orden de offset de
    /// fichero justo por esto -- ver la nota del orden en `task/proc.rs`.
    pub fn rango(&mut self, offset: usize, dst: &mut [u8]) -> usize {
        match self {
            Fuente::Fat32 { cur, tam, .. } => {
                crate::ring0::fsys::fs::leer_rango(cur, offset, *tam, dst)
            }
            // ** ESTRATOS todavia lee desde cero, y por eso aqui no hay rango.
            //
            // No es que falte escribirlo: es que su gate (`:firma`) cubre el
            // fichero ENTERO, asi que comprobarlo obliga a leerlo entero -- y eso
            // es la bodega entrando por la puerta de la seguridad. La salida esta
            // decidida y escrita (`docs/EL_CONTRATO_DE_CARGA.md`): que `:firma`
            // pase a cubrir la seccion `Signature` del `.bex`, que es la que
            // responde por todas las demas. Mientras tanto, ESTRATOS no entrega
            // rangos y quien lo pida se entera aqui en vez de recibir ceros.
            Fuente::Estratos(_) => 0,
        }
    }

    /// **UN RANGO FUERA DEL FLUJO. No mueve el cursor de las secciones.**
    ///
    /// === Por que hace falta una segunda puerta ===
    ///
    /// El cursor **solo avanza**, y eso no es un descuido: llegar al byte `N` en
    /// FAT32 es seguir la cadena, asi que un cursor que retrocede convierte la
    /// carga en cuadratica sin que nada avise. El cargador cumple su parte
    /// aterrizando las secciones **en orden de offset de fichero**.
    ///
    /// ** Pero un `.bex` no se lee solo por secciones. Antes de aterrizar
    /// ninguna, el cargador necesita **dos tablas**: los hashes (`Signature`) y
    /// las relocations. Y las dos viven **al final del fichero** -- el escritor
    /// las pone detras de todo, que es donde tienen que ir.
    ///
    /// O sea que el flujo real era: saltar al final a por los hashes, y volver
    /// al byte 512 a por el codigo. **Hacia atras.** El cursor contestaba que no
    /// --correctamente-- y el cargador lo leia como "la seccion llego a medias":
    ///
    /// ```text
    ///    FAULT proc: una seccion se quedo a medias al aterrizar =0
    /// ```
    ///
    /// El arreglo no es dejar que el cursor retroceda --eso mata la garantia
    /// para todo el mundo por dos lecturas-- sino **reconocer que son dos
    /// patrones de acceso distintos**: uno secuencial y grande (las secciones) y
    /// otro suelto y pequeno (las dos tablas). Cada uno con su cursor.
    ///
    /// Cuesta un recorrido de la cadena por llamada, y se llama **dos veces por
    /// carga**. El flujo de las secciones -- que es el 99,9% de los bytes --
    /// sigue costando uno solo.
    pub fn rango_suelto(&mut self, offset: usize, dst: &mut [u8]) -> usize {
        match self {
            Fuente::Fat32 { inicio, tam, .. } => {
                // Una COPIA del cursor sin estrenar. El del flujo no se entera.
                let mut aparte = *inicio;
                crate::ring0::fsys::fs::leer_rango(&mut aparte, offset, *tam, dst)
            }
            Fuente::Estratos(_) => 0,
        }
    }

    /// Entrega rangos? Mientras ESTRATOS no lo haga, el cargador tiene que saber
    /// por cual de los dos caminos va **antes** de reservar un solo marco.
    pub fn por_rangos(&self) -> bool {
        matches!(self, Fuente::Fat32 { .. })
    }

    /// El cursor, para poder decir DE DONDE se lee. `None` en ESTRATOS, que no
    /// tiene clusters que contar.
    pub fn cursor(&self) -> Option<&bmo_fat32::Cursor> {
        match self {
            Fuente::Fat32 { cur, .. } => Some(cur),
            Fuente::Estratos(_) => None,
        }
    }

    /// Lo que mide el archivo. `0` si no se encontro.
    pub fn tamano(&self) -> usize {
        match self {
            Fuente::Fat32 { tam, .. } => *tam as usize,
            Fuente::Estratos(_) => 0,
        }
    }

    /// Como se llama esto en el log y en el panel.
    fn origen(&self) -> &'static str {
        match self {
            Fuente::Estratos(_) => "ESTRATOS",
            Fuente::Fat32 { .. } => "FAT32",
        }
    }

    /// **El principio de la imagen**, lo justo para preguntarle que necesita.
    /// Devuelve cuantos bytes llegaron.
    fn prologo(&self, path: &str, dst: &mut [u8]) -> Result<usize, Fallo> {
        match self {
            Fuente::Estratos(nd) => est::read(nd, dst).ok_or(Fallo::NoSePudoLeer),
            Fuente::Fat32 { .. } => match crate::ring0::fsys::fs::load_prefijo(path, dst) {
                Ok((leidos, _)) => Ok(leidos),
                Err(e) => Err(Fallo::NoSeEncuentra(e.name())),
            },
        }
    }

    /// **Lo que hace falta de verdad.** `(leidos, tamano real, veredicto)`.
    ///
    /// El veredicto es la unica asimetria real entre los dos: en ESTRATOS todos
    /// los bytes le pasan al hasher por delante en la misma pasada, asi que la
    /// firma cubre el archivo entero sin una segunda lectura. En FAT32 no hay
    /// donde guardarla, asi que es `None` -- y eso **no es que no se comprobara
    /// nada**: desde el 2026-08-09 el hash por seccion viaja dentro del propio
    /// `.bex`, y ese sigue su camino en los dos casos. Ver `task/aterrizaje.rs`.
    fn cuerpo(
        &self,
        path: &str,
        dst: &mut [u8],
    ) -> Result<(usize, usize, Option<est::Firma>), Fallo> {
        match self {
            Fuente::Estratos(nd) => match est::leer_y_firmar(nd, dst) {
                Some((leidos, tam, v)) => Ok((leidos, tam, Some(v))),
                None => Err(Fallo::NoSePudoLeer),
            },
            Fuente::Fat32 { .. } => match crate::ring0::fsys::fs::load_prefijo(path, dst) {
                Ok((leidos, tam)) => Ok((leidos, tam, None)),
                Err(e) => Err(Fallo::NoSeEncuentra(e.name())),
            },
        }
    }
}

/// El cuerpo, ya con el buffer tomado. Separado para que el `EN_USO` se suelte
/// por un solo camino pase lo que pase.
fn con_buffer(path: &str) -> Informe {
    // ** EL PROLOGO NO SALE DE LA MESA, SALE DE LA PILA (2026-08-10).
    //
    // Estaba leyendose en los primeros 2 KB del bufer de 4 MiB, o sea que **el
    // camino sin mesa seguia tocando la mesa** -- y por dos kilos arrastraba el
    // recurso compartido entero, que es lo unico que obliga a `EN_USO` a
    // serializar la maquina.
    //
    // Aqui es un local, y el tamano no es una eleccion: la cabecera son 48 bytes
    // y la tabla como mucho `16 * 48`, o sea **816 bytes que salen del formato**.
    // Dos kilos dejan sitio a que ese contrato crezca, y contra los 64 KiB de
    // pila del kernel es el 3%.
    //
    // > Un limite que sale del contrato no hay que subirlo nunca. `MAX_BEX` hubo
    // > que subirlo dos veces porque era una suposicion.
    let mut prologo_buf = [0u8; PROLOGO];
    let buf = &mut prologo_buf[..];

    // ESTRATOS primero: es el sistema de ficheros propio y el UNICO donde un
    // binario puede traer su firma pegada. Si no esta ahi, se cae a FAT32, que
    // sigue siendo de donde arranca la maquina. De aqui para abajo **no se
    // vuelve a preguntar cual de los dos es**: ver `Fuente`.
    let mut fuente = Fuente::abrir(path);
    let origen = fuente.origen();
    // Los contadores de DMA ANTES de leer nada: lo que interesa es el delta de
    // ESTA carga, no el total del arranque.
    let (d0, r0) = crate::ring0::dev::disk::cuentas_dma();

    // == FASE 1: EL PROLOGO ==
    //
    // La cabecera son 48 bytes y la tabla de secciones empieza en el 48 --el
    // escritor la pone SIEMPRE ahi (`BefBuilder::build`)-- con 48 por seccion y
    // dieciseis como maximo: 816 bytes cubren cualquier `.bex` que exista. Con
    // dos kilos sobra sitio para que ese contrato pueda crecer sin que esto se
    // quede corto en silencio.
    let prologo_n = match fuente.prologo(path, &mut buf[..PROLOGO]) {
        Ok(n) => n,
        Err(f) => {
            crate::ring0::core::phase::dashboard_log("[lanzar] NO se pudo cargar la imagen");
            crate::ring0::cabina::warn("lanzar", f.motivo(), 0);
            return Informe { origen, bytes: 0, firma: None, pid: None, res: Err(f) };
        }
    };

    // == ** EL CAMINO SIN MESA (pieza B) ==
    //
    // Si la fuente entrega rangos, aqui se acaba la lectura. No hay FASE 2 --no
    // hace falta preguntar "cuanto traigo" si no se trae nada-- ni FASE 3. El
    // plan sale del prologo, y cada seccion se le pide al disco cuando su marco
    // ya existe, cayendo dentro sin pasar por ningun sitio.
    //
    // Lo que desaparece con este `if`:
    //
    // | | antes | ahora |
    // |---|---|---|
    // | bufer | 4 MiB de `.bss`, uno para toda la maquina | ninguno |
    // | tope de programa | `MAX_BEX`, subido dos veces | la RAM que haya |
    // | copias por byte | una | **cero** |
    // | lanzamientos a la vez | uno (`EN_USO`) | los que quepan |
    //
    // ** El `EN_USO` sigue puesto porque el otro camino --ESTRATOS y las imagenes
    // embebidas-- todavia usa el bufer. El dia que ESTRATOS entregue rangos, el
    // candado se va con la mesa: sin recurso compartido no hay nada que
    // serializar. Ver `Fuente::rango`.
    if fuente.por_rangos() {
        let tam = fuente.tamano();
        // ** DE DONDE SE VA A LEER, dicho ANTES de leer.
        //
        // El 2026-08-11 el sistema supo decir que los ocho primeros bytes no eran
        // los del fichero --eran codigo x86-64 que no esta en NADA de lo que
        // construimos-- y no supo decir **de que sector**. Con eso solo, quedaban
        // dos explicaciones que se arreglan en sitios opuestos:
        //
        // | | |
        // |---|---|
        // | el LBA es raro | el mapa cluster->sector esta mal |
        // | el LBA es razonable | el disco tiene ahi otra cosa, o el volumen es otro |
        //
        // Va delante de la lectura a proposito: si la lectura cuelga la maquina,
        // esta linea ya esta grabada. Un diagnostico que solo se imprime cuando
        // todo fue bien no sirve para el caso en que no.
        if let Some(cur) = fuente.cursor() {
            let (cluster, lba, base) = crate::ring0::fsys::fs::donde(cur);
            crate::ring0::cabina::info("lanzar", "el directorio dice: cluster", cluster as u64);
            crate::ring0::cabina::info("lanzar", "que en el disco es el LBA", lba);
            crate::ring0::cabina::info("lanzar", "y la particion empieza en", base);
            crate::ring0::cabina::info("lanzar", "y el fichero mide", tam as u64);
        }
        let name = match path.as_bytes().iter().rposition(|&c| c == b'/' || c == b'\\') {
            Some(i) => &path[i + 1..],
            None => path,
        };
        let (res, pid) = match crate::ring0::task::proc::admitir_por_rangos(
            name,
            &buf[..prologo_n],
            &mut fuente,
            tam,
        ) {
            Some((tid, pid)) => (Ok(tid), Some(pid)),
            None => (Err(Fallo::NoAdmitido), None),
        };
        let (d1, _) = crate::ring0::dev::disk::cuentas_dma();
        crate::ring0::cabina::info("lanzar", "bytes DIRECTOS del disco al marco", d1 - d0);
        if let Some(pid) = pid {
            crate::ring0::task::paquete::recordar(pid, path);
        }
        return Informe { origen, bytes: tam, firma: None, pid, res };
    }

    // ** DE AQUI PARA ABAJO, EL CAMINO CON MESA -- y solo el.
    //
    // Solo llega ESTRATOS, cuyo gate hashea el fichero entero y por eso necesita
    // tenerlo entero. El bufer de 4 MiB se toma **aqui** y no arriba: mientras
    // fue de los dos caminos, el que no lo necesitaba lo arrastraba igual, y con
    // el se arrastraba `EN_USO` -- o sea la serializacion de toda la maquina por
    // dos kilos de prologo.
    //
    // El dia que ESTRATOS entregue rangos, este bloque entero desaparece y
    // `MAX_BEX` con el. Ver `Fuente::rango`.
    let buf = unsafe { &mut (*core::ptr::addr_of_mut!(IMAGE)).0 };

    // == FASE 2: QUE NECESITA ==
    //
    // ** Aqui esta el escalon 2 entero. No se pregunta "cuanto mide" sino **que
    // necesita**, y quien lo contesta es el propio fichero: `bex::necesita` lee
    // la tabla y suma hasta donde llega lo ultimo que el cargador va a tocar --
    // codigo, datos, relocations y hashes--. Los recursos van detras y **no se
    // traen**: el programa los pedira en ejecucion por `TASK_OP_MI_PAQUETE`, que
    // es su puerta.
    //
    // Si `necesita` no sabe contestar --no es un BEF, o la tabla no cabio en el
    // prologo-- se lee lo que quepa y se deja que `inspect` rechace **con su
    // nombre**. Un cargador que se calla el motivo por no saber leer la tabla
    // seria peor que el que leia de mas.
    // ** SE GUARDA EL VEREDICTO, no solo el numero. Ver `contradiccion`.
    //
    // `necesita` solo contesta `Ok` si vio un `BEF1`, una version buena y una
    // tabla de secciones que cabe. O sea que ese `Ok` no es un detalle de
    // implementacion: es **el testimonio de que en esta ruta, hace un momento,
    // habia un .bex valido**. Tirarlo --que es lo que se hacia-- es lo que deja
    // al sistema sin poder distinguir despues un fichero malo de una lectura
    // mala.
    // [!] Del PROLOGO, que es donde se leyo -- no de `buf`, que es la mesa y
    // todavia esta a ceros. Preguntarle a la mesa daria `Err` siempre, y el
    // sintoma seria que este camino se trae `MAX_BEX` de todos los ficheros
    // mientras aparenta estar preguntando.
    let veredicto_prologo = crate::ring0::task::bex::necesita(&prologo_buf[..prologo_n]);
    let prologo_valido = veredicto_prologo.is_ok();
    let hace_falta = match veredicto_prologo {
        Ok(h) => h.min(MAX_BEX),
        Err(_) => MAX_BEX,
    };

    // == FASE 3: LEER SOLO ESO ==
    let (n, tam, veredicto) = match fuente.cuerpo(path, &mut buf[..hace_falta]) {
        Ok(v) => v,
        Err(f) => {
            // * EL MOTIVO, AL KLOG. `Fallo::NoSeEncuentra` ya lo lleva dentro,
            // pero por la puerta solo cabe un codigo y Ring 3 lo pinta todo como
            // *"no esta: revisa la ruta"* -- un mensaje que te manda a mirar la
            // ruta cuando la ruta es perfecta.
            //
            // Paso de verdad el 2026-08-07: `c/read.bex` SALIA EN `ls` y `run`
            // decia que no estaba. El motivo real era otro --la imagen pesa
            // 1,1 MB y `MAX_BEX` es 1 MiB, asi que no cabia en el bufer-- y no
            // habia forma de saberlo desde fuera.
            //
            // Arreglar el codigo de error es tocar el ABI; escribir el motivo
            // donde ya se mira, no. F11 lo cuenta.
            crate::ring0::core::phase::dashboard_log("[lanzar] NO se pudo cargar la imagen");
            crate::ring0::cabina::warn("lanzar", f.motivo(), 0);
            return Informe { origen, bytes: 0, firma: None, pid: None, res: Err(f) };
        }
    };

    // ** LA MEDIDA, DICHA. Es el numero que dice si esto sirve de algo: "de 5,5
    // MB he traido 812 KB" es el escalon 2 entero en una linea. Sin apuntarlo,
    // la unica forma de saber si el cargador dejo de tragar seria cronometrar
    // arranques.
    if tam > n {
        crate::ring0::cabina::info("lanzar", "bytes que NO hubo que traer", (tam - n) as u64);
    }
    // Y el escalon 3: cuantos de los que SI se trajeron fueron del disco a su
    // sitio sin pasar por la pagina de rebote. Un camino rapido que nadie mide
    // es un camino rapido que un dia deja de tomarse en silencio.
    let (d1, r1) = crate::ring0::dev::disk::cuentas_dma();
    crate::ring0::cabina::info("lanzar", "bytes DIRECTOS del disco al buffer", d1 - d0);
    if r1 > r0 {
        crate::ring0::cabina::info("lanzar", "bytes que tuvieron que rebotar", r1 - r0);
    }
    // ** Y si alguien mas queria el disco mientras. Cada una de estas esperas
    // era, antes de que el disco tuviera dueno, **una lectura que se solapaba
    // con otra sobre la misma ranura del HBA**. El numero es la prueba de que
    // el candado hacia falta; el cero, la de que no estorba.
    // ** Y si el disco AVISO por su cuenta o hubo que seguir preguntandole.
    //
    // Este es el numero que hay que mirar en metal: `armada` dice que la placa
    // acepto la programacion de MSI, y `avisos` dice si de verdad la esta
    // enrutando. **Son cosas distintas** -- un chipset puede aceptar lo primero
    // y no hacer lo segundo, y entonces todo sigue funcionando por la red de
    // seguridad sin que nada lo diga.
    let (armada, avisos) = crate::ring0::dev::disk::irq_estado();
    if armada {
        crate::ring0::cabina::info("disk", "avisos del disco por interrupcion", avisos);
    }
    let (esperas, robos) = crate::ring0::dev::disk::cuentas_dueno();
    if esperas > 0 {
        crate::ring0::cabina::info("disk", "veces que hubo que esperar al disco", esperas as u64);
    }
    if robos > 0 {
        crate::ring0::cabina::warn("disk", "veces que hubo que quitarle el disco a un muerto", robos as u64);
    }

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

    // ** LA CONTRADICCION: los mismos bytes, leidos dos veces, distintos.
    //
    // === Por que esto no lo puede decir el que valida el formato ===
    //
    // `bex::inspect` mira UNA imagen y contesta si es valida. Cuando dice
    // `cabecera invalida` esta diciendo la verdad, y esa verdad manda a mirar el
    // fichero -- que es el sitio equivocado si el fichero esta bien.
    //
    // El cargador sabe algo que el validador no puede saber: que **hace dos
    // lecturas de la misma ruta**, la corta y la larga, y que la corta trajo un
    // `BEF1` legible. Si despues de la larga ya no lo es, ningun fichero ha
    // cambiado en el disco entre las dos: **lo que fallo fue traerlo**.
    //
    // Eso no es una linea de depuracion que se anade para una foto y se quita.
    // Es la unica conclusion que esta funcion esta en posicion de sacar, y
    // callarla fue lo que costo la tanda de fotos del 2026-08-10. Ver
    // `docs/EL_CONTRATO_DE_CARGA.md`, pieza A.
    if prologo_valido && crate::ring0::task::bex::necesita(&buf[..n]).is_err() {
        crate::ring0::cabina::fault(
            "lanzar",
            "el prologo traia un BEX valido y la lectura larga NO: fallo el TRANSPORTE, no el fichero",
            n as u64,
        );
    }

    let (res, pid) = match crate::ring0::task::proc::admit_from_disk(name, &buf[..n], tam) {
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
