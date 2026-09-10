//! **EL DUENO DE CADA MARCO: la columna que el mapa de bits no tiene.**
//!
//! [carril]  ROJO      el VOCABULARIO de los tres: el byte por marco,
//!                     `indice`, `de_byte` y las cinco cuentas
//!
//! [cuesta]  MAQUINA -- `indice` decide QUE marco se toca. Errarlo por uno no
//!           equivoca una respuesta: marca al vecino, y entonces las tres
//!           preguntas de esta carpeta contestan de otro marco (L6e)
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- son DOS tablas sobre los mismos marcos: el mapa de
//!                       bits dice si esta entregado, esta dice a quien.
//!                       Cuando discrepen manda el mapa de bits, porque aqui
//!                       no se reparte memoria
//!           SILENCIO -- **un marco sin etiquetar contesta `Anonimo`, y eso
//!                       se lee igual que "todo en orden"**. La cobertura
//!                       parcial se disfraza de salud. Por eso `cubiertos()`
//!                       existe y por eso el veredicto se llama `SinOpinion`
//!                       y no `Ok` (L6f)
//!
//! # [!] POR QUE UN FICHERO QUE NO CAMBIA NADA ES EL ROJO
//!
//! La primera version de esta cabecera puso `CARRILERA` --*no es un carril,
//! es el vocabulario*-- y **R10 la rechazo en el build**, que es lo que R10
//! esta para hacer. Al tener que elegir un color de verdad salio el argumento
//! que faltaba:
//!
//! ```text
//!    UN CIMIENTO NO PUEDE SER MAS VERDE QUE EL MAS ROJO DE LOS QUE LO USAN
//! ```
//!
//! `indice` y `de_byte` los llaman los tres carriles. Un fallo aqui **sale
//! por el rojo tambien** -- el bit de vuelo puesto en el marco de al lado no
//! da fault, no tiene sintoma y se ve tres arranques despues. Pintar de verde
//! lo que sostiene a un rojo es como se cuela un fallo rojo por un fichero
//! que nadie mira.
//!
//! ** Y a cambio este fichero se gana el derecho a ser ABURRIDO: aqui no se
//! decide nada, no hay una sola rama que elija por alguien. Solo dice que es
//! que. Lo unico que se puede hacer con un cimiento rojo es que no piense.
//!
//! # *** EL REPARTO, Y EL COLOR NO LO ELIGE NADIE
//!
//! Los tres carriles escriben en EL MISMO BYTE. Si el color saliera de *lo
//! que una pieza toca*, los tres serian del mismo color y el letrero no diria
//! nada. Sale de otro sitio:
//!
//! ```text
//!    **COMO SE ENTERA UNO DE QUE FALLO**
//!
//!    roja.rs      EL VUELO      nadie avisa. Se ve tres arranques despues,
//!                 nibble alto   con otra cara -- un dato ajeno en la memoria
//!                               de otro. Por eso se CUENTA          ROJO
//!
//!    amarilla.rs  EL MARCADO    `vmm::es_tabla` se planta y lo dice con
//!                 nibble bajo   nombre en CABINA. Se pierde lo que dependia
//!                               de esa etiqueta, no la maquina     AMARILLO
//!
//!    verde.rs     LAS CUENTAS   ni un `mut` sale de ahi. Devuelve un numero
//!                 solo lee      feo y decide quien pregunto           VERDE
//! ```
//!
//! ** Es la misma regla que el `[aparece]` de BMO C: **el carril lo decide
//! DONDE APARECE el fallo, no una opinion sobre su gravedad.** Y aqui da un
//! resultado que la intuicion no daba -- el marcado es el que puede negarle
//! una tabla de paginas al `vmm`, o sea el que suena mas grave, y aun asi es
//! el amarillo: **porque grita**.
//!
//! # [!] Y LA PARTICION ENCONTRO ALGO, QUE ES PARA LO QUE SIRVE
//!
//! El `///` de `neutros()` habia quedado pegado encima de `en_vuelo`: la
//! documentacion de una funcion describiendo otra, en el mismo `rustdoc`, sin
//! que ningun compilador tenga forma de objetar. Al repartir por carril hubo
//! que decidir a cual iba cada linea, y ahi se cayo sola.
//!
//! > Partir por ejes no es ordenar. Es hacer preguntas que estando junto todo
//! > nadie hace.
//!
//! # Lo que NO se partio, y por que
//!
//! `Titular`, `TABLA`, `indice` y las cinco cuentas se quedan aqui. No es
//! comodidad: **son de los tres**. Bajar `indice` al carril rojo obligaria al
//! amarillo a subir a por el, y una dependencia entre carriles convierte el
//! letrero de la carpeta en decoracion -- que es exactamente lo que R9 le
//! objeto a este fichero cuando quiso vivir dentro de `phys/`.
//!
//!
//! # *** POR QUE EXISTE: el fichero de al lado promete esto en su titulo
//!
//! `phys/roja.rs` se llama, literalmente:
//!
//! > **EL BITMAP: quien es dueno de cada marco**
//!
//! Y no lo sabe. Tiene UN BIT por marco --entregado o libre-- y con eso
//! `free_frame` solo puede decir una cosa:
//!
//! ```text
//!    "ya estaba libre"        <- lo unico que sabe decir
//!    "ese marco no es tuyo"   <- lo que hacia falta el 04-09
//! ```
//!
//! ## El dia que se pago
//!
//! DOOM murio, `destroy_address_space` recorrio su arbol de paginas, y en la
//! tabla `4D2000` encontro trece casillas cuyo contenido era **codigo
//! maquina**: `5053544156415741` es `push r15; push r14; push r12; push rbx;
//! push rax`. Ese marco se solto, se volvio a entregar, alguien cargo un
//! programa encima, y el recorrido seguia teniendolo por una tabla de paginas.
//!
//! El asignador lo acepto todo sin una palabra, porque **no tenia con que
//! objetar**. Y la pantalla azul si sabe reconstruir de quien era un marco
//! --preguntandole a `obj::memory` y a `obj::file`-- pero eso es ARQUEOLOGIA:
//! se averigua el dueno con la maquina ya muerta, no en el instante en que
//! alguien se equivoco.
//!
//! # La regla, y es la que hace que esto sea seguro de encender
//!
//! ```text
//!    los dos declaran y COINCIDEN   -> adelante
//!    los dos declaran y DIFIEREN    -> se rehusa, y se dice quien es quien
//!    alguno NO declara              -> SIN OPINION. Adelante, y callado
//! ```
//!
//! ** La tercera fila es la que permite adoptar esto sin convertir los 34
//! sitios que llaman al asignador. Un marco que nadie etiqueto no puede
//! producir un rechazo, asi que **la cobertura parcial no puede provocar una
//! fuga**. La cobertura crece sitio a sitio y el juez nunca miente sobre lo que
//! no sabe.
//!
//! # ** POR QUE VIVE AQUI Y NO DENTRO DE `phys/`
//!
//! Ahi era donde tenia sentido por afinidad, y la ley R9 lo rechazo con razon:
//! `mm/phys/` es una CARPETA DE CARRILES, y en una carpeta de carriles solo
//! caben `roja.rs`, `amarilla.rs` y `verde.rs`. Un cuarto fichero al lado
//! convierte el letrero de la carpeta en decoracion.
//!
//! Y al mirarlo con la regla puesta, la regla tenia el mejor argumento: esto no
//! es un carril del asignador. **`phys` reparte RAM y esta tabla no reparte
//! nada** -- vive del mismo tamano que sus marcos y opina sobre ellos, igual
//! que `vmm` opina sobre ellos por otro lado. Su sitio es el piso donde los dos
//! se ven.
//!
//! # Lo que cuesta, dicho en voz alta
//!
//! Un byte por marco. Con el techo del physmap en 16 GiB son 4.194.304 marcos,
//! o sea **4 MiB de BSS** -- ocho veces el mapa de bits, que son 512 KiB. Se
//! paga entero y a proposito: la alternativa era medio byte por marco (2 MiB y
//! dieciseis clases) y empaquetar nibbles dentro de una decision de vida o
//! muerte para ahorrar 2 MiB de quince mil no es un ahorro, es una trampa.


use super::{PAGE, PHYSMAP_SIZE};

/// Cuantos marcos cubre la tabla. El MISMO techo que el mapa de bits, y por eso
/// esta el `[riesgo] ESPEJO` arriba: si uno se mueve, este se mueve con el.
const MARCOS: usize = (PHYSMAP_SIZE / PAGE) as usize;

/// **De quien es un marco.** No es un `pid`: es PARA QUE se pidio.
///
/// Un `pid` contestaria "de la tarea 7", y la pregunta que hunde la maquina no
/// es esa -- es *"esto es una tabla de paginas o es el codigo de alguien?"*.
/// Los duenos de un marco en este kernel son SUBSISTEMAS antes que procesos.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Titular {
    /// Libre. Nadie lo tiene.
    Nadie = 0,
    /// Entregado, y quien lo pidio no dijo para que. **No es un error**: es la
    /// mayoria del kernel todavia, y significa SIN OPINION.
    Anonimo = 1,
    /// Una tabla de paginas: PML4, PDPT, PD o PT.
    Tabla = 2,
    /// Una hoja de un espacio de Ring 3 -- imagen o pila de usuario.
    Hoja = 3,
    /// Una pila de tarea, de kernel o de aterrizaje.
    Pila = 4,
    /// Un bloque de `obj::memory`.
    Bloque = 5,
    /// El bufer de un fichero reflejado.
    Bufer = 6,
    /// Una estructura del propio kernel.
    Kernel = 7,
    /// **NEUTRO: un aparato escribe aqui por DMA, y no obedece a nadie.**
    ///
    /// == *** LA NOVENA CLASE, Y LA UNICA QUE NO ES DE ESTE PROCESADOR ======
    ///
    /// Las ocho de arriba son cosas que el CPU escribe. Esta no: es un marco
    /// que **el AHCI, la tarjeta de red, el xHC o la GPU** tienen programado
    /// como destino, y en el que van a escribir cuando les parezca -- sin
    /// pedir permiso, sin capability y sin que el orquestador se entere.
    ///
    /// ** Hasta hoy esos marcos salian `Anonimo`, que significa SIN OPINION.
    /// O sea que los unicos marcos del sistema en los que escribe alguien a
    /// quien no se puede parar eran, justamente, sobre los que el juez se
    /// callaba.
    ///
    /// > El marco mas peligroso de la maquina era el que no tenia etiqueta.
    ///
    /// [!] Y esta clase NO se devuelve nunca. Los cuatro sitios que la piden
    /// la piden UNA vez, en el arranque, y viven lo que vive el kernel. Si un
    /// dia aparece uno que la suelte, `puede_soltar` ya sabra decir que no --
    /// que es media respuesta a la pista 1.5 de la hoja del 07-09.
    ///
    /// Ver `NEUTRO/LEY.md`, regla N2.
    Neutro = 8,
}

impl Titular {
    /// El nombre, para CABINA. Un numero en una pantalla que se lee con una
    /// camara no lo descifra nadie -- es la leccion del `motivo` de la morgue.
    pub fn nombre(self) -> &'static str {
        match self {
            Titular::Nadie => "libre",
            Titular::Anonimo => "anonimo",
            Titular::Tabla => "TABLA",
            Titular::Hoja => "hoja",
            Titular::Pila => "pila",
            Titular::Bloque => "bloque",
            Titular::Bufer => "bufer",
            Titular::Kernel => "kernel",
            // En MAYUSCULAS como `TABLA`, y por el mismo motivo: los dos
            // aparecen en una pantalla azul que se lee con una camara, y los
            // dos significan "esto no lo tocaba quien creias".
            Titular::Neutro => "NEUTRO (un aparato)",
        }
    }

    fn de_byte(b: u8) -> Titular {
        // ** SOLO EL NIBBLE BAJO. El alto lo usa EL BIT EN VUELO desde el
        // 2026-09-09 (paso N4): el titular vive en 0..8 y le sobraban
        // cuatro bits en el mismo byte. Ver `en_vuelo`.
        match b & 0x0F {
            2 => Titular::Tabla,
            3 => Titular::Hoja,
            4 => Titular::Pila,
            5 => Titular::Bloque,
            6 => Titular::Bufer,
            7 => Titular::Kernel,
            8 => Titular::Neutro,
            1 => Titular::Anonimo,
            // [!] Un byte que esta tabla no sabe producir se lee como `Nadie` y
            // NO como un dueno inventado. Un juez que se inventa una respuesta
            // ante un dato corrupto es peor que uno que se calla.
            _ => Titular::Nadie,
        }
    }
}

/// La tabla. Vive en BSS por lo mismo que el mapa de bits: existe antes de que
/// haya asignador, asi que no puede pedirsela a nadie.
static mut TABLA: [u8; MARCOS] = [0; MARCOS];

#[allow(static_mut_refs)]
fn tabla() -> &'static mut [u8; MARCOS] {
    unsafe { &mut TABLA }
}

fn indice(phys: u64) -> Option<usize> {
    if phys % PAGE != 0 || phys >= PHYSMAP_SIZE {
        return None;
    }
    Some((phys / PAGE) as usize)
}

// == LA CUENTA DEL NEUTRO, Y POR QUE SE LLEVA AQUI ==========================
//
// `NEUTRO/LEY.md` regla N4 pide que la cantidad de marcos de aparato se pueda
// mirar y este QUIETA. La primera version la contaba recorriendo la tabla
// entera -- cuatro millones de entradas con el techo de 16 GiB-- y esa version
// no se podia poner en un panel que se repinta.
//
// ** Asi que se lleva AQUI, que es el unico sitio por el que un marco cambia de
// dueno. Dos comparaciones por marcado, y el numero esta siempre listo.
//
// *** Y AL LLEVARLA APARECIO ALGO QUE NO SE BUSCABA.
//
// Si la cuenta puede SUBIR cuando un marco pasa a `Neutro`, tambien puede BAJAR
// cuando deja de serlo. Y un marco de aparato que deja de ser de un aparato es
// **exactamente lo que N3 prohibe**:
//
// ```text
//    N3   un marco neutro NO se devuelve
// ```
//
// [!] Lo importante es COMO se detecta: **desde el lado del marcado, sin tocar
// el camino de devolucion de marcos**. Ese camino es ROJO, es donde vive la
// azul del 07-09, y `NEUTRO/REQUISITOS.md` (R4) dice que no se toca hasta
// haberla reproducido. Esto lo vigila sin entrar.
//
// > Se puede saber que una regla se rompio sin ponerse delante de ella.

/// Marcos que AHORA MISMO son de un aparato.
static mut NEUTROS_VIVOS: u64 = 0;
/// Veces que un marco dejo de ser de un aparato. **Tiene que ser CERO**: si no
/// lo es, N3 se rompio y aqui esta la prueba. Ver la nota de arriba.
static mut NEUTROS_SOLTADOS: u64 = 0;

// == *** EL BIT EN VUELO -- EL PASO N4 =====================================
//
// # Por que existe, y lo dijo el juez del DMA antes que nadie
//
// `bmo-dma-juicio` pregunta *"se le puede dar esta direccion a este aparato?"*
// y tiene DOS formas de contestar que si:
//
// ```text
//    1. el marco es SUYO          `Titular::Neutro` y su corral
//    2. se lo PRESTARON para esto  <- este campo no lo podia rellenar nadie
// ```
//
// *** El caso 2 aparecio al ir a cablear el juez en el AHCI: el camino DIRECTO
// de una lectura le da al disco la direccion del **bufer del que llamo**, que
// no es del aparato y no tiene por que serlo. Un juez con una sola regla habria
// rechazado una lectura legitima **y dejado el disco sin arrancar**.
//
// # ** LA PREGUNTA QUE NINGUNA IOMMU CONTESTA
//
// ```text
//    DONDE puede escribir un aparato   -> una IOMMU
//    CUANDO puede escribir             -> ESTO
// ```
//
// Un aparato con DMA en vuelo sobre un bufer **ya liberado** escribe en una
// direccion que la IOMMU considera legitima: el mapeo es valido, el permiso
// existe, y el dato aterriza encima de otra cosa. La IOMMU contesta *"puede
// tocar esta pagina"*, y la respuesta correcta era *"ya no"*.
//
// # Donde vive, y por que no hace falta una tabla nueva
//
// ```text
//    bits 0..3   el titular      0..8, y sobran cuatro
//    bits 4..7   EL APARATO      0 = nada en vuelo, 1..15 = quien
// ```
//
// ** Cero bytes de memoria nueva. `Titular` nunca paso de 8, asi que el nibble
// alto llevaba libre desde el primer dia -- y el byte ya se leia y escribia en
// un solo sitio (`marcar`), que es lo que hace esto seguro.
//
// [!] Quince aparatos como maximo. Hoy hay TRES censados y la GPU sera el
// cuarto (`NEUTRO/CENSO.txt`). Si algun dia hicieran falta mas, esto es una
// tabla aparte y no un rediseno -- pero inventarla hoy seria pagar por una
// maquina que no existe (LEY 24).

/// Marcos con un DMA EN VUELO ahora mismo.
///
/// ** Sube al programar un descriptor y baja al consumir el fin. **Al apagar
/// tiene que ser CERO**: cualquier otra cosa es un aparato al que se le pidio
/// algo y nadie recogio la respuesta.
static mut EN_VUELO_VIVOS: u64 = 0;
/// Veces que un marco cambio de titular **con un DMA en vuelo**.
///
/// *** TIENE QUE SER CERO. Cada uno es un bufer reasignado mientras un aparato
/// todavia escribia en el, y no da fault: da un dato ajeno apareciendo en la
/// memoria de otro, mas tarde. Lo cuenta [`marcar`], desde el lado del marcado
/// y sin entrar en el camino rojo de devolucion.
static mut EN_VUELO_PISADOS: u64 = 0;
/// Veces que se pidio poner en vuelo un marco que YA lo estaba, para OTRO
/// aparato. Es el caso mas peligroso de todos: dos aparatos sobre el mismo
/// bufer. Se rechaza Y se cuenta.
static mut EN_VUELO_CHOQUES: u64 = 0;

// == *** EL PERRO GUARDIAN DEL PLAZO -- EL PASO N5 =========================
//
// ** Son 360 bytes para las cuatro tablas, contra los 32 MiB que costaria un
// sello de tiempo por marco. El razonamiento entero --por que el plazo es del
// APARATO y por que lo que se mide es el SILENCIO y no la duracion-- esta en
// la nota del plazo de `roja.rs`, que es quien las escribe.
//
// [!] Y viven aqui por lo mismo que las otras cinco: las escribe el rojo y las
// lee el verde. Una cuenta que se lleva en un carril y se ensena en otro es de
// los dos, y ponerla en cualquiera de ellos obligaria al otro a subir a por
// ella -- que es como el letrero de la carpeta se vuelve decoracion.

/// Vuelos abiertos AHORA MISMO de cada aparato. Indice = su numero (1..15).
static mut VUELOS_DE: [u32; 16] = [0; 16];
/// Cuando se supo de cada aparato por ultima vez -- despegue o aterrizaje.
///
/// ** El numero es OPACO: lo trae quien programa el descriptor y aqui no se
/// sabe de que reloj sale. Cero significa **nunca se supo nada de el**, que no
/// es lo mismo que llevar mucho callado.
static mut ULTIMA_NOTICIA: [u64; 16] = [0; 16];
/// El PEOR silencio visto de cada aparato teniendo trabajo pendiente.
///
/// *** Esto es N5b: **el plazo sale de aqui**, con margen, despues de varios
/// arranques. No se elige (LEY 24). Y es el peor y no el minimo porque un
/// plazo se pone en la cola, no en el mejor caso.
static mut PEOR_SILENCIO: [u64; 16] = [0; 16];
/// Vuelos que pasaron de plazo. **R-DMA-8: tiene que ser CERO.** Hoy no puede
/// subir, porque el plazo todavia no se ha medido y vale cero.
static mut CADUCADOS: u64 = 0;

mod amarilla;
mod roja;
mod verde;

// *** LOS TRES CARRILES SE REEXPORTAN, asi que `mm::titular::marcar` sigue
// escribiendose igual desde los 34 sitios que llaman al asignador. Partir por
// dentro no puede costarle una linea a quien llama: si costara, la particion
// se estaria pagando con el diff de otro.
pub use amarilla::{marcar, puede_soltar, titular_de, Veredicto};
pub use roja::{aterrizo, caducados, en_vuelo, en_vuelo_de, APARATO_AHCI,
               APARATO_GPU, APARATO_NIC, APARATO_XHCI, PLAZO_SIN_MEDIR};
pub use verde::{cubiertos, neutros, peor_silencio, vuelos};


/// **EL GUARDIAN DEL TECHO**, y corre en compilacion.
///
/// La tabla y el mapa de bits cuentan los mismos marcos. Si `PHYSMAP_SIZE`
/// cambia y uno de los dos no se entera, este kernel no llega a enlazar -- que
/// es justo lo que no paso el 30-08, cuando `MAX_PHYS` y `caminable` juzgaban
/// la misma direccion con techos distintos y la maquina se paro.
const _: () = {
    assert!(MARCOS == (PHYSMAP_SIZE / PAGE) as usize);
    assert!(Titular::Nadie as u8 == 0, "libre TIENE que ser el cero del BSS");
};
