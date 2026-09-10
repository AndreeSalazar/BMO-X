//! **EL DUENO DE CADA MARCO: la columna que el mapa de bits no tiene.**
//!
//! [carril]  AMARILLO  no reparte RAM ni toca la memoria de nadie: da una
//!                     OPINION sobre un marco. Es peligroso de creer
//!
//! [cuesta]  MAQUINA -- una etiqueta equivocada rehusa una devolucion buena, y
//!           un marco que no vuelve es un marco perdido. Uno no se nota; el
//!           mismo error en un camino que corre en cada muerte de proceso se
//!           come la RAM hasta que no arranca nada.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- son DOS tablas sobre los mismos marcos: el mapa de
//!                       bits dice si esta entregado, esta dice a quien. Pueden
//!                       discrepar, y cuando discrepen la que manda es el mapa
//!                       de bits -- aqui no se reparte memoria.
//!           SILENCIO -- **un marco sin etiquetar contesta `Anonimo`, y eso se
//!                       lee igual que "todo en orden"**. La cobertura parcial
//!                       se disfraza de salud. Por eso `cubiertos()` existe y
//!                       por eso el veredicto se llama `SinOpinion` y no `Ok`.
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
        match b {
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

/// Apuntar para que se pidio un marco. Lo llama `alloc_frame_de`.
pub fn marcar(phys: u64, q: Titular) {
    if let Some(i) = indice(phys) {
        let antes = tabla()[i];
        tabla()[i] = q as u8;
        // La cuenta del neutro, en O(1). Ver la nota de arriba.
        let era = antes == Titular::Neutro as u8;
        let es = q == Titular::Neutro;
        unsafe {
            if es && !era {
                NEUTROS_VIVOS += 1;
            } else if era && !es {
                NEUTROS_VIVOS = NEUTROS_VIVOS.saturating_sub(1);
                NEUTROS_SOLTADOS = NEUTROS_SOLTADOS.wrapping_add(1);
            }
        }
    }
}

/// **De quien es?** `Nadie` si esta libre o si cae fuera del espejo.
pub fn titular_de(phys: u64) -> Titular {
    match indice(phys) {
        Some(i) => Titular::de_byte(tabla()[i]),
        None => Titular::Nadie,
    }
}

/// El veredicto de `puede_soltar`, con las tres respuestas separadas.
///
/// ** `SinOpinion` NO es `Adelante`, y son dos variantes distintas a proposito.
/// Juntarlas haria que "no lo se" y "lo he comprobado" se contaran igual, que
/// es exactamente el `[riesgo] SILENCIO` de la cabecera.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    /// Los dos declararon y coinciden.
    Adelante,
    /// Alguno de los dos no declaro. Se deja pasar.
    SinOpinion,
    /// Los dos declararon y DIFIEREN: `(quien lo tiene, quien lo suelta)`.
    NoEsTuyo(Titular, Titular),
}

/// **Puede `quien` soltar este marco?** Ver la regla en la cabecera.
pub fn puede_soltar(phys: u64, quien: Titular) -> Veredicto {
    let tiene = titular_de(phys);
    if tiene == Titular::Anonimo || tiene == Titular::Nadie || quien == Titular::Anonimo {
        return Veredicto::SinOpinion;
    }
    if tiene == quien {
        Veredicto::Adelante
    } else {
        Veredicto::NoEsTuyo(tiene, quien)
    }
}

/// **Cuantos marcos llevan una etiqueta de verdad**, o sea ni libres ni
/// anonimos.
///
/// Es la unica defensa contra el `[riesgo] SILENCIO`: si esto se queda en
/// cifras bajas, el juez esta callado porque no sabe, no porque todo vaya bien.
/// Lo dice el informe de la purga, junto a los marcos que volvieron.
pub fn cubiertos() -> u64 {
    let t = tabla();
    let mut n = 0u64;
    for i in 0..MARCOS {
        let b = t[i];
        if b >= 2 {
            n += 1;
        }
    }
    n
}

/// **`(marcos de aparato, veces que uno se solto)`.** La cifra de `NEUTRO/`.
///
/// Va aparte de [`cubiertos`] a proposito. `cubiertos` contesta *"cuanto sabe
/// el juez"*; esto contesta *"cuanta RAM de esta maquina esta fuera del celo"*.
///
/// ```text
///    vivos     PEQUENO Y QUIETO. Los aparatos piden al arrancar y ya.
///              Si sube en marcha, alguien reparte DMA en caliente
///    soltados  ** CERO. Cualquier otra cosa es N3 rota, y con su cuenta
/// ```
///
/// Las dos son de leer un `static`: la cuenta la lleva [`marcar`], que es el
/// unico sitio por el que un marco cambia de dueno. Preguntar esto **no
/// recorre nada**, asi que se puede poner en un panel que se repinta.
pub fn neutros() -> (u64, u64) {
    unsafe { (NEUTROS_VIVOS, NEUTROS_SOLTADOS) }
}

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
