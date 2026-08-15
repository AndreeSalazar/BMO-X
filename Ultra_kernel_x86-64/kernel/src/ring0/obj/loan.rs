//! **PRESTAR memoria**: un proceso cede un trozo del suyo a otro.
//!
//! === Lo que este modulo NO sabe ===
//!
//! No sabe que es un lienzo, ni una ventana, ni un escritorio. **No sabe para
//! que se presta.** Mueve paginas y comprueba que quien las presta es su dueno.
//!
//! Y eso es el cambio entero respecto a la version anterior, que si lo sabia:
//! tenia `KIND_LIENZO`, una operacion para *"declarar mi lienzo"* y otra para
//! *"pedir un reflejo"*. Funcionaba, y metia un concepto de escritorio dentro
//! de Ring 0.
//!
//! * La pregunta del dueno lo destapo: *"Ring 3 no puede administrar eso el?"*.
//! Si puede, y debe. Lo unico que Ring 3 **no** puede hacer es tocar las tablas
//! de paginas -- y eso es lo unico que se queda aqui.
//!
//! | | Quien decide |
//! |---|---|
//! | cuanto se presta, a quien, cuando | **el compositor**. Es politica |
//! | mover las paginas | **el kernel**. Es mecanismo, y solo el puede |
//!
//! Esa es la separacion que hace que un microkernel valga lo que cuesta, y es
//! el patron de **seL4** -- el linaje que la hoja de ruta declara como el mas
//! cercano: el kernel no sabe que es una ventana, un fichero ni un socket.
//!
//! === Y lo que se gana, que es mas que el lienzo ===
//!
//! Con una operacion generica salen gratis el audio (un programa presta su bufer
//! al mezclador), la captura de video, y el paso de bloques grandes entre
//! procesos, que hoy tendrian que ir por un canal de mensajitos. **Una
//! operacion, cuatro problemas** -- en vez de una operacion por cada cosa nueva.
//!
//! === Se OFRECE y se TOMA, y no al reves ===
//!
//! El que presta **ofrece**: apunta que un trozo suyo es para tal proceso. El
//! que recibe **toma**: el mapeo ocurre dentro de SU llamada, en SU espacio de
//! direcciones.
//!
//! * No es un detalle de estilo. Mapear en el espacio de otro exigiria que el
//! kernel supiera el `CR3` de un proceso que no esta corriendo, y eso es
//! infraestructura que hoy no existe. Tomando, el espacio de destino es
//! `read_cr3()` -- el del que llama. El problema no se resuelve: se coloca donde
//! no existe.

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Ofertas vivas a la vez.
///
/// * Subio de 8 a 16 el 2026-08-10, y el motivo tiene nombre: **el DIRECTOR
/// tiene una por ventana**. Con el modelo de superficies, cada app en una caja
/// es un prestamo vivo mientras esa caja exista; ocho era el numero de cuando
/// solo se prestaba el escritorio a una app cada vez. Es el mismo 16 de
/// `package::MAX_VIVOS` y de `family::MAX_VIVOS`, que es el censo de programas
/// que este sistema se cree a la vez.
const MAX: usize = 16;

/// Donde empieza la zona de lo prestado en el espacio del que toma.
///
/// Lejos de `MEMORIA_VA_BASE` (`0xE000_0000`) a proposito: un proceso puede
/// tener bloques de `malloc` **y** algo prestado, y que se pisaran seria un
/// fallo sin mensaje -- escribirias encima de tu propio `malloc`.
const PRESTAMO_VA_BASE: u64 = 0x0000_0001_0000_0000;

/// Cuanto espacio de direcciones se reserva a CADA prestamo.
///
/// [!] Esto es el arreglo de un fallo que no habia salido todavia porque nadie
/// habia tomado dos cosas. `take` mapeaba **siempre en `PRESTAMO_VA_BASE`**: el
/// segundo prestamo caia encima del primero, la capability se concedia con el
/// mismo objeto --la VA-- y `operation` buscaba por `va_destino == base`, o sea
/// que dos handles distintos apuntaban al mismo sitio y contestaban lo del otro.
/// Con una app en una caja no se nota; con dos, la segunda ventana ensena los
/// pixeles de la primera y nada falla en ningun sitio.
///
/// La direccion la decide **la ranura**: `BASE + ranura * WINDOW`. Sin cursor,
/// sin contabilidad y sin poder solaparse, porque dos prestamos vivos nunca
/// comparten ranura. 64 MiB es el tope de un bloque de `KIND_MEMORIA`
/// (`memory::MAX_BYTES`), asi que lo mas grande que se puede ofrecer entra en
/// su ventana; 16 ventanas son 1 GiB de espacio de direcciones, que en 64 bits
/// no es un recurso escaso.
const PRESTAMO_VENTANA: u64 = 64 * 1024 * 1024;

/// Donde le toca a la ranura `i`.
fn va_de_ranura(i: usize) -> u64 {
    PRESTAMO_VA_BASE + i as u64 * PRESTAMO_VENTANA
}

#[derive(Clone, Copy)]
struct Offer {
    viva: bool,
    /// Quien presta, y su espacio: hace falta para traducir sus paginas.
    /// Se captura al ofrecer, que es cuando ese espacio esta cargado.
    owner: u32,
    aspace_dueno: u64,
    /// Donde empieza lo ofrecido, **en el espacio del dueno**.
    origen: u64,
    bytes: u64,
    /// A quien va. Solo el puede tomarla.
    destino: u32,
    /// Ya tomada: donde quedo en el espacio del destino, para desmapear.
    tomada: bool,
    va_destino: u64,
    /// **El dueno murio y esto sigue mapeado.** Ver [`process_died`]: las
    /// paginas se quedan, y lo unico que cambia es que [`OP_DUENO`] contesta 0.
    huerfana: bool,
}

const NOTHING: Offer = Offer {
    viva: false, owner: 0, aspace_dueno: 0, origen: 0, bytes: 0,
    destino: 0, tomada: false, va_destino: 0, huerfana: false,
};
static mut OFERTAS: [Offer; MAX] = [NOTHING; MAX];

/// Donde esta lo prestado, en MI espacio.
pub const OP_BASE: u64 = 0x01;
/// Cuantos bytes son.
pub const OP_BYTES: u64 = 0x02;
/// **El TID de quien me lo presto, o `0` si ya no vive.**
///
/// * Es el detector de vida de la ventana, y por eso existe. El DIRECTOR
/// compone la memoria de otro proceso; cuando ese proceso muere, la unica forma
/// de enterarse seria mirar la superficie y ver que la secuencia no sube -- que
/// no se distingue de una app pensando. Aqui se pregunta y se contesta.
pub const OP_DUENO: u64 = 0x03;
/// **Devolver lo prestado**: se desmapea de MI espacio y la ranura queda libre.
///
/// La contrapartida de `take`, y hace falta desde que hay mas de un prestamo: si
/// el DIRECTOR no pudiera soltar, cerrar y abrir ventanas agotaria las 16
/// ranuras y a partir de ahi ninguna app volveria a tener caja hasta reiniciar.
pub const OP_SOLTAR: u64 = 0x04;

/// **Ofrecer un trozo del bloque propio.** Devuelve `true` si quedo apuntado.
///
/// `base` es la del bloque del que ofrece --ya resuelta por su capability, o sea
/// que **es suyo por construccion**-- y `desde`/`bytes` el trozo. La unica
/// comprobacion que hace falta es que el trozo quepa dentro, y es una resta:
/// el rango lo concedio el kernel y lo tiene apuntado.
pub fn offer(owner: u32, aspace: u64, base: u64, entregado: u64, desde: u64, bytes: u64, destino: u32) -> bool {
    if bytes == 0 || desde.checked_add(bytes).map_or(true, |f| f > entregado) {
        crate::ring0::cabina::warn("prestamo", "el trozo no cabe en el bloque", desde);
        return false;
    }
    // Y que quepa en SU WINDOW, que es lo que decide donde se mapea. Se
    // comprueba al ofrecer y no al tomar porque el que ofrece es quien puede
    // hacer algo al respecto: pedir una superficie mas pequena.
    if bytes > PRESTAMO_VENTANA {
        crate::ring0::cabina::warn("prestamo", "no cabe en una ventana de prestamo", bytes);
        return false;
    }
    if destino == owner {
        return false;
    }
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    // Una oferta por pareja (dueno, destino): reofrecer sustituye, no apila.
    // Un programa que reintenta no debe llenar la tabla.
    for o in ofertas.iter_mut() {
        if o.viva && o.owner == owner && o.destino == destino && !o.tomada {
            o.origen = base + desde;
            o.bytes = bytes;
            o.aspace_dueno = aspace;
            return true;
        }
    }
    for o in ofertas.iter_mut() {
        if !o.viva {
            *o = Offer {
                viva: true, owner, aspace_dueno: aspace, origen: base + desde,
                bytes, destino, tomada: false, va_destino: 0, huerfana: false,
            };
            crate::ring0::cabina::info("prestamo", "ofrecido al pid", destino as u64);
            return true;
        }
    }
    crate::ring0::cabina::warn("prestamo", "no quedan ofertas libres", MAX as u64);
    false
}

/// **Tomar lo que me ofrecieron.** Devuelve el handle, o `None`.
///
/// El mapeo ocurre aqui, en el espacio del que llama. Se traduce pagina a
/// pagina en el espacio del dueno y se mapea en el del que toma: **los marcos
/// son los mismos, las direcciones no.** Eso es todo el prestamo.
pub fn take(pid: u32, aspace: u64) -> Option<u64> {
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    let i = ofertas.iter().position(|o| o.viva && o.destino == pid && !o.tomada)?;
    let (origen, bytes, aspace_dueno) =
        (ofertas[i].origen, ofertas[i].bytes, ofertas[i].aspace_dueno);
    // La direccion la decide LA RANURA, no un contador: ver `PRESTAMO_VENTANA`.
    let va = va_de_ranura(i);

    let paginas = bytes.div_ceil(mm::PAGE) * mm::PAGE;
    let mut off = 0u64;
    while off < paginas {
        let Some(fisica) = vmm::translate(aspace_dueno, origen + off) else {
            undo(aspace, va, off);
            crate::ring0::cabina::warn("prestamo", "lo ofrecido no esta mapeado en el dueno", off);
            return None;
        };
        if vmm::map_page(aspace, va + off, fisica, true, true).is_err() {
            // Igual que en `memory::request`: un mapeo a medias deja paginas
            // sueltas en el espacio del usuario, y eso es peor que nada.
            undo(aspace, va, off);
            return None;
        }
        off += mm::PAGE;
    }

    let handle = cap::grant(
        pid,
        cap::KIND_PRESTADO,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        va,
    );
    match handle {
        Some(h) => {
            ofertas[i].tomada = true;
            ofertas[i].va_destino = va;
            crate::ring0::cabina::info("prestamo", "tomado, bytes", bytes);
            Some(h)
        }
        None => {
            undo(aspace, va, paginas);
            None
        }
    }
}

fn undo(aspace: u64, va: u64, hasta: u64) {
    let mut off = 0u64;
    while off < hasta {
        vmm::unmap_page(aspace, va + off);
        off += mm::PAGE;
    }
}

/// Lo que contesta el handle. Ver [`OP_BASE`], [`OP_BYTES`], [`OP_DUENO`] y
/// [`OP_SOLTAR`].
///
/// `OP_SOLTAR` escribe --desmapea-- y por eso lee `read_cr3()`: durante un
/// syscall desde Ring 3, CR3 sigue siendo el del llamante. Es la misma nota que
/// llevan `memory::request` y el framebuffer, y por el mismo motivo.
pub fn operation(base: u64, op: u64, pid: u32) -> Option<u64> {
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    let i = ofertas
        .iter()
        .position(|o| o.viva && o.tomada && o.destino == pid && o.va_destino == base)?;
    match op {
        OP_BASE => Some(ofertas[i].va_destino),
        OP_BYTES => Some(ofertas[i].bytes),
        OP_DUENO => {
            if ofertas[i].huerfana {
                // El dueno murio. Se contesta 0 en vez de quitar el mapeo: ver
                // `process_died`.
                return Some(0);
            }
            Some(crate::ring0::task::scheduler::tid_de(ofertas[i].owner).unwrap_or(0) as u64)
        }
        OP_SOLTAR => {
            let paginas = ofertas[i].bytes.div_ceil(mm::PAGE) * mm::PAGE;
            undo(vmm::read_cr3(), ofertas[i].va_destino, paginas);
            crate::ring0::cabina::info("prestamo", "devuelto por el pid", pid as u64);
            ofertas[i] = NOTHING;
            // ** Y EL HANDLE SE REVOCA, que no es limpieza cosmetica.
            //
            // Sin esto, un handle viejo sigue vivo apuntando a esta VA. La
            // ranura se reutiliza, el siguiente prestamo del MISMO proceso cae
            // en la misma direccion --la elige la ranura-- y entonces el handle
            // del prestamo que ya se solto **resuelve al nuevo**: contestaria
            // por una superficie que no es la suya, sin que nada falle. Es la
            // clase de fallo que la generacion de la capability existe para
            // impedir, y aqui basta con dejarla hacer su trabajo.
            if let Some(h) = cap::find(pid, cap::KIND_PRESTADO, base) {
                cap::revoke(pid, h);
            }
            Some(1)
        }
        _ => None,
    }
}

/// **Lo llama `cap::revoke_all`.**
///
/// [!] Aqui esta el truco que mas caro se paga: `vmm::unmap_page` **devuelve el
/// marco y NO lo libera**, y eso es exactamente lo que hace falta. Los marcos
/// son **del que presto**; devolverlos al pool seria entregarle su memoria a un
/// tercero, y el fallo apareceria tres arranques despues y en otro sitio.
///
/// Se limpian las dos puntas: lo que este proceso tomo (se desmapea) y lo que
/// ofrecio (se retira, porque su espacio ya no existe para traducir).
///
/// ## ** Y si murio el dueno de algo que YA ESTABA TOMADO, no se desmapea
///
/// Es la decision que sostiene todo el modelo de superficies, asi que va dicha:
/// **el prestamo sobrevive al que lo presto.**
///
/// Lo tentador es quitarselo al que lo tomo --tenemos su `cr3` con
/// `scheduler::cr3_de_pid`-- y es justo lo que no se puede hacer: el que lo tomo
/// es el DIRECTOR, y esta componiendo. Desmapearle paginas por debajo mientras
/// las recorre es un fallo de pagina **en el compositor**, o sea que **una app
/// que se cierra se lleva el escritorio**. Que es exactamente lo que este diseno
/// existe para impedir: al lado de eso, una ventana congelada un fotograma de
/// mas no es nada.
///
/// Los marcos siguen siendo validos: `destroy_address_space` libera las tablas
/// de paginas, no las hojas. Asi que lo prestado se queda quieto y legible hasta
/// que el que lo tomo lo suelte con [`OP_SOLTAR`] -- y como sabe que soltarlo,
/// [`OP_DUENO`] le contesta 0 desde el fotograma siguiente.
/// **Queda algo de `pid` PRESTADO Y TOMADO dentro de `[base, base+bytes)`?**
///
/// La pregunta la hace [`super::memory::process_died`] antes de devolver los
/// marcos de un bloque muerto, y de la respuesta depende que el escritorio siga
/// vivo o no.
///
/// El motivo esta cuatro parrafos mas arriba, en la cabecera de
/// [`process_died`]: **el prestamo sobrevive al que lo presto**. Si una app
/// ofrecio su superficie al DIRECTOR y se muere, el DIRECTOR sigue componiendo
/// con esos marcos. Devolverlos al asignador seria entregarselos al siguiente
/// programa **mientras el compositor los esta leyendo** -- una ventana congelada
/// se convertiria en un escritorio pintando la memoria de otro.
///
/// Se pregunta por rango y no por bloque entero porque se ofrece un TROZO
/// (`base + desde`): basta con que se solape un byte para que ese bloque no se
/// pueda tocar. Aqui no vale "casi".
///
/// [!] Solo cuentan las ofertas **tomadas**. Las que nadie llego a tomar ya las
/// retiro [`process_died`], que corre antes que el de `memory` en
/// `cap::revoke_all` -- y ese orden es parte del contrato, no una casualidad.
pub fn hay_prestado_en(pid: u32, base: u64, bytes: u64) -> bool {
    let ofertas = unsafe { &*core::ptr::addr_of!(OFERTAS) };
    let fin = base.saturating_add(bytes);
    for o in ofertas.iter() {
        if !o.viva || o.owner != pid || !o.tomada {
            continue;
        }
        let o_fin = o.origen.saturating_add(o.bytes);
        if o.origen < fin && base < o_fin {
            return true;
        }
    }
    false
}

pub fn process_died(pid: u32, aspace: u64) {
    let ofertas = unsafe { &mut *core::ptr::addr_of_mut!(OFERTAS) };
    for o in ofertas.iter_mut() {
        if !o.viva {
            continue;
        }
        if o.destino == pid && o.tomada {
            let paginas = o.bytes.div_ceil(mm::PAGE) * mm::PAGE;
            undo(aspace, o.va_destino, paginas);
            crate::ring0::cabina::info("prestamo", "devuelto por el pid", pid as u64);
            *o = NOTHING;
        } else if o.owner == pid && !o.tomada {
            // Murio el que prestaba y nadie llego a tomarlo. La oferta no vale:
            // su espacio de direcciones se destruye y no habria contra que
            // traducir. Aqui si se puede tirar, porque no hay nadie mapeado.
            crate::ring0::cabina::warn("prestamo", "murio el dueno: oferta retirada", pid as u64);
            *o = NOTHING;
        } else if o.owner == pid {
            // Ver la cabecera: se queda mapeado a proposito. Lo unico que cambia
            // es que a partir de aqui `OP_DUENO` contesta 0.
            o.huerfana = true;
            crate::ring0::cabina::info("prestamo", "murio el dueno: queda huerfano", pid as u64);
        }
    }
}
