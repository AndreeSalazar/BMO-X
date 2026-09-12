//! **EL JUEZ DEL PRESTAMO** -- que paginas hay que mapear para prestar N bytes
//! que NO empiezan al principio de una pagina?
//!
//! generacion: nieto
//!
//! [cuesta]  MAQUINA -- por herencia, no por lo que hace. No toca hardware ni
//!           tiene un solo `unsafe`, pero `obj/loan.rs` mapea con su respuesta
//!           en el espacio del DIRECTOR. Instrumentar no contagia el coste;
//!           decidir si (L6e)
//!
//! # *** EL FALLO QUE LO TRAJO (2026-09-12)
//!
//! Tres arranques buscando por que DOOM y el cubo no salian en ventana, y la
//! respuesta cabia en una linea del DIRECTOR:
//!
//! ```text
//!    run c/cubo.bex
//!    [ventana] tid 6 ofrecio 518704 B: NO es BSUP
//! ```
//!
//! Las constantes coincidian en los dos lados --magia, formato, 32/16/8-- y el
//! tamano cabia. O sea que **el DIRECTOR no estaba leyendo la memoria que la app
//! escribio**. Y el motivo:
//!
//! ```text
//!    la app        s->base = malloc(bytes)     -> NO alineado a pagina: va
//!                                                 detras de un malloc(48) y
//!                                                 de una cabecera de 16
//!    loan::take    mapea PAGINAS desde `origen`, y `translate` devuelve el
//!                  MARCO, sin desplazamiento
//!    OP_BASE       devolvia `va`, el principio de la pagina
//! ```
//!
//! ** Nadie sumaba nunca `origen & 0xFFF`. El DIRECTOR leia la cabecera al
//! principio de la pagina y la app la habia escrito unos bytes mas adentro.
//! Magia distinta, y el veredicto: *"NO es BSUP"*.
//!
//! [!] Y NO FUE UNA REGRESION: `OP_BASE` nacio asi en `0e92c581`, y el
//! `malloc(48)` delante del de la imagen esta desde la primera version de la
//! superficie (`de3a74b9`). Ninguna hoja de metal registra una ventana vista:
//! la del 09-10 dice *"probado con ray.bex"* como afirmacion del plan.
//!
//! # ** Y EL SEGUNDO FALLO, QUE EL PRIMERO TAPABA
//!
//! Las paginas se contaban como `bytes.div_ceil(PAGINA)`, desde la pagina de
//! `origen`. Con desplazamiento, **el final del prestamo se quedaba sin
//! mapear**: hasta 4.095 bytes de la ultima fila. El dia que la cabecera se
//! leyera bien, componer la ultima fila habria sido un fallo de pagina **en el
//! compositor** -- justo lo que este diseno existe para impedir. Y soltar y
//! morir desmapeaban con la misma cuenta, o sea de menos.
//!
//! # POR QUE UN JUEZ APARTE Y NO TRES `div_ceil` ARREGLADOS
//!
//! Porque eran TRES copias de la misma cuenta --tomar, soltar, morir-- y las
//! tres estaban mal de la misma manera. Una cuenta que se escribe tres veces se
//! arregla dos. Y porque **aqui se puede PROBAR**: esto no avisa cuando se
//! equivoca, da una ventana en blanco o un fault tres arranques despues.
//!
//! La idea no es nuestra: Wayland la resolvio poniendo el desplazamiento en el
//! PROTOCOLO. `wl_shm_pool.create_buffer(id, offset, width, height, stride,
//! format)` lleva el `offset` como campo aparte, porque `mmap` trabaja en
//! paginas y un buffer dentro de un pool no empieza en una. Aqui el que presta
//! no lo manda: el kernel lo sabe --es `origen & (PAGINA - 1)`-- y lo guarda.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// La unidad minima de mapeo. No se puede mapear media pagina: la MMU no sabe.
///
/// [!] El kernel comprueba en compilacion que su `mm::PAGE` vale lo mismo. Dos
/// constantes que tienen que coincidir y nada que lo obligue es como una acaba
/// valiendo otra cosa.
pub const PAGINA: u64 = 4096;

/// **Lo que hay que mapear para prestar `bytes` desde `origen`.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tramo {
    /// El principio de la pagina donde empieza lo prestado. Desde aqui se
    /// traduce en el espacio del que presta.
    pub pagina: u64,
    /// **Cuanto hay que andar DENTRO de esa pagina** hasta el primer byte
    /// prestado. Es lo que se perdia.
    pub dentro: u64,
    /// Cuantos bytes de paginas enteras hay que mapear: `dentro + bytes`,
    /// redondeado arriba. Siempre multiplo de [`PAGINA`].
    pub mapeado: u64,
}

impl Tramo {
    /// Donde lee el que toma, dado donde quedo mapeada la primera pagina.
    ///
    /// Una funcion y no una suma en `loan.rs` para que el sitio donde se
    /// mapea y el sitio donde se contesta `OP_BASE` no puedan discrepar.
    pub fn base_para(&self, va: u64) -> u64 {
        va + self.dentro
    }

    /// Cabe en una ventana de prestamo de `ventana` bytes?
    ///
    /// ** Cuenta lo MAPEADO, no lo pedido: una superficie de exactamente una
    /// ventana que empiece 16 bytes dentro de su pagina necesita una pagina
    /// mas, y esa pagina caeria en la ventana del siguiente prestamo.
    pub fn cabe_en(&self, ventana: u64) -> bool {
        self.mapeado <= ventana
    }
}

/// **El tramo de paginas de `bytes` bytes que empiezan en `origen`.**
///
/// `None` en los dos casos que no tienen tramo, y ninguno se redondea a algo
/// que parezca valido:
///
/// ```text
///    bytes == 0              no hay nada que prestar. Mapear una pagina "por
///                            si acaso" seria prestar memoria que nadie ofrecio
///    origen + bytes desborda  el tramo daria la vuelta y saldria PEQUENO, que
///                            es el caso que tiene que parar
/// ```
pub fn tramo(origen: u64, bytes: u64) -> Option<Tramo> {
    if bytes == 0 {
        return None;
    }
    // El final tiene que existir en 64 bits antes de redondear nada.
    origen.checked_add(bytes)?;
    let dentro = origen & (PAGINA - 1);
    let pagina = origen - dentro;
    let crudo = dentro.checked_add(bytes)?;
    let mapeado = crudo.checked_add(PAGINA - 1)? & !(PAGINA - 1);
    // ** Y EL FINAL DE LA PAGINA TAMBIEN TIENE QUE EXISTIR, no solo el de lo
    // pedido. Lo cazo el banco el primer dia: `origen + bytes` cabia en 64 bits
    // y `pagina + mapeado` era 2^64 exacto, porque redondear arriba anade hasta
    // 4.095 bytes. Comprobar solo lo pedido era dejar pasar un tramo cuyo final
    // no se puede escribir.
    pagina.checked_add(mapeado)?;
    Some(Tramo { pagina, dentro, mapeado })
}

#[cfg(test)]
mod pruebas;
