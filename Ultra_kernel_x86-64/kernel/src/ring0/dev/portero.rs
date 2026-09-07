//! **EL PORTERO DEL BUS: que hay enchufado a esta placa, y para que hay codigo.**
//!
//! [carril]  VERDE     lee configuracion de PCI y cuenta. No escribe ni un bit
//!
//! [cuesta]  NADA -- un recorrido de configuracion en el arranque, una vez. No
//!           habilita nada, no reclama ningun aparato y no cambia que se
//!           adopta: `find_ahci`, `find_nic` y `find_xhci` siguen decidiendo.
//!
//! [riesgo]  SILENCIO -- si esto se calla, se vuelve al estado de antes: la
//!           maquina tiene una tarjeta grafica dentro y no lo menciona jamas.
//!           No rompe nada; deja sin respuesta *"que tengo, y que se ignora"*.
//!
//! # *** LA SEGUNDA PUERTA DEL PORTERO
//!
//! Hermano de `dev/usb/portero.rs`, y con la misma frase del dueno detras:
//!
//! > *"es como un guardian con que busca nombres y papeles, y si no sale le
//! > avisa al kernel y ya"*
//!
//! Aquel mira **un puerto USB** cuando llega algo. Este mira **el bus entero**
//! al arrancar. Son dos ficheros y no uno porque contestan dos preguntas
//! distintas (L6b):
//!
//! ```text
//!    dev/usb/portero.rs   que LLEGO a un puerto, y que se le contesto
//!    ESTE                 que HAY en la placa, y para que hay codigo
//! ```
//!
//! # Por que hacia falta, dicho con el caso que lo pidio
//!
//! BMO-X recorre el PCI **tres veces**, y las tres buscando algo concreto:
//!
//! ```text
//!    find_ahci   clase 0x01   almacenamiento
//!    find_nic    clase 0x02   Ethernet
//!    find_xhci   clase 0x0C   USB
//! ```
//!
//! ** Cada recorrido se para en cuanto encuentra lo suyo y **lo demas no se
//! mira**. Asi que una tarjeta grafica, una de sonido o un Wi-Fi pueden estar
//! ahi dentro y BMO-X no los nombra ni una vez -- ni para decir que los ignora.
//!
//! El dueno lo topo con la pregunta de la GPU: tiene una RTX 3060 en la maquina
//! y **el sistema no dice que existe**. Antes de escribir una linea de driver,
//! lo primero es que la maquina sepa decir lo que tiene delante.
//!
//! > Un aparato que el sistema no nombra ni para descartarlo es indistinguible
//! > de un aparato que no esta puesto.
//!
//! # Lo que NO hace, y es la mitad del diseno
//!
//! ```text
//!    [ ] no habilita nada          ni MEM, ni Bus Master, ni un BAR
//!    [ ] no reclama ningun aparato los tres `find_*` siguen mandando
//!    [ ] no decide nada            solo dice si HAY codigo o no lo hay
//! ```
//!
//! [!] Es el mismo reparto que el portero del USB: **apuntar no puede cambiar
//! la decision**. Un censo que ademas habilitara aparatos seria una cuarta
//! politica de adopcion viviendo al lado de las tres, y el dia que discreparan
//! ganaria la que corriera antes.
//!
//! # Y por que no se dicen los sesenta
//!
//! Una placa moderna tiene entre treinta y sesenta funciones de PCI, casi todas
//! puentes y raices. CABINA guarda 82 eventos: soltar sesenta renglones en el
//! arranque **vaciaria el anillo antes de que el escritorio exista**, y con el
//! se irian las lineas que explican por que algo fallo.
//!
//! ** Asi que se CUENTAN todas y se DICEN las que este sistema podria querer.
//! Un puente PCI no es una noticia; una tarjeta grafica sin driver si.

use super::pci::cfg_read32;

/// Las clases que se anuncian. El resto se cuenta y se calla.
///
/// No es una lista de lo que BMO-X soporta --soporta tres-- sino de **lo que
/// tendria sentido soportar**. Un aparato de una de estas clases sin codigo
/// detras es una decision pendiente; un puente no lo es.
const INTERESANTES: [(u8, &str); 6] = [
    (0x01, "almacenamiento"),
    (0x02, "red"),
    (0x03, "video"),
    (0x04, "multimedia"),
    (0x0C, "bus serie (USB y companyia)"),
    (0x0D, "radio (Wi-Fi, Bluetooth)"),
];

/// Los fabricantes que se saben nombrar.
///
/// Un numero de fabricante que hay que ir a buscar a una web es un numero que
/// no se busca con la maquina delante. Son los cuatro que pueden aparecer en
/// esta placa; el resto sale por su numero, que sigue siendo la verdad.
const FABRICANTES: [(u16, &str); 6] = [
    (0x1002, "AMD/ATI"),
    (0x1022, "AMD"),
    (0x10DE, "NVIDIA"),
    (0x8086, "Intel"),
    (0x1969, "Qualcomm/Atheros"),
    (0x144D, "Samsung"),
];

/// Cuantas funciones de PCI hay, en total.
static mut FUNCIONES: u32 = 0;
/// De esas, cuantas son de una clase que este sistema podria querer.
static mut INTERESA: u32 = 0;
/// Y de esas, cuantas se quedan SIN CODIGO.
static mut SIN_CODIGO: u32 = 0;

/// `(funciones en el bus, de clase interesante, sin codigo)`.
pub fn stats() -> (u32, u32, u32) {
    unsafe { (FUNCIONES, INTERESA, SIN_CODIGO) }
}

/// **Recorre el bus una vez y dice que hay.** Se llama en el arranque, DESPUES
/// de los tres `find_*`, para que "hay codigo" sea la verdad y no una promesa.
///
/// [!] El orden importa y por eso se dice: llamarlo antes daria "sin codigo"
/// para el AHCI que se va a reclamar tres lineas mas abajo, que es exactamente
/// la clase de dato falso que este fichero existe para no producir.
pub fn censar() {
    let mut funciones = 0u32;
    let mut interesa = 0u32;
    let mut sin_codigo = 0u32;
    for bus in 0u16..=255 {
        let bus = bus as u8;
        for dev in 0u8..32 {
            let vd0 = cfg_read32(bus, dev, 0, 0x00);
            if vd0 == 0xFFFF_FFFF {
                continue;
            }
            // Bit 7 de Header Type: el aparato tiene varias funciones.
            let header0 = (cfg_read32(bus, dev, 0, 0x0C) >> 16) & 0xFF;
            let max_func = if header0 & 0x80 != 0 { 8 } else { 1 };
            for func in 0u8..max_func {
                let vd = cfg_read32(bus, dev, func, 0x00);
                if vd == 0xFFFF_FFFF {
                    continue;
                }
                funciones += 1;
                let vendor = (vd & 0xFFFF) as u16;
                let device = (vd >> 16) as u16;
                let clase = cfg_read32(bus, dev, func, 0x08);
                let base = (clase >> 24) as u8;
                let sub = (clase >> 16) as u8;
                let prog = (clase >> 8) as u8;
                let Some(que) = interesante(base) else { continue };
                interesa += 1;
                if !hay_codigo(base, sub, prog) {
                    sin_codigo += 1;
                }
                anuncia(que, hay_codigo(base, sub, prog), vendor, device, bus, dev, func, base, sub);
            }
        }
    }
    unsafe {
        FUNCIONES = funciones;
        INTERESA = interesa;
        SIN_CODIGO = sin_codigo;
    }
    crate::ring0::cabina::count("portero", "funciones de PCI en la placa", funciones as u64);
    if sin_codigo != 0 {
        crate::ring0::cabina::warn(
            "portero",
            "aparatos de una clase que BMO-X podria querer y NO tienen codigo",
            sin_codigo as u64,
        );
    }
}

fn interesante(base: u8) -> Option<&'static str> {
    INTERESANTES.iter().find(|(c, _)| *c == base).map(|(_, q)| *q)
}

/// **Hay codigo para esto en BMO-X?** La pregunta entera del dueno, en una
/// funcion.
///
/// [!] Y contesta por lo que los tres `find_*` buscan DE VERDAD, no por la
/// clase a secas. Un Wi-Fi es clase 0x0D y `find_nic` solo mira 0x02/0x00 -- si
/// aqui se dijera "hay codigo" por ser de red, el censo mentiria en el unico
/// caso donde su respuesta importa.
fn hay_codigo(base: u8, sub: u8, prog: u8) -> bool {
    match base {
        // Almacenamiento: AHCI, NVMe, IDE y RAID. Ver `TipoAlmacen`.
        0x01 => matches!(sub, 0x01 | 0x04 | 0x06 | 0x08),
        // Red: SOLO Ethernet. Un Wi-Fi (0x80) se parece en la clase y en nada mas.
        0x02 => sub == 0x00,
        // USB: SOLO xHCI. Un EHCI o un OHCI no los toca nadie aqui.
        0x0C => sub == 0x03 && prog == 0x30,
        _ => false,
    }
}

/// Un renglon por aparato, con sus papeles enteros dentro del numero.
#[allow(clippy::too_many_arguments)]
fn anuncia(
    que: &'static str,
    codigo: bool,
    vendor: u16,
    device: u16,
    bus: u8,
    dev: u8,
    func: u8,
    base: u8,
    sub: u8,
) {
    // Igual que el portero del USB: el nombre arriba, para que se lea de
    // izquierda a derecha en el hexadecimal que pinta CABINA.
    //
    //    vid(16) | did(16) | bus | dev:func | clase | subclase
    let papeles = ((vendor as u64) << 48)
        | ((device as u64) << 32)
        | ((bus as u64) << 24)
        | (((dev as u64) << 3 | func as u64) << 16)
        | ((base as u64) << 8)
        | sub as u64;
    if codigo {
        crate::ring0::cabina::info("portero", que, papeles);
    } else {
        // ** El fabricante va en el TEXTO y no solo en el numero. Es lo unico
        // que convierte "hay algo de video sin driver" en una frase con la que
        // se puede ir a buscar documentacion.
        crate::ring0::cabina::warn("portero", sin_codigo_dice(que, vendor), papeles);
    }
}

/// El renglon de un aparato sin codigo, con el fabricante dentro.
///
/// Se devuelve una frase entera y no se compone: CABINA guarda `&'static str`,
/// asi que un texto armado en RAM no sobrevive a la vuelta. Son doce
/// combinaciones y caben escritas.
fn sin_codigo_dice(que: &'static str, vendor: u16) -> &'static str {
    let fab = FABRICANTES.iter().find(|(v, _)| *v == vendor).map(|(_, n)| *n);
    match (que, fab) {
        ("video", Some("NVIDIA")) => "hay una GRAFICA NVIDIA y BMO-X no tiene codigo para ella",
        ("video", Some("AMD/ATI")) => "hay una GRAFICA AMD y BMO-X no tiene codigo para ella",
        ("video", Some("Intel")) => "hay una GRAFICA Intel y BMO-X no tiene codigo para ella",
        ("video", _) => "hay una GRAFICA y BMO-X no tiene codigo para ella",
        ("red", _) => "hay algo de RED que no es Ethernet: sin codigo",
        ("radio (Wi-Fi, Bluetooth)", _) => "hay una RADIO (Wi-Fi o Bluetooth): sin codigo",
        ("multimedia", _) => "hay un aparato de SONIDO por PCI: sin codigo (el audio va por USB)",
        ("almacenamiento", _) => "hay un ALMACENAMIENTO de un tipo que no se maneja",
        ("bus serie (USB y companyia)", _) => "hay un controlador USB que NO es xHCI: sin codigo",
        _ => "hay un aparato de una clase que interesa y no tiene codigo",
    }
}
