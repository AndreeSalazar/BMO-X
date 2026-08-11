//! **La tarjeta de red: encontrarla y preguntarle quien es.** Nada mas.
//!
//! ## Por que este modulo no hace nada todavia, a proposito
//!
//! El paso siguiente --anillos de descriptores, DMA, tramas-- es el mas
//! delicado que hay: un anillo mal armado no da un fault, da **la tarjeta
//! escribiendo en memoria de otro**, y el sintoma tres arranques despues. Ya se
//! piso esa mina con el PRDT de AHCI (patron 4).
//!
//! Antes de eso hay tres preguntas que cuestan cuatro lecturas y que ninguna
//! teoria contesta:
//!
//! | | |
//! |---|---|
//! | la tarjeta del barrido, es la que dice el otro sistema? | la MAC |
//! | el BAR elegido, lleva a los registros? | que la MAC sea creible |
//! | hay cable? | `PHYstatus` |
//!
//! ** Y las tres se PREDIJERON antes de mirar. El Windows de esta misma maquina
//! dice `2C-F0-5D-D9-3C-E3`, enlace arriba a 100 Mbps. Si el arranque imprime
//! eso, las tres estan contestadas de una vez; si imprime otra cosa, **el numero
//! dice cual de las tres fallo**. Eso es lo que un "no funciona" nunca dice.
//!
//! Es el metodo de las cinco sondas del `#GP` de julio: predecir, leer,
//! comparar. Aqui ademas sale gratis, porque **no se escribe un solo byte en el
//! aparato**: lo unico que se le toca es el registro de comando de PCI, para
//! que el MMIO conteste.

use crate::ring0::dev::pci;
use crate::ring0::mm;

/// Realtek. El unico vendor cuyos registros sabemos leer hoy.
const VENDOR_REALTEK: u16 = 0x10EC;

/// Lo que se supo de la NIC en el arranque. `None` = no se busco o no habia.
static mut ID: Option<bmo_net::Identidad> = None;
static mut HAY: bool = false;
/// Donde vive, ya en direccion virtual. `0` = no se sabe leer esta tarjeta.
///
/// Se guarda para poder **volver a preguntar** sin repetir el barrido del PCI,
/// que son unas 65.000 lecturas de config. Y volver a preguntar es lo que
/// convierte el comando `net` en una prueba de verdad: ver la cabecera de
/// [`releer`].
static mut MMIO: *mut u8 = core::ptr::null_mut();
/// Donde estaba en el bus, para poder decirlo. `(vendor, device, bus, dev, func, bar)`.
static mut DONDE: (u16, u16, u8, u8, u8, u8) = (0, 0, 0, 0, 0, 0);

/// Busca la NIC, la identifica y lo **cuenta**. Se llama una vez al arrancar.
pub fn init() {
    let loc = match pci::find_net(0) {
        Some(l) => l,
        None => {
            // No es un fallo: esta maquina podria no tener NIC. Pero decirlo es
            // lo que evita buscar un bug en el driver cuando no hay tarjeta.
            crate::ring0::cabina::info("red", "no hay ninguna NIC Ethernet en el PCI", 0);
            return;
        }
    };
    unsafe {
        HAY = true;
        DONDE = (loc.vendor, loc.device, loc.bus, loc.dev, loc.func, loc.bar_index);
    }

    // Quien es, segun el PCI. Los dos numeros juntos: `10EC8168` se compara de
    // un vistazo con lo que dice cualquier otro sistema operativo.
    let vd = ((loc.vendor as u64) << 16) | loc.device as u64;
    crate::ring0::cabina::info("red", "NIC hallada: vendor:device", vd);
    crate::ring0::cabina::info("red", "y esta en bus:dev:func", bdf(&loc));

    // ** LOS SEIS BAR, DICHOS. Aunque todo vaya bien.
    //
    // Si la MAC sale mal, la primera pregunta va a ser "de que BAR la leiste", y
    // esa foto tiene que existir YA -- un diagnostico que solo se imprime cuando
    // algo falla obliga a reproducir el fallo para poder mirarlo.
    for i in 0..6 {
        if loc.bars[i] != 0 {
            crate::ring0::cabina::info("red", "BAR crudo", ((i as u64) << 32) | loc.bars[i] as u64);
        }
    }
    if loc.mmio == 0 {
        crate::ring0::cabina::fault("red", "la NIC no declara ni un BAR de memoria", 0);
        return;
    }
    crate::ring0::cabina::info("red", "el MMIO sale del BAR numero", loc.bar_index as u64);
    crate::ring0::cabina::info("red", "y esta en la direccion fisica", loc.mmio);

    // ** Y AQUI SE PARA SI NO ES REALTEK.
    //
    // Los offsets de abajo son del mapa de la familia 8169/8168. Leerlos en otra
    // tarjeta devolveria lo que hubiera ahi, y eso saldria como una MAC -- una
    // MAC inventada con la que despues se filtrarian tramas. Un "no se leerlo"
    // dicho vale mas que seis bytes adivinados (patron 26).
    if loc.vendor != VENDOR_REALTEK {
        crate::ring0::cabina::warn("red", "NIC de un vendor que no se leer todavia", loc.vendor as u64);
        return;
    }

    let mmio = mm::phys_to_virt(loc.mmio) as *mut u8;
    let id = unsafe { bmo_net::identificar(mmio) };
    unsafe {
        ID = Some(id);
        MMIO = mmio;
    }

    crate::ring0::cabina::info("red", "MAC", id.mac_u64());
    if !id.creible() {
        // Ceros o unos no dicen "tarjeta rota": dicen que la lectura no llego al
        // aparato. Es el BAR, no la NIC, y confundirlos manda a cambiar de
        // tarjeta cuando lo que hay que cambiar es un indice.
        crate::ring0::cabina::fault("red", "esa MAC no es creible: el BAR no lleva a los registros", id.mac_u64());
        return;
    }
    crate::ring0::cabina::info("red", "PHYstatus crudo", id.phy as u64);
    if id.enlace_arriba() {
        crate::ring0::cabina::info("red", "enlace ARRIBA, megabits", id.megabits() as u64);
    } else {
        // Sin cable no hay nada roto: hay que enchufarlo. Se dice para que no se
        // busque el fallo en el driver el dia que no lleguen tramas.
        crate::ring0::cabina::warn("red", "enlace ABAJO: no hay cable o el otro lado no contesta", id.phy as u64);
    }
}

/// `bus:dev:func` en un solo numero, para que quepa en un evento de CABINA.
fn bdf(loc: &pci::NetLoc) -> u64 {
    ((loc.bus as u64) << 16) | ((loc.dev as u64) << 8) | loc.func as u64
}

/// Hay NIC en la maquina?
pub fn hay() -> bool {
    unsafe { HAY }
}

/// Lo que se supo de ella EN EL ARRANQUE. `None` si no hay, o si no se sabe leer.
pub fn identidad() -> Option<bmo_net::Identidad> {
    unsafe { ID }
}

/// Donde estaba: `(vendor, device, bus, dev, func, bar)`.
pub fn donde() -> (u16, u16, u8, u8, u8, u8) {
    unsafe { DONDE }
}

/// **Vuelve a preguntarle al chip, AHORA.** `None` si no hay tarjeta legible.
///
/// === Por que esto no es `identidad()` con otro nombre ===
///
/// `identidad()` devuelve la foto del arranque. Esto va al aparato otra vez, y
/// esa diferencia es la que convierte el comando `net` en una prueba en vez de
/// un volcado:
///
/// > **Desenchufa el cable, escribe `net`, y el enlace tiene que caerse.**
///
/// Si el numero cambia, la lectura llega al silicio de verdad: el BAR es el
/// bueno, el mapeo esta vivo y `PHYstatus` es ese registro y no otro. Si NO
/// cambia, lo que se esta leyendo es una copia, una cache o el sitio
/// equivocado -- y eso hay que saberlo **antes** de montar un anillo de DMA
/// encima, no despues.
///
/// Una prueba que no puede fallar no prueba nada. Esta se puede tirar al suelo
/// con la mano, que es la mejor clase que hay.
pub fn releer() -> Option<bmo_net::Identidad> {
    let mmio = unsafe { MMIO };
    if mmio.is_null() {
        return None;
    }
    Some(unsafe { bmo_net::identificar(mmio) })
}
