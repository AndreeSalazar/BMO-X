//! **EL JUEZ DE LO QUE SE PUEDE CEDER** -- se puede dar este rango fisico a un
//! proceso de Ring 3?
//!
//! generacion: nieto
//!
//! El plan y el por que: `docs/plan/PLAN_SUELO_RING3.md`. El censo que lo pidio:
//! `docs/maestro/RING3_MAESTRO.md`.
//!
//! # *** POR QUE ESTO ES UN JUEZ APARTE Y NO UN `if` DENTRO DEL KERNEL
//!
//! Por lo mismo que `bmo-bex-gate` y `bmo-disco-juicio`: **aqui se puede
//! PROBAR**. Un veredicto que solo se puede comprobar arrancando la maquina no
//! es un veredicto comprobado, y este es de los que no avisan cuando se
//! equivocan -- ceder una pagina de mas no da un fault: da una ventana.
//!
//! Cero dependencias y cero `unsafe`, para que pueda vivir en Ring 0 y correr
//! bajo `cargo test` a la vez.
//!
//! # ** LA FRASE QUE OBLIGA A QUE ESTO EXISTA
//!
//! > Un proceso que puede decir *"mapeame la fisica 0x1000"* es un proceso que
//! > esta pidiendo ser el kernel.
//!
//! Con esa operacion, en tres pasos: mapea la fisica donde viven las tablas de
//! pagina, se pone el bit U/S y quita el NX donde quiera, y **los siete muros de
//! `docs/identidad/EL_AISLAMIENTO.md` se caen todos a la vez** -- no por un bug,
//! sino por la propia operacion funcionando como se pidio.
//!
//! Por eso el proceso NO nombra una direccion: nombra un aparato, y el kernel
//! saca la fisica de su propio censo. Este fichero es el ultimo filtro: aun
//! eligiendo el kernel, hay rangos que no se pueden ceder.
//!
//! # LA REGLA QUE ORDENA EL VEREDICTO
//!
//! Copiada de `bmo-disco-juicio`, y por el mismo motivo: **ninguna funcion de
//! aqui contesta que SI por defecto.** Cuando falta un dato, la respuesta es la
//! que no asume. Un juez de almacenamiento que se calla pierde datos; uno de
//! cesion que se calla regala la maquina.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// La unidad minima de cesion. No se puede mapear media pagina: la MMU no sabe.
pub const PAGINA: u64 = 4096;

/// El primer megabyte: BIOS, VGA, y la rampa donde aterrizan los nucleos al
/// despertar (`smp all`). No es de nadie y no se cede nunca.
pub const UN_MEGA: u64 = 0x10_0000;

/// LAPIC / IO-APIC / HPET. **La ventana que no se cede aunque sea MMIO.**
///
/// Es el mismo rango que `mm::phys::init` ya reserva, y esta aqui repetido a
/// proposito: aquel lo reserva para que el asignador no lo entregue como RAM;
/// esto lo niega para que no se entregue como APARATO. Dos preguntas distintas
/// sobre el mismo rango.
pub const APIC_BASE: u64 = 0xFEC0_0000;
/// Ver [`APIC_BASE`].
pub const APIC_BYTES: u64 = 0x140_0000;

/// Un tramo del mapa fisico, tal y como lo entrego el arranque.
///
/// [!] `es_ram` es **usable**, no "existe": la memoria reservada por el firmware
/// tampoco se cede, pero por otro motivo y con otro nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tramo {
    pub base: u64,
    pub bytes: u64,
    pub es_ram: bool,
}

/// Un rango que la casa se queda, con su nombre.
///
/// El nombre no es decoracion: es lo que convierte *"denegado"* en *"eso es la
/// pantalla, y la pantalla se pide por su propia puerta"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reserva {
    pub base: u64,
    pub bytes: u64,
    pub nombre: &'static str,
}

/// Por que NO se puede ceder este rango.
///
/// **Lleva los dos lados** siempre que puede -- lo que se pidio y con que
/// choca-- porque un `bool` frena la cesion sin decir como arreglarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veto {
    /// Cero bytes. No es un rango.
    Vacio,
    /// La base no empieza en una pagina.
    NoAlineado { base: u64 },
    /// El largo no es multiplo de pagina.
    LargoNoAlineado { bytes: u64 },
    /// `base + bytes` da la vuelta. Viene de fuera, asi que se comprueba.
    SeSaleDelEspacio { base: u64, bytes: u64 },
    /// *** Mas pequeno que una pagina.
    ///
    /// Un BAR de 256 bytes existe y es legitimo. Lo que no es legitimo es
    /// cederlo: la unidad de la MMU es la pagina, asi que ceder ese BAR cede
    /// **los 4.096 bytes que lo rodean**, y ahi pueden vivir los registros de
    /// otro aparato. Conceder eso seria conceder dos cosas y nombrar una.
    MasPequenoQueUnaPagina { bytes: u64 },
    /// El megabyte legacy.
    DebajoDeUnMega { base: u64 },
    /// **El veto que sostiene todos los demas.** El rango pisa memoria usable:
    /// cederlo da a Ring 3 una ventana a la RAM del kernel.
    PisaRam { base: u64, tramo_base: u64, tramo_bytes: u64 },
    /// Es el APIC. Ceder el APIC es ceder el control de las interrupciones, o
    /// sea ceder Ring 0 con otro nombre.
    EsElApic { base: u64 },
    /// Es un rango que la casa reparte por otra puerta.
    EsDeLaCasa { base: u64, nombre: &'static str },
}

/// Se solapan `[a, a+ab)` y `[b, b+bb)`?
///
/// Un rango de cero bytes no solapa con nada: no existe.
const fn solapan(a: u64, ab: u64, b: u64, bb: u64) -> bool {
    if ab == 0 || bb == 0 {
        return false;
    }
    // Sin sumas que puedan dar la vuelta: quien llama ya paso por
    // `SeSaleDelEspacio`, pero este helper tambien lo usan las reservas fijas.
    let (fin_a, of_a) = a.overflowing_add(ab);
    let (fin_b, of_b) = b.overflowing_add(bb);
    if of_a || of_b {
        // Un rango que da la vuelta se trata como que lo toca todo. Es la
        // respuesta que no asume.
        return true;
    }
    a < fin_b && b < fin_a
}

/// **Se puede ceder `[base, base+bytes)` a un proceso de Ring 3?**
///
/// `mapa` es el mapa fisico del arranque. `reservas` son los rangos que la casa
/// reparte por otra puerta (la pantalla, hoy).
///
/// # El APIC no se pasa: se comprueba siempre
///
/// Y es deliberado. Si el APIC viajara en `reservas`, olvidarlo seria posible --
/// y olvidarlo una vez es ceder el control de las interrupciones para siempre.
/// Lo que puede olvidarse, se olvida. Ver [`APIC_BASE`].
///
/// # El orden de los vetos importa
///
/// Primero lo que hace que el rango no sea un rango (vacio, alineacion,
/// desbordamiento), y solo despues contra que choca. Al reves, un `bytes = 0`
/// saldria como *"no pisa nada"*, que es un SI.
pub fn cedible(base: u64, bytes: u64, mapa: &[Tramo], reservas: &[Reserva]) -> Result<(), Veto> {
    if bytes == 0 {
        return Err(Veto::Vacio);
    }
    if base % PAGINA != 0 {
        return Err(Veto::NoAlineado { base });
    }
    // ** EL ORDEN DE ESTOS DOS NO ES ESTILO: DECIDE SI UNO DE ELLOS EXISTE.
    //
    // Al reves --primero el multiplo-- un BAR de 256 bytes sale como "el largo
    // no es multiplo de pagina", y `MasPequenoQueUnaPagina` **no se alcanza
    // nunca**: cualquier largo alineado y distinto de cero ya es >= PAGINA.
    // Seria un veto muerto, y un veto muerto es peor que no tenerlo: parece que
    // el caso esta cubierto.
    //
    // Y los dos mandan a sitios distintos. "No es multiplo" se arregla
    // redondeando. "Mas pequeno que una pagina" **no se arregla**: ceder ese BAR
    // cede los 4.096 bytes que lo rodean, y ahi puede vivir otro aparato.
    if bytes < PAGINA {
        return Err(Veto::MasPequenoQueUnaPagina { bytes });
    }
    if bytes % PAGINA != 0 {
        return Err(Veto::LargoNoAlineado { bytes });
    }
    if base.checked_add(bytes).is_none() {
        return Err(Veto::SeSaleDelEspacio { base, bytes });
    }
    if base < UN_MEGA {
        return Err(Veto::DebajoDeUnMega { base });
    }
    if solapan(base, bytes, APIC_BASE, APIC_BYTES) {
        return Err(Veto::EsElApic { base });
    }
    for t in mapa {
        if t.es_ram && solapan(base, bytes, t.base, t.bytes) {
            return Err(Veto::PisaRam { base, tramo_base: t.base, tramo_bytes: t.bytes });
        }
    }
    for r in reservas {
        if solapan(base, bytes, r.base, r.bytes) {
            return Err(Veto::EsDeLaCasa { base, nombre: r.nombre });
        }
    }
    Ok(())
}

impl Veto {
    /// Un nombre corto y estable para CABINA. **No lleva el numero**: el numero
    /// viaja en el `value` del evento, que es donde CABINA sabe pintarlo.
    ///
    /// [!] Y **corto** tiene un numero detras, no es una opinion: la fila de la
    /// bitacora son 80 columnas, el prefijo (`seq`, tick, severidad, `mmio:`)
    /// gasta 27, el ` =` dos, y una direccion de 16 digitos el resto. Quedan
    /// **35**. Una prueba de este crate lo exige, porque el 25-08 una frase de
    /// 48 columnas se comio el numero que era el motivo de la linea.
    pub const fn nombre(self) -> &'static str {
        match self {
            Veto::Vacio => "cero bytes no es un rango",
            Veto::NoAlineado { .. } => "la base no empieza en una pagina",
            Veto::LargoNoAlineado { .. } => "el largo no es multiplo de pagina",
            Veto::SeSaleDelEspacio { .. } => "base+largo se sale del espacio",
            Veto::MasPequenoQueUnaPagina { .. } => "mas pequeno que una pagina",
            Veto::DebajoDeUnMega { .. } => "el megabyte legacy no se cede",
            Veto::PisaRam { .. } => "PISA RAM: una ventana al kernel",
            Veto::EsElApic { .. } => "es el APIC: seria ceder las IRQ",
            Veto::EsDeLaCasa { .. } => "se reparte por otra puerta",
        }
    }

    /// La direccion que provoco el veto, para el `value` del evento.
    pub const fn donde(self) -> u64 {
        match self {
            Veto::Vacio => 0,
            Veto::NoAlineado { base } => base,
            Veto::LargoNoAlineado { bytes } => bytes,
            Veto::SeSaleDelEspacio { base, .. } => base,
            Veto::MasPequenoQueUnaPagina { bytes } => bytes,
            Veto::DebajoDeUnMega { base } => base,
            Veto::PisaRam { base, .. } => base,
            Veto::EsElApic { base } => base,
            Veto::EsDeLaCasa { base, .. } => base,
        }
    }
}

#[cfg(test)]
mod pruebas;
