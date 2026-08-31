//! **La tarjeta de red: encontrarla y preguntarle quien es.** Nada mas.
//!
//! [carril]  AMARILLO  la tarjeta de red: EN OBRAS, y por eso amarilla
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
    crate::ring0::cabina::addr("red", "y esta en la direccion fisica", loc.mmio);

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


    // ================================================================
    //  *** LO QUE LA PLACA OFRECE PARA ESTA TARJETA (2026-08-24)
    // ================================================================
    //
    //  El dueno lo pidio asi: *"a base de lo que la placa ofrece, recuerda que
    //  llame y que datos DAN y exprimir"*. Y la placa da dos cosas que hoy no
    //  se estaban ni mirando -- las dos deciden lo que se puede hacer despues.
    //
    //  ** No se PROGRAMA nada aqui. Se lee y se cuenta, que es el paso 0 de
    //  siempre: predecir, leer, comparar. Encender MSI el mismo dia que se
    //  descubre que existe seria cambiar dos cosas a la vez.
    la_placa_dice(&loc);

    crate::ring0::cabina::mac("red", "MAC", id.mac_u64());
    if !id.creible() {
        // Ceros o unos no dicen "tarjeta rota": dicen que la lectura no llego al
        // aparato. Es el BAR, no la NIC, y confundirlos manda a cambiar de
        // tarjeta cuando lo que hay que cambiar es un indice.
        crate::ring0::cabina::fault("red", "esa MAC no es creible: el BAR no lleva a los registros", id.mac_u64());
        return;
    }
    crate::ring0::cabina::bits("red", "PHYstatus crudo", id.phy as u64);
    if id.enlace_arriba() {
        crate::ring0::cabina::count("red", "enlace ARRIBA, megabits", id.megabits() as u64);
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

/// **El plano del corral**, una vez reclamado. `None` = sin arrancar.
///
/// *** 2026-08-24: ANTES ERAN DOS RESERVAS Y UNA CUENTA A MANO.
///
/// El anillo iba por un lado, los buferes por otro, y la direccion de cada
/// bufer se calculaba en el sitio --`bufs + i * RX_BUF_LEN`-- sin que nada
/// comprobara que caia donde tenia que caer. Funcionaba. Y esa es exactamente
/// la forma del bug caro: **la aritmetica que decide donde escribe un aparato
/// no daba un fallo si estaba mal, daba una direccion.**
///
/// Ahora es UNA arena contigua y el reparto lo hace `bmo_net::anillo::Plan`,
/// que vive en el crate del driver **porque alli hay banco de pruebas en el
/// anfitrion**: nueve casos que comprueban que ningun bufer se sale, que
/// ninguno pisa al vecino, que hay exactamente un `EOR`, y que un largo que
/// desborda el contador no cuela.
///
/// [!] El kernel ya no calcula ninguna direccion de DMA. La pide y la comprueba.
static mut PLANO: Option<bmo_net::anillo::Plan> = None;
/// Which descriptor is next to be looked at. The card walks the ring in order and
/// so do we.
static mut RX_NEXT: usize = 0;
/// Frames seen since the ring started. The number that answers the question.
static mut RX_FRAMES: u64 = 0;
/// Bytes de trama recibidos, **sin contar el FCS**: lo que se leyo de verdad.
static mut RX_BYTES: u64 = 0;
/// El reparto por protocolo: `[ARP, IPv4, IPv6, otros]`.
///
/// *** CUATRO CASILLAS Y NO UNA, y es lo que convierte "hay trafico" en una
/// lectura. En una red domestica en reposo lo que llega es **ARP y broadcast**;
/// si sale IPv4 sin que nadie haya pedido nada, hay alguien hablando. Un solo
/// contador de tramas no distingue las dos cosas, y son la diferencia entre
/// "el cable esta vivo" y "esta red tiene vecinos".
static mut RX_TIPOS: [u64; 4] = [0; 4];
/// Tramas mas cortas que una cabecera. Aparte: es cable o filtro, no trafico.
static mut RX_CORTAS: u64 = 0;

/// **El consumo del receptor.** `(tramas, bytes, [arp, ipv4, ipv6, otros], cortas)`.
pub fn rx_consumo() -> (u64, u64, [u64; 4], u64) {
    unsafe { (RX_FRAMES, RX_BYTES, RX_TIPOS, RX_CORTAS) }
}

/// **Lo que la TARJETA dice que perdio.** `None` si no hay NIC legible.
///
/// *** El unico numero de esta pagina que no lo lleva BMO-X. Un contador propio
/// solo puede contar lo que cogio; lo que se perdio por no tener descriptor
/// libre **solo lo sabe el silicio**. Sin esto, "40 tramas recibidas" es una
/// cifra sin denominador.
pub fn rx_perdidas() -> Option<u32> {
    let mmio = unsafe { MMIO };
    if mmio.is_null() {
        return None;
    }
    // Solo lectura: escribir aqui pondria el contador a cero, y un instrumento
    // que borra lo que mide al mirarlo no sirve para mirar dos veces.
    Some(unsafe { core::ptr::read_volatile(mmio.add(bmo_net::reg_rx::MPC) as *const u32) })
}

/// Is the receiver armed?
pub fn rx_activo() -> bool {
    unsafe { PLANO.is_some() }
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
///
/// [!] `i` ya viene acotado por el bucle que llama, pero el plano es quien
/// tiene la ultima palabra sobre donde empieza el anillo: aqui no se suma nada
/// que no venga de el.
unsafe fn desc(p: &bmo_net::anillo::Plan, i: usize) -> *mut bmo_net::RxDesc {
    let virt = mm::phys_to_virt(p.descriptores()) as *mut bmo_net::RxDesc;
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
        crate::ring0::cabina::count("red", "el receptor ya estaba armado, tramas", rx_tramas());
        return true;
    }

    // *** UNA SOLA RESERVA, Y ES EL CORRAL.
    //
    // ** Antes eran dos --anillo por un lado, buferes por otro-- y entre las dos
    // no habia ninguna relacion que se pudiera comprobar. Con una arena
    // contigua hay UNA pregunta que lo decide todo: "esta esta direccion dentro
    // de la arena?", y esa pregunta tiene una funcion con nombre y con tests.
    //
    // [!] La tarjeta escribe DONDE SE LE MANDE. Si todas las direcciones que se
    // le dan estan dentro del corral, un error de cuenta mio corrompe mi propio
    // bufer de red -- visible, reproducible, y sin llevarse nada por delante.
    let bytes = bmo_net::anillo::bytes_necesarios();
    let paginas = (bytes + mm::PAGE - 1) / mm::PAGE;
    let arena = match crate::ring0::mm::phys::alloc_frames_contig(paginas) {
        Some(p) => p,
        None => {
            crate::ring0::cabina::fault("red", "sin marcos contiguos para el corral de DMA", paginas);
            return false;
        }
    };

    // ** Y EL PLANO SE VALIDA ANTES DE TOCAR NADA. Un marco viene alineado a
    // 4096 y `RDSAR` pide 256, asi que esto tendria que pasar siempre -- razon
    // de mas para comprobarlo: lo que "tendria que pasar siempre" es justo lo
    // que nadie mira el dia que deja de pasar.
    let plan = match bmo_net::anillo::Plan::nuevo(arena, paginas * mm::PAGE) {
        Ok(p) => p,
        Err(e) => {
            let cual = match e {
                bmo_net::anillo::Falta::NoAlineada => 1,
                bmo_net::anillo::Falta::Pequena => 2,
                bmo_net::anillo::Falta::Desborda => 3,
            };
            crate::ring0::cabina::fault("red", "el corral no pasa su propia revision (1=alineacion 2=corta 3=desborda)", cual);
            return false;
        }
    };

    unsafe {
        PLANO = Some(plan);
        RX_NEXT = 0;
        RX_FRAMES = 0;
        RX_BYTES = 0;
        RX_TIPOS = [0; 4];
        RX_CORTAS = 0;

        // Los descriptores salen del plano ENTEROS, `EOR` incluido. El kernel no
        // arma ninguno: copia lo que el modulo probado le da.
        for i in 0..bmo_net::RX_RING_LEN {
            match plan.descriptor(i) {
                Some(d) => core::ptr::write_volatile(desc(&plan, i), d),
                None => {
                    // No puede pasar --`i` viene del propio largo del anillo--
                    // y por eso si pasa hay que parar aqui y no seguir con un
                    // anillo a medio armar, que es un anillo que la tarjeta
                    // recorre igual.
                    crate::ring0::cabina::fault("red", "el plano rechazo un descriptor que deberia existir", i as u64);
                    PLANO = None;
                    return false;
                }
            }
        }

        // *** Y QUE LOS DESCRIPTORES ESTEN EN MEMORIA ANTES DE QUE LA TARJETA
        // SEPA DONDE MIRAR.
        //
        // ** En x86 el DMA es coherente con la cache, asi que no hace falta
        // vaciar nada -- y eso es UNA PROPIEDAD DE ESTA ARQUITECTURA, no una
        // ley: en ARM habria que limpiar la linea a mano y este mismo codigo
        // recibiria descriptores viejos. Se dice porque el dia que alguien
        // porte esto, esta es la linea que hay que leer.
        //
        // Lo que si hace falta es que el COMPILADOR no mueva las escrituras de
        // arriba por debajo del `RDSAR` de abajo. Eso es lo que ata la valla.
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

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
                PLANO = None;
                return false;
            }
            core::hint::spin_loop();
        }
        crate::ring0::cabina::count("red", "reset completado, vueltas", spins as u64);

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
        let anillo = plan.descriptores();
        w32(mmio, reg_rx::RDSAR_LO, (anillo & 0xFFFF_FFFF) as u32);
        w32(mmio, reg_rx::RDSAR_HI, (anillo >> 32) as u32);

        // 5. La tabla de multicast, ANTES del filtro que la usa.
        //
        //    *** ESTO FALTABA, Y ERA LA MITAD DE `AM` (2026-08-28).
        //
        //    ** `rx_config()` pide multicast desde el primer dia, pero el reset
        //    deja `MAR` a ceros y ningun bit de esa tabla deja pasar nada. O
        //    sea: un filtro pedido que no admitia ni una trama. Y lo que mas
        //    suena en una LAN parada es multicast --mDNS, SSDP-- asi que la
        //    foto del paso 1 se estaba pidiendo con el grifo medio cerrado.
        //
        //    [!] Va antes de `RCR` a proposito: la tabla tiene que estar puesta
        //    cuando se enciende el filtro que la consulta, no despues.
        w32(mmio, reg_rx::MAR0, bmo_net::mar_todos());
        w32(mmio, reg_rx::MAR0 + 4, bmo_net::mar_todos());

        // 6. What gets accepted. Broadcast is the one that matters: it is what
        //    makes a plugged cable produce traffic with nobody doing anything.
        w32(mmio, reg_rx::RCR, bmo_net::rx_config());

        w8(mmio, reg_rx::CFG9346, 0x00);

        // 7. And only now, the receiver. TE stays OFF.
        let c = r8(mmio, reg_rx::CR);
        w8(mmio, reg_rx::CR, (c & !cr::TE) | cr::RE);
    }

    crate::ring0::cabina::addr("red", "receptor ARMADO, corral en la fisica", plan.descriptores());
    crate::ring0::cabina::bytes("red", "  ...y el corral mide", plan.bytes());
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
    let plan = match unsafe { PLANO } {
        Some(p) => p,
        None => return 0,
    };
    unsafe {
        // Bounded by the ring length: one turn never walks more than once around,
        // so a card that returns everything at once cannot keep this loop.
        for _ in 0..bmo_net::RX_RING_LEN {
            let d = core::ptr::read_volatile(desc(&plan, RX_NEXT));
            let largo = match d.frame_len() {
                Some(l) => l,
                None => break,
            };
            // *** Y AQUI SE PREGUNTA OTRA VEZ, aunque el plano ya lo hubiera
            // garantizado al armar el anillo.
            //
            // ** No es paranoia repetida: `largo` **lo escribio la tarjeta**, y
            // este es el unico sitio de todo el driver donde un numero venido de
            // fuera decide cuanta memoria se lee. Una trama de 2049 bytes en un
            // bufer de 2048 sale del corral por un byte, y ese byte se lo cree
            // el parser de Ethernet como si fuera suyo.
            let Some(buf_fis) = plan.bufer(RX_NEXT) else { break };
            if !plan.contiene(buf_fis, largo as u64) {
                crate::ring0::cabina::fault("red", "la tarjeta declara una trama que NO CABE en su bufer", largo as u64);
                break;
            }
            let buf = mm::phys_to_virt(buf_fis) as *const u8;
            let trama = core::slice::from_raw_parts(buf, largo as usize);
            match bmo_net::EthHeader::parse(trama) {
                Some(h) => {
                    RX_FRAMES = RX_FRAMES.wrapping_add(1);
                    RX_BYTES = RX_BYTES.wrapping_add(largo as u64);
                    let casilla = match h.ethertype {
                        0x0806 => 0,
                        0x0800 => 1,
                        0x86DD => 2,
                        _ => 3,
                    };
                    RX_TIPOS[casilla] = RX_TIPOS[casilla].wrapping_add(1);

                    // *** LA FOTO DEL PASO 1, y son CUATRO lineas y no dos.
                    //
                    // ** Antes se imprimian el origen y un numero con el tipo y
                    // el largo EMPAQUETADOS en un solo `u64`. Las dos cosas
                    // estaban mal por el mismo motivo:
                    //
                    //   1. `((tipo << 16) | largo)` obliga a quien mira la
                    //      pantalla a deshacer un desplazamiento con la cabeza.
                    //      Un numero que hay que decodificar no es una lectura.
                    //   2. Sin el DESTINO, esta foto no distingue "el filtro de
                    //      recepcion funciona" de "el filtro esta abierto de
                    //      par en par" -- las dos dan los mismos origenes. El
                    //      paso 1 tiene tres preguntas que contestar y asi solo
                    //      contestaba dos.
                    crate::ring0::cabina::mac("red", "trama DE", h.src_u64());
                    crate::ring0::cabina::mac("red", "     PARA", h.dst_u64());
                    // ** EL TIPO CON SU NOMBRE EN EL MENSAJE. `0x0806` es ARP
                    // para quien tenga la tabla memorizada y no es nada para
                    // todos los demas -- y esta linea la lee una persona una
                    // vez, decidiendo si el driver sirve.
                    crate::ring0::cabina::id("red", h.nombre_del_tipo(), h.ethertype as u64);
                    crate::ring0::cabina::bytes("red", "     largo", largo as u64);

                    // *** LA TRAMPA QUE HAY QUE CAZAR AQUI Y NO TRES ARRANQUES
                    // DESPUES: que el origen sea NUESTRA PROPIA MAC.
                    //
                    // ** El paso 1 NO TRANSMITE -- `CR.TE` se queda apagado a
                    // proposito-- asi que una trama que diga venir de nosotros
                    // no puede ser nuestra. Significa una de dos, y las dos son
                    // hallazgos: la tarjeta esta en loopback interno, o el
                    // anillo esta leyendo memoria que no es la suya.
                    //
                    // Sin este aviso, eso se veria como "la red RECIBE" y la
                    // casilla se pondria verde por el motivo equivocado.
                    if let Some(yo) = ID {
                        if h.src_u64() == yo.mac_u64() {
                            crate::ring0::cabina::warn(
                                "red",
                                "[!] dice venir de NOSOTROS y aqui no se transmite",
                                h.src_u64(),
                            );
                        }
                    }
                }
                // Under fourteen bytes there is no header. Counted separately: a
                // runt is a cable or a filter problem, not a missing frame.
                None => {
                    RX_CORTAS = RX_CORTAS.wrapping_add(1);
                    crate::ring0::cabina::count("red", "trama demasiado corta para tener cabecera", largo as u64);
                }
            }
            // Give the descriptor back to the card, with EOR preserved on the
            // last one -- rebuilding it from scratch is what keeps that bit from
            // being lost on the first wrap.
            // Devuelto al plano, `EOR` incluido: reconstruirlo desde el plano
            // --y no a mano-- es lo que impide perder ese bit en la primera
            // vuelta del anillo, que es el unico fallo de aqui que se sale del
            // corral.
            if let Some(nuevo) = plan.descriptor(RX_NEXT) {
                core::ptr::write_volatile(desc(&plan, RX_NEXT), nuevo);
            }
            RX_NEXT = (RX_NEXT + 1) % bmo_net::RX_RING_LEN;
            leidas += 1;
        }
    }
    leidas
}


/// **Que declara la placa sobre esta NIC**, leido y contado. No cambia nada.
///
/// # Las dos preguntas que deciden la red de manana
///
/// ```text
///    MSI?          hoy el driver SONDEA. Con MSI la tarjeta AVISA, y eso es
///                  lo que TCP necesita: un RTT medido con un bucle de sondeo
///                  mide el bucle, no la red
///    ancho PCIe    cuanto cabe por el bus. El enlace Ethernet son 10 Mbit
///                  hoy, asi que el bus SOBRA -- pero el dia que no,
///                  este numero es el que lo dice
/// ```
///
/// *** Y la capability extendida solo se alcanza por ECAM, o sea por el MCFG,
/// o sea **porque la placa lo conto**. Por el camino de puertos de siempre
/// --256 bytes-- estos registros no existen. Es literalmente exprimir lo que
/// el firmware dio.
///
/// [!] Si ECAM no se creyo, se dice y no se inventa. Un numero de un sitio en
/// el que no se confia es peor que la ausencia del numero.
fn la_placa_dice(loc: &pci::NetLoc) {
    // La lista encadenada de capabilities vive en los 256 bytes de siempre, asi
    // que MSI se puede preguntar aunque ECAM no este.
    let estado = pci::cfg_read32(loc.bus, loc.dev, loc.func, 0x04) >> 16;
    if estado & (1 << 4) == 0 {
        crate::ring0::cabina::warn("red", "la NIC no anuncia NINGUNA capability", estado as u64);
        return;
    }
    let mut off = (pci::cfg_read32(loc.bus, loc.dev, loc.func, 0x34) & 0xFC) as u8;
    let mut msi = false;
    let mut pcie_off = 0u8;
    // ** Acotado a 48 saltos: una lista encadenada que un aparato construye mal
    // puede apuntarse a si misma, y un bucle infinito en el arranque no da
    // autopsia. Mismo motivo que el tope del Report Count de HID.
    for _ in 0..48 {
        if off < 0x40 {
            break;
        }
        let cab = pci::cfg_read32(loc.bus, loc.dev, loc.func, off);
        match (cab & 0xFF) as u8 {
            0x05 => msi = true,
            0x10 => pcie_off = off,
            _ => {}
        }
        off = ((cab >> 8) & 0xFC) as u8;
    }

    // *** LA FILA QUE ABRE LA PUERTA DE TCP. Hoy `rx_poll` sondea; con MSI la
    // tarjeta escribe en memoria y el CPU llega solo. Un RTT medido con un
    // bucle de sondeo mide el bucle.
    if msi {
        crate::ring0::cabina::info("red", "la placa OFRECE MSI: la NIC puede avisar sin sondeo", 1);
    } else {
        crate::ring0::cabina::warn("red", "sin MSI: solo queda sondear", 0);
    }

    if pcie_off == 0 {
        return;
    }
    // `Link Status` esta en +0x12 de la capability PCI Express. Los 4 bits
    // bajos son la velocidad y los 6 siguientes el ancho.
    let ls = pci::cfg_read32(loc.bus, loc.dev, loc.func, pcie_off + 0x10) >> 16;
    let gen = (ls & 0xF) as u64;
    let carriles = ((ls >> 4) & 0x3F) as u64;
    crate::ring0::cabina::count("red", "PCIe: generacion del enlace", gen);
    crate::ring0::cabina::count("red", "PCIe: carriles", carriles);

    // Y lo que SOLO se alcanza por ECAM, o sea gracias al MCFG.
    if !pci::hay_ecam() {
        crate::ring0::cabina::warn("red", "sin ECAM: las capabilities extendidas no se pueden leer", 0);
        return;
    }
    let mut ext = [pci::CapExt { id: 0, version: 0, offset: 0 }; 16];
    let n = pci::caps_extendidas(loc.bus, loc.dev, loc.func, &mut ext);
    crate::ring0::cabina::count("red", "capabilities EXTENDIDAS (solo por ECAM)", n as u64);
    for c in ext.iter().take(n) {
        crate::ring0::cabina::id("red", pci::nombre_cap_ext(c.id), c.id as u64);
    }
}