//! **CUAL DE LAS TRES FORMAS LE TOCA A ESTA PETICION -- Y POR QUE.**
//!
//! [carril]  AMARILLO  no toca hardware ni memoria de nadie: contesta una
//!                     pregunta con numeros. Es peligroso de CREER
//!
//! [cuesta]  TAREA -- elegir mal no rompe la maquina: elegir REBOTE de mas
//!           cuesta dos `memcpy` que no hacian falta, y elegir DIRECTO de mas
//!           lo caza `bmo_ahci` con `BadRequest` y la lectura sale corta. Se
//!           pierde lo que dependia de esa lectura, no el sistema (L6e)
//!
//! [riesgo]  AJENO -- **todos los numeros entran de fuera**: la ventana del
//!           espejo, la alineacion que exige el aparato, hasta donde
//!           direcciona. Aqui no se comprueba ninguno contra la maquina porque
//!           desde aqui no se ve la maquina. Si el que llama miente, esto
//!           contesta con seguridad una respuesta falsa (L6f)
//!
//! # *** POR QUE EXISTE: la eleccion ya se tomaba, y no decia por que
//!
//! `NEUTRO/DMA/INTELIGENTE.txt` lleva escrito desde el 09-09 que hay TRES
//! formas de darle memoria a un aparato y que lo inteligente es **elegir por
//! camino**:
//!
//! ```text
//!    1. SU PROPIO CORRAL       una arena suya. La direccion no PUEDE estar mal
//!    2. PRESTADO PARA ESTA VEZ el bufer del que llamo, tal cual. Cero copias
//!    3. REBOTE                 se copia a una pagina que si vale, y de vuelta
//! ```
//!
//! ** Y el AHCI ya elegia entre 2 y 3. Lo hacia dentro de `tramo_dma`, con
//! cuatro `return None` seguidos, y **los cuatro se veian igual desde fuera**:
//!
//! ```text
//!    Some(..)  -> directo
//!    None      -> rebote        <- y aqui se perdia el motivo
//! ```
//!
//! *** Un rebote sin motivo es un numero que no se puede usar. `cuentas_dma`
//! decia *"rebotaron 40 MiB"* y con eso no se puede hacer NADA: no se sabe si
//! sobra alineacion, si hay bufers fuera del espejo, o si es un tamano. Los
//! tres se arreglan de formas distintas y ninguno se parece a los otros.
//!
//! > Una decision que no dice por que no se puede mejorar: solo se puede
//! > deshacer entera y ver que pasa.
//!
//! # LO QUE ESTE CRATE ES, DICHO EN UNA LINEA
//!
//! Los mismos cuatro `if` de antes, **fuera del kernel, con nombre cada uno, y
//! con pruebas que se ejecutan en el anfitrion**. No es logica nueva: es la
//! misma logica en un sitio donde se le puede preguntar.
//!
//! ** Y esa es la diferencia con [`bmo_dma_juicio`], que es el otro crate del
//! DMA y contesta otra cosa:
//!
//! ```text
//!    bmo-dma-juicio   SE PUEDE dar esta direccion a este aparato?   permiso
//!    ESTE             CUAL de las tres formas le toca?              forma
//! ```
//!
//! [!] Y el orden importa: primero se elige la forma, y la direccion que salga
//! de ahi es la que va al juez. Al reves seria juzgar una direccion que todavia
//! no se sabe si se va a usar.
//!
//! [`bmo_dma_juicio`]: ../bmo_dma_juicio/index.html

#![no_std]

/// Las tres formas de darle memoria a un aparato.
///
/// Son las de `NEUTRO/DMA/INTELIGENTE.txt` y no hay una cuarta: el dia que
/// aparezca, este `enum` es donde se nota.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forma {
    /// Sale de la arena del propio aparato. **La direccion no puede estar mal**,
    /// asi que no hay nada que comprobar.
    Corral,
    /// El aparato escribe DIRECTAMENTE en el bufer del que llamo. Cero copias,
    /// y es la unica que necesita saber CUANDO -- de ahi el bit en vuelo.
    Prestado,
    /// Se copia a una pagina que si vale, se manda, y se copia de vuelta.
    Rebote,
}

/// **EL MOTIVO. Vocabulario CERRADO, y esa es toda su gracia.**
///
/// *** Si un caso nuevo no cabe en estas seis palabras, **no se ha entendido el
/// caso**. Anadir `Otros` seria devolver al estado de antes --todos los rebotes
/// iguales-- con el trabajo hecho y sin el beneficio.
///
/// ** Y se cuentan por separado porque los tres motivos de rebote se arreglan
/// de formas que no se parecen en nada:
///
/// ```text
///    FueraDelEspejo   se arregla MOVIENDO el bufer, no tocando el disco
///    Desalineado      se arregla en quien pide, con un `+ 1 & !1`
///    NoCabeElTramo    se arregla pidiendo mas de golpe
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PorQue {
    /// CORRAL: sale de su propia arena.
    EsSuyo,
    /// PRESTADO: el destino ya sirve tal cual, sin tocar nada.
    ElDestinoYaSirve,
    /// REBOTE: la direccion no cae en el espejo, asi que su fisica **habria que
    /// preguntarla** caminando tablas -- y esta casa ya pago por fiarse de esa
    /// respuesta (el arranque del 2026-08-10).
    FueraDelEspejo,
    /// REBOTE: la base no cumple la alineacion que el aparato exige.
    Desalineado,
    /// REBOTE: lo que hay seguido no llega ni a la unidad minima del aparato.
    NoCabeElTramo,
    /// REBOTE: la fisica cae por encima de lo que el aparato sabe direccionar.
    NoLoDireccionaElAparato,
}

impl PorQue {
    /// El nombre, para CABINA. Mismo motivo que `Titular::nombre`: un numero en
    /// una pantalla que se lee con una camara no lo descifra nadie.
    pub fn nombre(self) -> &'static str {
        match self {
            PorQue::EsSuyo => "es su corral",
            PorQue::ElDestinoYaSirve => "el destino ya sirve",
            PorQue::FueraDelEspejo => "fuera del espejo",
            PorQue::Desalineado => "desalineado",
            PorQue::NoCabeElTramo => "no cabe el tramo",
            PorQue::NoLoDireccionaElAparato => "no lo direcciona el aparato",
        }
    }

    /// Cuantos motivos hay. Para dimensionar la tabla de cuentas del que llama.
    pub const CUANTOS: usize = 6;

    /// El indice de este motivo, para esa tabla.
    pub fn indice(self) -> usize {
        match self {
            PorQue::EsSuyo => 0,
            PorQue::ElDestinoYaSirve => 1,
            PorQue::FueraDelEspejo => 2,
            PorQue::Desalineado => 3,
            PorQue::NoCabeElTramo => 4,
            PorQue::NoLoDireccionaElAparato => 5,
        }
    }
}

/// Lo que hay que saber para elegir. **Todo son numeros**, y ninguno se mira
/// contra la maquina desde aqui -- ver el `[riesgo] AJENO` de la cabecera.
#[derive(Clone, Copy, Debug)]
pub struct Peticion {
    /// Donde esta el bufer del que llamo, en virtual.
    pub virt: u64,
    /// Cuanto quiere, en bytes.
    pub bytes: u64,
    /// La ventana del espejo lineal: `virt = fisica + espejo_base`.
    pub espejo_base: u64,
    /// Cuanto mide esa ventana.
    pub espejo_bytes: u64,
    /// Lo que el aparato exige de la BASE. Potencia de dos; 1 = le da igual.
    pub alineacion: u64,
    /// La unidad indivisible del aparato. Un sector, para un disco.
    pub minimo: u64,
    /// Hasta donde sabe direccionar. 32 para el PRDT de AHCI, 64 para nada.
    pub bits_de_cuenta: u32,
    /// Si esta memoria ya sale de la arena del propio aparato.
    pub es_su_corral: bool,
}

/// La respuesta: la forma, el motivo, y lo que se puede usar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Eleccion {
    pub forma: Forma,
    pub por_que: PorQue,
    /// La fisica que le toca al aparato. **Solo vale si `forma != Rebote`**:
    /// para un rebote la direccion la pone el que rebota, no esto.
    pub fisica: u64,
    /// Cuanto de lo pedido cabe por esa forma. Puede ser menos que `bytes`.
    pub bytes: u64,
}

/// **ELEGIR LA FORMA.** El orden de las preguntas NO es de gusto.
///
/// ```text
///    1. es su corral?        -> CORRAL. No hay nada que comprobar
///    2. cabe el minimo?      -> si no, REBOTE: no hay tramo que dar
///    3. cae en el espejo?    -> si no, REBOTE: la fisica habria que preguntarla
///    4. esta alineado?       -> si no, REBOTE
///    5. lo direcciona?       -> si no, REBOTE
///    6.                      -> PRESTADO
/// ```
///
/// *** La 1 va primero porque **es la unica que hace innecesarias a las otras
/// cinco**. Un corral se calcula como `base + i * tamano` dentro de una arena
/// que se pidio entera: no puede estar desalineado ni fuera de sitio, y
/// comprobarlo seria comprobar una construccion. Es lo que hace la NIC, y es la
/// forma que `DMA_MAESTRO.md` llama *la tercera escuela*.
///
/// ** Y la 2 va antes que la 3 a proposito: una peticion que no llega ni a un
/// sector no se rechaza por DONDE esta, sino por lo que MIDE. Preguntarlo al
/// reves daria `FueraDelEspejo` para algo que tampoco habria servido estando
/// dentro -- y entonces la cuenta de motivos mentiria justo donde se usa.
pub fn elegir(p: &Peticion) -> Eleccion {
    // 1. Su corral: la direccion no puede estar mal.
    if p.es_su_corral {
        return Eleccion {
            forma: Forma::Corral,
            por_que: PorQue::EsSuyo,
            fisica: p.virt.wrapping_sub(p.espejo_base),
            bytes: p.bytes,
        };
    }
    let rebote = |q| Eleccion { forma: Forma::Rebote, por_que: q, fisica: 0, bytes: 0 };

    // 2. Que haya algo que dar.
    if p.minimo != 0 && p.bytes < p.minimo {
        return rebote(PorQue::NoCabeElTramo);
    }

    // 3. El espejo. Fuera de el la fisica hay que PREGUNTARLA, y esta casa ya
    //    pago por fiarse de esa respuesta: la misma lectura funcionaba con el
    //    destino en `.bss` y fallaba en la pila (2026-08-10).
    let fin = p.espejo_base.wrapping_add(p.espejo_bytes);
    if p.virt < p.espejo_base || p.virt >= fin {
        return rebote(PorQue::FueraDelEspejo);
    }

    // 4. La alineacion que exige el aparato.
    if p.alineacion > 1 && p.virt & (p.alineacion - 1) != 0 {
        return rebote(PorQue::Desalineado);
    }

    let fisica = p.virt - p.espejo_base;

    // 5. Hasta donde llega el campo de direccion del aparato. Un PRDT de AHCI
    //    lleva 32+32 bits, y `bits_de_cuenta = 64` significa "no acota".
    if p.bits_de_cuenta < 64 {
        let techo = 1u64 << p.bits_de_cuenta;
        // ** Se comprueba el FINAL y no solo la base: un tramo que empieza por
        // debajo del techo y lo cruza es exactamente el fallo que un aparato de
        // 32 bits no puede avisar -- se le da la vuelta al contador y escribe
        // en la direccion baja.
        if fisica >= techo || fisica.wrapping_add(p.bytes) > techo {
            return rebote(PorQue::NoLoDireccionaElAparato);
        }
    }

    // 6. Prestado: el destino sirve tal cual. Lo que quede de ventana, acotado.
    let bytes = p.bytes.min(fin - p.virt);
    if p.minimo != 0 && bytes < p.minimo {
        return rebote(PorQue::NoCabeElTramo);
    }
    Eleccion {
        forma: Forma::Prestado,
        por_que: PorQue::ElDestinoYaSirve,
        fisica,
        bytes,
    }
}

#[cfg(test)]
mod pruebas;
