//! **EL JUEZ DEL DMA** -- se le puede dar esta direccion fisica a este aparato?
//!
//! generacion: nieto
//!
//! [cuesta]  MAQUINA -- por herencia, igual que `bmo-mmio-juicio`. No toca
//!           hardware ni tiene un solo `unsafe`, pero los drivers DECIDEN con
//!           su respuesta si programan un aparato con DMA. Instrumentar no
//!           contagia el coste; decidir si (L6e)
//!
//! [riesgo]  SILENCIO -- un veredicto de mas no da un fault: da un aparato
//!           escribiendo donde no debia, y eso se ve tres arranques despues en
//!           forma de otra cosa. Es el `[riesgo]` que la casa ya le puso a
//!           `bmo-mmio-juicio`, y por el mismo motivo (L6f)
//!
//! El plan: `docs/plan/PLAN_EL_NEUTRO_VIGILADO.md`, pasos N0 y N1. El censo
//! que lo pidio: `NEUTRO/DMA/EMBUDO.txt`. Lo que copia del mundo:
//! `docs/maestro/DMA_MAESTRO.md`.
//!
//! # *** N0 Y N1 SON LA MISMA PIEZA, Y ESO SE DESCUBRIO AL ESCRIBIRLA
//!
//! El plan pedia dos cosas por separado:
//!
//! ```text
//!    N0   que solo haya UN sitio por donde salga una direccion fisica
//!    N1   un juez que diga que no
//! ```
//!
//! ** Y N0, tal como estaba escrito --*"hacer que todo pase por una funcion"*--
//! es una regla que hay que **acordarse** de cumplir. El decimoquinto sitio
//! entra igual; solo que ahora, ademas, hay una funcion que nadie le obligo a
//! llamar.
//!
//! *** La forma que SI se cumple sola es un TIPO. [`Prenda`] envuelve un `u64`
//! y **solo se puede construir con [`juzgar`]**. Si el que escribe el
//! descriptor pide una `Prenda` en vez de un `u64`, entonces:
//!
//! ```text
//!    el embudo lo cuenta el COMPILADOR, no un grep
//!    rodearlo no es "olvidarse": es un error de tipos
//!    y el numero de constructores es 1 por construccion
//! ```
//!
//! > Un contrato, no un cerebro. Es la regla de la casa, y aqui cae sola.
//!
//! # ** POR QUE ESTE JUEZ NO COMPARTE VOCABULARIO CON NADIE
//!
//! Un juez de esta casa suele tener un `[riesgo] ESPEJO`: la misma pregunta
//! escrita en dos sitios que se separan. Este **no lo tiene**, y es a proposito.
//!
//! ```text
//!    NO recibe `Duenno::Neutro`     recibe un `bool`: `es_neutro`
//!    NO recibe un enum de aparatos  recibe un `u16` que solo compara consigo
//! ```
//!
//! *** El kernel escribe `phys::duenno_de(p) == Duenno::Neutro` y pasa el
//! resultado. Si algun dia esa etiqueta cambia de nombre, de numero o de
//! fichero, **aqui no hay nada que actualizar**: no hay copia que se pueda
//! desincronizar porque no hay copia.
//!
//! ** Y el `u16` del aparato: a este juez le da igual lo que signifique. Solo
//! comprueba que el del marco y el del que va a escribir sean **el mismo**. Un
//! juez que no sabe los nombres no puede equivocarse de nombre.
//!
//! [!] Lo que eso SACRIFICA (L3): los mensajes de error no pueden decir *"la
//! tarjeta de red"*, solo *"el aparato 2"*. El que lea el veto tiene que ir a
//! `NEUTRO/CENSO.txt` a traducirlo. Se acepta: **un numero que no miente vale
//! mas que un nombre que se puede quedar viejo.**
//!
//! # Las seis preguntas, y por que son seis y no cinco
//!
//! ```text
//!    1. el marco es de un aparato, o se lo PRESTARON?   si no, es de otro
//!    2. es de ESTE aparato?                              si no, uno pisa a otro
//!    3. pide algo?                          `bytes == 0` es un fallo, no un no-op
//!    4. cabe dentro del marco?              el desbordamiento clasico
//!    5. esta alineada?                      lo que el aparato exija
//!    6. cabe en el ancho del descriptor?    el PRDT de AHCI cuenta 22 bits
//! ```
//!
//! ** La 3 no estaba en el plan y salio al escribir las pruebas: un DMA de cero
//! bytes pasa las otras cinco y **el aparato hace algo indefinido**. Es la clase
//! de caso que solo aparece cuando alguien tiene que decidir que devuelve.
//!
//! # [!] LO QUE ESTE JUEZ NO PUEDE CONTESTAR
//!
//! ```text
//!    [ ] no dice si el bufer SIGUE siendo del aparato. Eso es el CUANDO, y es
//!        el paso N4 del plan: un bit EN VUELO en el marco. Ninguna cantidad de
//!        comprobacion de direcciones lo sustituye
//!    [ ] no dice si el aparato hara lo que le pidieron. Para eso hace falta
//!        una IOMMU, y es el paso N7
//!    [ ] y no se entera de nada si el kernel le miente sobre el marco. Es
//!        DISCIPLINA, no barrera: un fallo del propio kernel se lo salta
//! ```

#![no_std]

/// **UNA DIRECCION FISICA QUE YA PASO EL JUICIO.**
///
/// *** ESTE TIPO ES EL PASO N0. No tiene constructor publico: la unica forma de
/// obtener uno es [`juzgar`], y por eso *"que solo haya un sitio por donde
/// salga una direccion fisica"* deja de ser una regla que recordar y pasa a ser
/// una que el compilador comprueba.
///
/// [!] Y por eso [`Prenda::cruda`] existe pero es lo ultimo que se llama: el
/// `u64` de dentro es la direccion sin proteccion ninguna. Sacarla es salir del
/// embudo, y el sitio donde se hace tiene que ser el descriptor y ninguno mas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prenda(u64);

impl Prenda {
    /// La direccion cruda, para escribirla en el descriptor. **Ultimo paso.**
    pub fn cruda(&self) -> u64 {
        self.0
    }
}

/// Lo que el kernel SABE del marco, y se lo cuenta al juez.
///
/// ** `es_neutro` es un `bool` y no un `Duenno`: ver la cabecera. Aqui no hay
/// espejo que se pueda desincronizar.
#[derive(Clone, Copy, Debug)]
pub struct Marco {
    /// `phys::duenno_de(base) == Duenno::Neutro`, ya resuelto por quien sabe.
    ///
    /// ** Es el caso 1: **el aparato escribe en SU propio corral** -- la pagina
    /// de rebote, los anillos, el DCBAA.
    pub es_neutro: bool,
    /// **EL CASO 2: el kernel le PRESTO este marco para esta operacion.**
    ///
    /// *** Este campo nacio de un fallo mio, el 2026-09-09. La primera version
    /// del juez solo tenia `es_neutro`, y al ir a cablearlo en el AHCI aparecio
    /// esto (`dev/disk/transfer.rs:64`):
    ///
    /// ```text
    ///    let directo = tramo_dma(va, restante).and_then(|(phys, bytes)| ...
    /// ```
    ///
    /// El camino DIRECTO de una lectura le da al disco la direccion fisica del
    /// **bufer del que llamo**, que no es del aparato ni tiene por que serlo.
    /// Un juez con una sola regla **habria rechazado una lectura legitima y
    /// dejado el disco sin funcionar**.
    ///
    /// ** Y no lo encontre leyendo: lo encontro el intento de cablearlo. Por
    /// eso el paso N2 del plan existe aparte de N1.
    ///
    /// [!] HOY NADIE PUEDE RELLENAR ESTE CAMPO CON LA VERDAD, y eso es lo que
    /// bloquea N2. Saber que un marco esta prestado A UN APARATO Y AHORA es
    /// exactamente el **bit EN VUELO** del paso N4. Hasta que exista, quien
    /// llame a `juzgar` solo puede poner `None` -- y el camino directo del
    /// disco no puede pasar por aqui.
    ///
    /// *** O sea que **N-B no se puede cablear sin N-D**. Los dos ejes del
    /// plan --el DONDE y el CUANDO-- no eran independientes, y esto lo
    /// demuestra: la regla del DONDE necesita un dato del CUANDO para no
    /// rechazar lo correcto.
    pub en_vuelo_para: Option<u16>,
    /// Donde empieza el marco (o la arena) que se le dio a este aparato.
    pub base: u64,
    /// Cuanto mide. Un marco suelto son 4096; una arena, lo que se pidio.
    pub bytes: u64,
    /// De quien es. Al juez le da igual lo que signifique: solo lo compara.
    pub aparato: u16,
}

/// Lo que un driver quiere darle a su aparato.
#[derive(Clone, Copy, Debug)]
pub struct Peticion {
    /// La direccion fisica que se pretende escribir en el descriptor.
    pub fisica: u64,
    /// Cuantos bytes va a tocar el aparato desde ahi.
    pub bytes: u64,
    /// Quien va a escribir. Tiene que ser el mismo que el del marco.
    pub aparato: u16,
    /// A cuanto exige alinearse este aparato. `1` = le da igual.
    pub alineacion: u64,
    /// Cuantos bits tiene el campo de LONGITUD de su descriptor.
    ///
    /// ** No es el ancho de la direccion: es el de la CUENTA. El PRDT de AHCI
    /// guarda la direccion en 32+32 bits y la longitud en **22**, y ese es el
    /// campo que se desborda en silencio.
    pub bits_de_cuenta: u32,
}

/// Por que NO. Cada uno nombra una pregunta de la cabecera.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veto {
    /// El marco no es del aparato NI se lo prestaron: es memoria de otro.
    NoEsDeUnAparato,
    /// El marco es de un aparato, pero **de otro**.
    DeOtroAparato { suyo: u16, pide: u16 },
    /// `bytes == 0`. No es un no-op: es un descriptor sin sentido.
    NoPideNada,
    /// La direccion no esta dentro del marco, o `fisica + bytes` se sale.
    SeSaleDelMarco,
    /// No cumple la alineacion que el aparato exige.
    MalAlineada { pide: u64 },
    /// La cuenta no cabe en el campo de longitud del descriptor.
    NoCabeLaCuenta { bits: u32 },
}

/// **EL UNICO CONSTRUCTOR DE [`Prenda`].**
///
/// Puro: entra lo que se pide y lo que se sabe del marco, sale un permiso o un
/// motivo. No lee nada global, no escribe nada, y se prueba en el anfitrion en
/// microsegundos.
///
/// ** El orden de las preguntas no es casual: **de la mas barata de contestar a
/// la mas cara**, y de la que mas dano hace a la que menos. Un marco que no es
/// de un aparato se rechaza antes de mirar ninguna aritmetica.
pub fn juzgar(p: Peticion, m: Marco) -> Result<Prenda, Veto> {
    // 1 y 2. De quien es esto, y hay DOS formas legitimas de que sea suyo.
    //
    // ** El orden importa: se mira primero el prestamo porque es el caso
    // ESTRECHO --un aparato concreto, ahora-- y `es_neutro` es el ancho. Si un
    // marco esta prestado a otro, eso se dice con nombre en vez de dejarlo caer
    // en `NoEsDeUnAparato`, que mandaria a mirar el sitio equivocado.
    match m.en_vuelo_para {
        // Caso 2: prestado, y al que pide.
        Some(a) if a == p.aparato => {}
        // Prestado a OTRO. Es el fallo mas peligroso de los seis --dos
        // aparatos sobre el mismo bufer-- y por eso tiene su propio veto.
        Some(a) => return Err(Veto::DeOtroAparato { suyo: a, pide: p.aparato }),
        None => {
            // Caso 1: su propio corral.
            if !m.es_neutro {
                return Err(Veto::NoEsDeUnAparato);
            }
            if m.aparato != p.aparato {
                return Err(Veto::DeOtroAparato { suyo: m.aparato, pide: p.aparato });
            }
        }
    }
    // 3. Pide algo. Va antes que la aritmetica porque con `bytes == 0` la
    //    comprobacion de "cabe" da que SI, y colar un descriptor vacio es un
    //    fallo del aparato que nadie sabria explicar.
    if p.bytes == 0 {
        return Err(Veto::NoPideNada);
    }
    // 4. Cabe dentro del marco. Con `checked_add` porque el desbordamiento de
    //    `u64` haria que "cabe" saliera cierto por dar la vuelta.
    let fin_marco = match m.base.checked_add(m.bytes) {
        Some(v) => v,
        None => return Err(Veto::SeSaleDelMarco),
    };
    let fin_pide = match p.fisica.checked_add(p.bytes) {
        Some(v) => v,
        None => return Err(Veto::SeSaleDelMarco),
    };
    if p.fisica < m.base || fin_pide > fin_marco {
        return Err(Veto::SeSaleDelMarco);
    }
    // 5. Alineada como el aparato exija. `alineacion` de 0 o 1 = le da igual.
    if p.alineacion > 1 && p.fisica % p.alineacion != 0 {
        return Err(Veto::MalAlineada { pide: p.alineacion });
    }
    // 6. Y la CUENTA cabe en su campo. `bits_de_cuenta == 0` o >= 64 se lee
    //    como "sin limite": un aparato que no declara el suyo no se bloquea por
    //    una comprobacion que no puede contestar.
    if p.bits_de_cuenta > 0 && p.bits_de_cuenta < 64 {
        let techo = 1u64 << p.bits_de_cuenta;
        if p.bytes > techo {
            return Err(Veto::NoCabeLaCuenta { bits: p.bits_de_cuenta });
        }
    }
    Ok(Prenda(p.fisica))
}

#[cfg(test)]
mod pruebas;
