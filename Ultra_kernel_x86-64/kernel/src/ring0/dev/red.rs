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

// == STEP 1: RECEIVE. NOTHING IS TRANSMITTED. =================================
//
// # Why this is behind a typed command and not in the boot path
//
// Same reason as `smp`: this is the first code that makes a device write into
// this machine's memory by itself. If the ring is wrong, what hangs is the
// command and not the machine at power-on, and the way out is the reset button
// instead of a disk that no longer boots.
//
// The bit layout, the sizes, the ownership rule and the frame length are all
// decided in `bmo-net` and tested on the host. What is left here is the part no
// test can cover: allocating the memory, telling the card where it is, and
// waiting.
//
// # The physical address is SUBTRACTED, not asked for
//
// `alloc_frames_contig` hands back a PHYSICAL base, and the physmap is a linear
// mirror. So the address the card needs is the one the allocator already
// returned, and the address the CPU uses is that plus the mirror base. Nothing is
// looked up in a page table. That is the lesson of Ep. 39 applied before the fact
// rather than after: asking allows being answered wrong.

/// Ring and buffers, once claimed. `0` = not started.
static mut RX_RING_PHYS: u64 = 0;
static mut RX_BUFS_PHYS: u64 = 0;
/// Which descriptor is next to be looked at. The card walks the ring in order and
/// so do we.
static mut RX_NEXT: usize = 0;
/// Frames seen since the ring started. The number that answers the question.
static mut RX_FRAMES: u64 = 0;

/// Is the receiver armed?
pub fn rx_activo() -> bool {
    unsafe { RX_RING_PHYS != 0 }
}

/// Frames received since [`rx_start`].
pub fn rx_tramas() -> u64 {
    unsafe { RX_FRAMES }
}

unsafe fn w8(mmio: *mut u8, off: usize, v: u8) {
    core::ptr::write_volatile(mmio.add(off), v);
}
unsafe fn r8(mmio: *mut u8, off: usize) -> u8 {
    core::ptr::read_volatile(mmio.add(off))
}
unsafe fn w16(mmio: *mut u8, off: usize, v: u16) {
    core::ptr::write_volatile(mmio.add(off) as *mut u16, v);
}
unsafe fn w32(mmio: *mut u8, off: usize, v: u32) {
    core::ptr::write_volatile(mmio.add(off) as *mut u32, v);
}

/// A pointer to descriptor `i` of the ring, in virtual space.
unsafe fn desc(i: usize) -> *mut bmo_net::RxDesc {
    let virt = mm::phys_to_virt(RX_RING_PHYS) as *mut bmo_net::RxDesc;
    virt.add(i)
}

/// **Arms the receiver.** Returns `false` and says why if it cannot.
///
/// Nothing is transmitted, here or anywhere else in step 1: `CR.TE` is left
/// alone on purpose, so a mistake in this code cannot put a single byte on the
/// wire and cannot disturb anyone else on the network.
pub fn rx_start() -> bool {
    let mmio = unsafe { MMIO };
    if mmio.is_null() {
        crate::ring0::cabina::warn("red", "no hay NIC legible: el receptor no arranca", 0);
        return false;
    }
    if rx_activo() {
        crate::ring0::cabina::info("red", "el receptor ya estaba armado, tramas", rx_tramas());
        return true;
    }

    // One page for the ring (16 x 16 = 256 bytes, and a page is 256-aligned with
    // room to spare -- the card REQUIRES 256-byte alignment for RDSAR).
    let ring = match crate::ring0::mm::phys::alloc_frames_contig(1) {
        Some(p) => p,
        None => {
            crate::ring0::cabina::fault("red", "sin marco para el anillo de recepcion", 0);
            return false;
        }
    };
    // And the buffers: 16 x 2048 = 32 KiB = 8 pages, contiguous so that buffer
    // `i` is simply base + i * 2048.
    let bytes = (bmo_net::RX_RING_LEN * bmo_net::RX_BUF_LEN as usize) as u64;
    let pages = (bytes + mm::PAGE - 1) / mm::PAGE;
    let bufs = match crate::ring0::mm::phys::alloc_frames_contig(pages) {
        Some(p) => p,
        None => {
            crate::ring0::cabina::fault("red", "sin marcos para los bufers de recepcion", pages as u64);
            return false;
        }
    };

    unsafe {
        RX_RING_PHYS = ring;
        RX_BUFS_PHYS = bufs;
        RX_NEXT = 0;
        RX_FRAMES = 0;

        // The ring, handed over descriptor by descriptor. The LAST one carries
        // End Of Ring -- without it the card walks off the end and writes over
        // whatever follows. That bit is the difference between a bug and
        // corruption, so it is built by `to_card` and not by hand here.
        for i in 0..bmo_net::RX_RING_LEN {
            let buf = bufs + (i * bmo_net::RX_BUF_LEN as usize) as u64;
            let last = i == bmo_net::RX_RING_LEN - 1;
            let d = bmo_net::RxDesc::to_card(buf, bmo_net::RX_BUF_LEN, last);
            core::ptr::write_volatile(desc(i), d);
        }

        // -- The card, in the order the family wants it --------------------
        use bmo_net::{cr, reg_rx};

        // 1. Soft reset. The chip clears the bit ITSELF when it is done: this is
        //    a handshake and not a delay, and a fixed wait is how a driver works
        //    on one machine and not on the next. Bounded so a dead card cannot
        //    hang the command forever.
        w8(mmio, reg_rx::CR, cr::RST);
        let mut spins = 0u32;
        while r8(mmio, reg_rx::CR) & cr::RST != 0 {
            spins += 1;
            if spins > 1_000_000 {
                crate::ring0::cabina::fault("red", "la NIC no termina su reset", spins as u64);
                RX_RING_PHYS = 0;
                return false;
            }
            core::hint::spin_loop();
        }
        crate::ring0::cabina::info("red", "reset completado, vueltas", spins as u64);

        // 2. Unlock the config registers, and lock them again at the end. Leaving
        //    them unlocked is how a stray write later becomes a card that forgot
        //    its own MAC.
        w8(mmio, reg_rx::CFG9346, 0xC0);

        // 3. No interrupts: this driver POLLS. Said out loud because an unmasked
        //    interrupt with no handler installed is a triple fault, not a bug.
        w16(mmio, reg_rx::IMR, 0);
        w16(mmio, reg_rx::ISR, 0xFFFF);

        // 4. Where the ring is, and how big a frame we accept.
        w16(mmio, reg_rx::RMS, bmo_net::RX_BUF_LEN);
        w32(mmio, reg_rx::RDSAR_LO, (ring & 0xFFFF_FFFF) as u32);
        w32(mmio, reg_rx::RDSAR_HI, (ring >> 32) as u32);

        // 5. What gets accepted. Broadcast is the one that matters: it is what
        //    makes a plugged cable produce traffic with nobody doing anything.
        w32(mmio, reg_rx::RCR, bmo_net::rx_config());

        w8(mmio, reg_rx::CFG9346, 0x00);

        // 6. And only now, the receiver. TE stays OFF.
        let c = r8(mmio, reg_rx::CR);
        w8(mmio, reg_rx::CR, (c & !cr::TE) | cr::RE);
    }

    crate::ring0::cabina::info("red", "receptor ARMADO, anillo en la fisica", ring);
    true
}

/// **Looks at the ring and reports whatever arrived.** Returns how many frames
/// were read this time.
///
/// Every frame is announced with its source and its ethertype, because those two
/// are the whole proof: a MAC that is not ours and a protocol number that means
/// something are bytes **that another computer put on the wire**. That is the
/// question step 1 exists to answer, and no amount of register dumping answers
/// it.
pub fn rx_poll() -> u32 {
    if !rx_activo() {
        return 0;
    }
    let mut leidas = 0u32;
    unsafe {
        // Bounded by the ring length: one turn never walks more than once around,
        // so a card that returns everything at once cannot keep this loop.
        for _ in 0..bmo_net::RX_RING_LEN {
            let d = core::ptr::read_volatile(desc(RX_NEXT));
            let largo = match d.frame_len() {
                Some(l) => l,
                None => break,
            };
            let buf = mm::phys_to_virt(RX_BUFS_PHYS + (RX_NEXT * bmo_net::RX_BUF_LEN as usize) as u64)
                as *const u8;
            let trama = core::slice::from_raw_parts(buf, largo as usize);
            match bmo_net::EthHeader::parse(trama) {
                Some(h) => {
                    RX_FRAMES = RX_FRAMES.wrapping_add(1);
                    crate::ring0::cabina::info("red", "trama de", h.src_u64());
                    crate::ring0::cabina::info(
                        "red",
                        "  ...tipo y largo",
                        ((h.ethertype as u64) << 16) | largo as u64,
                    );
                }
                // Under fourteen bytes there is no header. Counted separately: a
                // runt is a cable or a filter problem, not a missing frame.
                None => {
                    crate::ring0::cabina::warn("red", "trama demasiado corta para tener cabecera", largo as u64);
                }
            }
            // Give the descriptor back to the card, with EOR preserved on the
            // last one -- rebuilding it from scratch is what keeps that bit from
            // being lost on the first wrap.
            let buf_phys = RX_BUFS_PHYS + (RX_NEXT * bmo_net::RX_BUF_LEN as usize) as u64;
            let last = RX_NEXT == bmo_net::RX_RING_LEN - 1;
            core::ptr::write_volatile(
                desc(RX_NEXT),
                bmo_net::RxDesc::to_card(buf_phys, bmo_net::RX_BUF_LEN, last),
            );
            RX_NEXT = (RX_NEXT + 1) % bmo_net::RX_RING_LEN;
            leidas += 1;
        }
    }
    leidas
}
