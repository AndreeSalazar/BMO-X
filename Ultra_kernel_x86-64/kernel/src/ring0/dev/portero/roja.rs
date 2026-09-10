//! **EL PORTERO DURO: quien alcanza la RAM y NO tendria que poder.**
//!
//! [carril]  ROJO      **ESCRIBE en la configuracion de PCI**: retira el bit
//!                     de MAESTRO DEL BUS. Es el unico sitio de este kernel que
//!                     le quita a un aparato el permiso de llegar a la memoria
//!
//! [cuesta]  MAQUINA -- retirarselo a quien no debia no da un error: deja un
//!           aparato mudo. Si ese aparato es un PUENTE, deja mudo TODO LO QUE
//!           CUELGA DE EL, el disco del arranque incluido, y la maquina no
//!           llega a leer su propio sistema (L6e)
//!
//! [riesgo]  AJENO SILENCIO
//!           AJENO    -- **el bit que se lee no lo escribio este kernel**. El
//!                       firmware entrega la maquina con aparatos ya vivos, y
//!                       el modo SMM puede estar usando uno de ellos por
//!                       debajo. Un BME encendido que no reconocemos puede no
//!                       ser un olvido nuestro: puede ser la placa trabajando
//!           SILENCIO -- un BME retirado no da fault ni excepcion: el aparato
//!                       simplemente deja de contestar, y el sintoma aparece
//!                       en su driver, lejos de aqui (L6f)
//!
//! # *** QUE ES "HOT UNMAPPING" EN UNA MAQUINA SIN IOMMU
//!
//! Peticion del dueno, 2026-09-09: *"crear un portero duro con hot unmapping"*.
//!
//! Desmapear a un aparato es decirle a la IOMMU que esa pagina ya no es suya.
//! **Aqui no hay IOMMU encendida** (AMD-Vi es el paso N7), asi que la pregunta
//! honesta no es como desmapear: es **que queda cuando no se puede**.
//!
//! Queda un bit, y esta en el silicio desde 1992:
//!
//! ```text
//!    registro Command, offset 0x04, bit 2 -- BUS MASTER ENABLE
//!
//!    BME = 1   el aparato inicia transacciones: alcanza la RAM SOLO
//!    BME = 0   *** solo contesta cuando le preguntan
//! ```
//!
//! ** No es un desmapeo por pagina. Es el interruptor GENERAL: en vez de
//! quitarle una direccion, se le quita **la capacidad de emitir**. Mas basto y
//! mas fuerte, y es lo unico que hay hasta N7.
//!
//! Y es CALIENTE de verdad, que es la otra mitad de lo que se pidio:
//!
//! ```text
//!    no reinicia nada           el aparato sigue enchufado y alimentado
//!    no desmonta un driver      no hay driver: son los que nadie adopto
//!    no invalida un TLB         no hay traduccion que invalidar
//!    UNA escritura de 32 bits   y se deshace con otra
//! ```
//!
//! *** Ese es el argumento entero: **un desmapeo que cuesta una escritura y se
//! deshace con otra se puede permitir el lujo de equivocarse**. El de la IOMMU
//! no -- por eso N7 llega despues y no antes.
//!
//! # [!] LAS TRES COSAS QUE ESTE FICHERO NO CIERRA JAMAS
//!
//! Se pidio *"que no me juegue en contra"*, y este carril es el que mas podia.
//! Las exclusiones no son prudencia: son las tres formas de matar el arranque.
//!
//! ```text
//!    1. LOS PUENTES (clase 0x06)
//!       El bit de un puente no gobierna al puente: gobierna **si deja pasar
//!       hacia arriba lo que escriben los de abajo**. Cerrarselo al puerto raiz
//!       donde cuelga la grafica no calla a la grafica: calla la rama entera.
//!       Y el disco del arranque cuelga de otra igual
//!
//!    2. LO QUE ESTE KERNEL ENCENDIO
//!       `pci::enable_mem_bus_master` es el UNICO sitio que pone ese bit en
//!       todo BMO-X, y desde hoy apunta a quien se lo pone. Un aparato
//!       adoptado no se cierra ni por error, porque la lista no se escribe a
//!       mano: la escribe el propio acto de adoptarlo
//!
//!    3. NADA, MIENTRAS EL CERROJO ESTE EN `Mirar`
//!       Que es como sale de fabrica. Ver abajo
//! ```
//!
//! ** La 2 es la que hace que esto no envejezca. Una lista de aparatos escrita
//! a mano se queda corta el dia que se adopte el cuarto --y ese dia el portero
//! duro cerraria la GPU recien estrenada--. Una lista que se rellena sola en el
//! acto de encender el bit no puede quedarse corta: **es la misma linea**.
//!
//! # EL CERROJO EMPIEZA EN `Mirar`, Y ESO ES L3
//!
//! ```text
//!    Mirar    dice quien SERIA cerrado, con sus papeles enteros. Cero
//!             escrituras. *** ES LO QUE HAY PUESTO
//!    Cerrar   retira el bit de verdad, y comprueba que se quedo retirado
//! ```
//!
//! *** Lo que se sacrifica al empezar en `Mirar` es que **hoy esto no protege
//! de nada**. Se acepta, y el motivo es el mismo que el de las ocho reglas del
//! DMA (`NEUTRO/DMA/REGLAS.txt`): este codigo corre en el arranque, y una regla
//! mal afinada en el arranque no da un aviso -- deja la maquina sin poder leer
//! su propio sistema, y el arreglo es un flasheo a ciegas.
//!
//! ** El dia que un arranque diga que aparatos ajenos hay y el dueno los
//! reconozca uno a uno, pasar a `Cerrar` es **cambiar una palabra**. Al reves
//! --cerrar hoy y relajar despues-- no tiene vuelta atras por software.
//!
//! > Una barrera que se enciende antes de saber a quien para no es una barrera:
//! > es una averia con buenas intenciones.
//!
//! Paso N3 de `docs/plan/PLAN_EL_NEUTRO_VIGILADO.md`, y la mitad que faltaba de
//! R5b: el censo contra la maquina ya sabia CONTAR la diferencia. Esto sabe
//! hacer algo con ella.

use super::super::pci::{cfg_read32, cfg_write32};

/// El bit 2 del registro Command (offset 0x04): **Bus Master Enable**.
const BME: u32 = 0b100;
/// Offset del registro Command en la configuracion de PCI.
const COMMAND: u8 = 0x04;
/// Clase base de un PUENTE. La primera exclusion, y la que mata el arranque.
const CLASE_PUENTE: u8 = 0x06;

/// Cuantos aparatos adoptados caben. Hoy son TRES (`NEUTRO/CENSO.txt`) y la
/// grafica sera el cuarto; ocho deja sitio sin inventar una maquina que no
/// existe (LEY 24).
const ADOPTADOS_MAX: usize = 8;

/// Los `bus:dev.func` a los que ESTE kernel les encendio el bit de maestro.
///
/// [!] No es una lista de configuracion: **es un registro de lo que se hizo**.
/// La rellena `pci::enable_mem_bus_master`, que es el unico sitio de BMO-X que
/// enciende ese bit. Por eso no se puede quedar corta.
static mut ADOPTADOS: [u16; ADOPTADOS_MAX] = [0; ADOPTADOS_MAX];
/// Cuantas casillas de `ADOPTADOS` valen. Hace falta porque `0` es un BDF
/// legitimo: `0:0.0` es el puente raiz.
static mut CUANTOS: usize = 0;

/// Maestros del bus que nadie adopto, en el ultimo recorrido.
static mut AJENOS: u32 = 0;
/// De esos, a cuantos se les retiro el bit **y se comprobo que se quedo**.
static mut CERRADOS: u32 = 0;
/// Puentes con el bit puesto que NO se tocan nunca. Se cuentan para que la
/// diferencia entre "no habia" y "no se tocaron" no sea invisible.
static mut PUENTES: u32 = 0;

/// `bus:dev.func` en un numero, que es como viaja por CABINA.
fn bdf(bus: u8, dev: u8, func: u8) -> u16 {
    ((bus as u16) << 8) | ((dev as u16) << 3) | (func as u16)
}

/// **APUNTAR QUE ESTE KERNEL LE ENCENDIO EL BIT DE MAESTRO A ALGUIEN.**
///
/// La llama `pci::enable_mem_bus_master` y no la llama nadie mas. Si algun dia
/// aparece un segundo sitio que encienda BME, ese sitio tiene que llamar aqui
/// -- y si no lo hace, el portero duro cerrara su aparato: **el fallo se paga
/// donde se cometio**, que es lo unico que hace mantenible una lista asi.
pub fn adoptado(bus: u8, dev: u8, func: u8) {
    unsafe {
        if CUANTOS >= ADOPTADOS_MAX {
            // ** No se pisa la casilla 0 ni se hace sitio. Un registro que
            // olvida al primero para meter al noveno convierte a un aparato
            // adoptado en un ajeno, y este carril lo cerraria.
            crate::ring0::cabina::warn(
                "portero",
                "mas maestros adoptados de los que caben: el portero duro se DESARMA",
                bdf(bus, dev, func) as u64,
            );
            return;
        }
        ADOPTADOS[CUANTOS] = bdf(bus, dev, func);
        CUANTOS += 1;
    }
}

fn es_adoptado(quien: u16) -> bool {
    unsafe {
        // ** Si el registro se desbordo alguna vez, `CUANTOS` se quedo en el
        // tope y hay adoptados que no estan. Ver el aviso de `adoptado`.
        CUANTOS < ADOPTADOS_MAX && ADOPTADOS[..CUANTOS].contains(&quien)
    }
}

/// Que hace el portero duro con un maestro que nadie adopto.
///
/// [!] `Cerrar` no se construye en ningun sitio, y **eso es el diseno**: es la
/// palabra que el dueno cambia arriba el dia que reconozca la lista. Un
/// `allow` porque el compilador no tiene forma de saber eso.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cerrojo {
    /// Dice quien seria cerrado. **Cero escrituras.**
    Mirar,
    /// Retira el bit de verdad, y comprueba que se quedo retirado.
    Cerrar,
}

/// *** EL CERROJO. Sale de fabrica en `Mirar`, y el motivo esta en la cabecera.
pub const EL_CERROJO: Cerrojo = Cerrojo::Mirar;

/// `(ajenos, cerrados, puentes intocables)` del ultimo recorrido.
pub fn ajenos() -> (u32, u32, u32) {
    unsafe { (AJENOS, CERRADOS, PUENTES) }
}

/// **RECORRE EL BUS Y SE OCUPA DE QUIEN ALCANZA LA RAM SIN QUE NADIE LO
/// ADOPTARA.** Se llama en el arranque, DESPUES del censo.
///
/// El orden importa por lo de siempre: los tres `find_*` tienen que haber
/// corrido, porque son ellos los que llaman a `adoptado`. Correr esto antes
/// dejaria el registro vacio y **todos los maestros serian ajenos**, incluido
/// el disco del que se arranca.
pub fn duro() {
    let mut ajenos = 0u32;
    let mut cerrados = 0u32;
    let mut puentes = 0u32;
    let mut tercos = 0u32;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            if cfg_read32(bus, dev, 0, 0x00) == 0xFFFF_FFFF {
                continue;
            }
            let header0 = (cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0xFF;
            let max_func = if header0 & 0x80 != 0 { 8 } else { 1 };
            for func in 0u8..max_func {
                let vd = cfg_read32(bus, dev, func, 0x00);
                if vd == 0xFFFF_FFFF {
                    continue;
                }
                let cmd = cfg_read32(bus, dev, func, COMMAND);
                // Sin el bit no alcanza la RAM solo: no es asunto de este carril.
                if cmd & BME == 0 {
                    continue;
                }
                let quien = bdf(bus, dev, func);
                if es_adoptado(quien) {
                    continue;
                }
                // *** EXCLUSION 1: UN PUENTE NO SE CIERRA NUNCA. Ver la
                // cabecera. Se cuenta para que "no habia" y "no se toco"
                // no se lean igual.
                if ((cfg_read32(bus, dev, func, 0x08) >> 24) as u8) == CLASE_PUENTE {
                    puentes += 1;
                    continue;
                }
                ajenos += 1;
                let papeles = ((vd & 0xFFFF) as u64) << 48
                    | ((vd >> 16) as u64) << 32
                    | (quien as u64);
                match EL_CERROJO {
                    Cerrojo::Mirar => crate::ring0::cabina::warn(
                        "portero",
                        "MAESTRO AJENO: alcanza la RAM y nadie lo adopto (cerrojo en MIRAR)",
                        papeles,
                    ),
                    Cerrojo::Cerrar => {
                        cfg_write32(bus, dev, func, COMMAND, cmd & !BME);
                        // ** SE VUELVE A LEER. Un registro de configuracion no
                        // esta obligado a aceptar lo que se le escribe, y un
                        // cierre que se da por hecho es peor que no cerrar:
                        // deja creyendo que un aparato esta callado.
                        if cfg_read32(bus, dev, func, COMMAND) & BME == 0 {
                            cerrados += 1;
                            crate::ring0::cabina::warn(
                                "portero",
                                "BME RETIRADO a un maestro que nadie adopto",
                                papeles,
                            );
                        } else {
                            tercos += 1;
                            crate::ring0::cabina::warn(
                                "portero",
                                "se le pidio soltar BME y lo MANTIENE: sigue alcanzando la RAM",
                                papeles,
                            );
                        }
                    }
                }
            }
        }
    }
    unsafe {
        AJENOS = ajenos;
        CERRADOS = cerrados;
        PUENTES = puentes;
    }
    if ajenos == 0 {
        crate::ring0::cabina::count(
            "portero",
            "maestros del bus, y a todos los adopto este kernel",
            unsafe { CUANTOS as u64 },
        );
    } else if EL_CERROJO == Cerrojo::Mirar {
        crate::ring0::cabina::warn(
            "portero",
            "maestros ajenos VISTOS y NO cerrados: el cerrojo esta en MIRAR",
            ajenos as u64,
        );
    } else {
        crate::ring0::cabina::warn("portero", "maestros ajenos CERRADOS", cerrados as u64);
    }
    if tercos != 0 {
        crate::ring0::cabina::warn("portero", "maestros que NO soltaron BME", tercos as u64);
    }
    if puentes != 0 {
        crate::ring0::cabina::count(
            "portero",
            "puentes con BME: intocables a proposito (cerrarlos calla la rama)",
            puentes as u64,
        );
    }
}
