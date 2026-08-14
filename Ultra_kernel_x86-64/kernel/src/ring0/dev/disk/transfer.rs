//! **MOVING THE BYTES** -- read, DMA, and the bounce buffer.
//!
//! === Why this is a file of its own ===
//!
//! Because it is the only part of the driver with a **physical** problem
//! underneath. Everything else here is registers and policy; this is where a
//! buffer that Ring 3 handed over has to become an address the controller can
//! reach, and the two are not the same kind of thing.
//!
//! === The distinction worth keeping visible ===
//!
//! `tramo_dma` asks whether the caller's memory is contiguous in PHYSICAL terms
//! for long enough to hand straight to the controller. When it is, the bytes go
//! direct and nothing is copied. When it is not, `leer_rebotando` copies through
//! a bounce buffer.
//!
//! Both paths are counted (`cuentas_dma`), and that pair of numbers is the whole
//! point: **the difference between a read that copied and one that did not is
//! invisible from outside and is most of the cost.** Without the counter,
//! "reading is slow" has no next question.
//!
//! [!] `MAX_POR_COMANDO` is 8192 sectors, and it is a limit of the command and
//! not of the disk -- a request larger than that is split, not refused.

use super::*;

/// Lee `count` sectores desde `lba` en `buf`. Devuelve los sectores leidos.
///
/// La lectura no tiene ventana ni gate: mirar un sector no rompe nada, y es
/// justo mirando como BMO averigua de quien es el disco.
///
/// Va por la pagina de rebote y copia: el llamante puede tener su buffer donde
/// quiera --la pila, un estatico-- sin que nada de eso tenga que ser memoria
/// apta para DMA ni de direccion fisica contigua.
pub fn read(lba: u64, count: u16, buf: &mut [u8]) -> u16 {
    if !is_ready() || count == 0 { return 0; }
    let want = count as usize * SECTOR;
    if buf.len() < want { return 0; }
    let dma = unsafe { DMA_PHYS };
    if dma == 0 { return 0; }
    // ** El disco es de UNO cada vez. Ver `tomar_disco`: sin esto, dos lecturas
    // solapadas --y el temporizador expropia en cualquier punto-- se pisan la
    // ranura 0 y la primera acaba leyendo el `PRDBC` de la segunda.
    let _testigo = tomar_disco();

    const PER_BATCH: u16 = (4096 / SECTOR) as u16; // 8 sectores por pagina
    let mut done = 0u16;
    while done < count {
        // == ** EL CAMINO DIRECTO: el HBA escribe EN EL BUFFER DEL LLAMANTE ==
        //
        // Escalon 3 de `LA_RAM.md`. Si el trozo que toca ahora esta seguido en
        // memoria FISICA, se le da esa direccion al HBA y no se copia nada: el
        // dato va del disco a su sitio y no pasa por ninguna parte.
        //
        // No se supone que lo este: **se comprueba**, pagina a pagina, con la
        // tabla que el propio kernel monto. Cuatro lecturas de memoria por
        // pagina frente a copiar 4096 bytes -- y si la respuesta es que no, se
        // cae al rebote de siempre sin que nadie se entere.
        let va = buf.as_ptr() as u64 + done as u64 * SECTOR as u64;
        let restante = (count - done) as u64 * SECTOR as u64;
        // Un tramo que no da ni para un sector entero no sirve: se rebota.
        let directo = tramo_dma(va, restante).and_then(|(phys, bytes)| {
            let sectores = ((bytes / SECTOR as u64) as u16).min(count - done).min(MAX_POR_COMANDO);
            if sectores == 0 { None } else { Some((phys, sectores)) }
        });

        let (batch, got) = match directo {
            Some((phys, batch)) => {
                let got = match mandar_lectura(lba + done as u64, batch, phys) {
                    Some(n) => n,
                    None => return done,
                };
                unsafe { SIN_REBOTE += got as u64 * SECTOR as u64; }
                (batch, got)
            }
            None => {
                let batch = (count - done).min(PER_BATCH);
                match leer_rebotando(lba + done as u64, batch, dma, buf, done) {
                    Some(got) => (batch, got),
                    None => return done,
                }
            }
        };
        if got == 0 { return done; }
        done += got;
        if got < batch { break; } // lectura corta: el disco dijo basta
    }
    done
}

/// Lo mas grande que puede pedir UN comando.
///
/// El PRDT lleva el contador de bytes en 22 bits (`DBC`, ver
/// `bmo_ahci::run_command`), o sea 4 MiB por entrada, y aqui se usa **una sola
/// entrada**. 8192 sectores son exactamente esos 4 MiB.
const MAX_POR_COMANDO: u16 = 8192;

/// Bytes que llegaron a su sitio SIN pasar por la pagina de rebote.
///
/// Existe para poder decir el numero. Un camino rapido que nadie mide es un
/// camino rapido que un dia deja de tomarse --una pagina que cambia de sitio,
/// un buffer que se desalinea-- y todo sigue funcionando, solo que despacio y
/// sin que nada lo diga.
static mut SIN_REBOTE: u64 = 0;
/// Bytes que SI tuvieron que rebotar.
static mut CON_REBOTE: u64 = 0;

/// `(directos, rebotados)` en bytes desde el arranque.
pub fn cuentas_dma() -> (u64, u64) { unsafe { (SIN_REBOTE, CON_REBOTE) } }

/// ** LA PRIMITIVA DE LECTURA: un LBA, unos sectores, y una direccion FISICA.
///
/// === Por que la de verdad es esta y no `read` ===
///
/// El HBA no sabe lo que es una direccion virtual. Lo unico que entiende es
/// *"escribe estos sectores AQUI"*, con `aqui` en fisico. Todo lo demas --mirar
/// las tablas de pagina, comprobar continuidad, caer al rebote-- es trabajo que
/// hace [`read`] **para poder acabar llamando a esto**.
///
/// Que estuviera al reves --la funcion publica pidiendo un `&mut [u8]` y la
/// fisica escondida dentro-- obliga a **preguntar** por una traduccion que en el
/// caso que viene (la pieza B: el disco escribiendo en los marcos del proceso)
/// **ya se sabe sin preguntar**: un marco recien pedido al asignador mide una
/// pagina, es contiguo por definicion, y el asignador acaba de devolver su
/// direccion fisica. Preguntarle a las tablas donde esta algo que uno mismo
/// acaba de reservar es trabajo, y es una respuesta de la que fiarse.
///
/// Hoy la llaman los dos caminos de `read` --el directo y el de rebote-- que es
/// lo unico que hay. Cuando la pieza B entre, entrara **por aqui**, sin
/// envoltorio y sin traducir nada.
///
/// [!] Da por hecho que el disco YA esta tomado. Ver `tomar_disco`: tomarlo aqui
/// dentro convertiria cada vuelta de `read` en una peticion anidada.
fn mandar_lectura(lba: u64, count: u16, phys: u64) -> Option<u16> {
    match unsafe { bmo_ahci::read_sectors_phys(unsafe { PORT }, lba, count, phys) } {
        Ok(n) => Some(n),
        Err(e) => {
            // El LBA y no el numero de sectores: cuando un disco se queja, lo
            // que hace falta saber es DONDE, para poder mirar ese sector con
            // otra herramienta.
            crate::ring0::cabina::fault("disk", e.name(), lba);
            None
        }
    }
}

/// El trozo de rebote de siempre, para cuando el destino no sirve para DMA.
fn leer_rebotando(lba: u64, batch: u16, dma: u64, buf: &mut [u8], done: u16) -> Option<u16> {
    let got = mandar_lectura(lba, batch, dma)?;
    if got == 0 { return None; }
    let src = mm::phys_to_virt(dma) as *const u8;
    let dst_off = done as usize * SECTOR;
    let n = got as usize * SECTOR;
    unsafe {
        core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr().add(dst_off), n);
        CON_REBOTE += n as u64;
    }
    Some(got)
}

/// **Cuanto, a partir de `va`, se puede entregarle al HBA tal cual.**
///
/// Devuelve `(direccion fisica, bytes seguidos)`, o `None` si esa direccion no
/// sirve para DMA. Dos condiciones, y las dos son del hardware:
///
/// 1. **Contiguo en fisico.** El PRDT que arma `bmo-ahci` lleva UNA entrada:
///    una direccion y una longitud. Dos paginas que en virtual estan pegadas y
///    en fisico no, escritas como si fueran una, le entregarian al HBA media
///    pagina de alguien.
/// 2. **Alineado a 2 bytes.** Lo pide AHCI para la base del PRD. Es gratis
///    comprobarlo y el sintoma de no hacerlo seria una transferencia que el HBA
///    rechaza o desplaza.
///
/// [!] Se pregunta con [`vmm::fisica_exacta`] y **no con `translate`**, que
/// contesta la base de la pagina en 4 KiB y la direccion exacta en las grandes.
/// Sumarle el desplazamiento a la segunda apunta unos bytes mas alla -- y como
/// el physmap del kernel esta montado con paginas de 2 MiB, ese es justo el caso
/// de cualquier buffer que viva ahi. No seria una lectura mala: seria el disco
/// escribiendo encima de memoria de otro. Ver la cabecera de esa funcion.
///
/// * Y sobre el PML4 **del kernel**, no sobre el CR3 actual: esto puede correr
/// dentro de un syscall de Ring 3, donde CR3 es el del proceso. El buffer del
/// cargador existe igual en ese espacio porque la mitad alta se comparte, pero
/// preguntarselo al espacio equivocado seria confiar en esa coincidencia.
fn tramo_dma(va: u64, max: u64) -> Option<(u64, u64)> {
    if max < SECTOR as u64 {
        return None;
    }
    // ** SOLO EL PHYSMAP, Y LA TRADUCCION ES UNA RESTA (2026-08-10).
    //
    // === Lo que habia aqui, y por que se fue ===
    //
    // Esto caminaba las tablas de pagina con `vmm::fisica_exacta` para cualquier
    // direccion que le dieran, y le entregaba al HBA la respuesta. O sea que le
    // PREGUNTABA a una estructura de datos donde vive un buffer, y se fiaba.
    //
    // El arranque del 2026-08-10 dijo que eso no se sostiene: la MISMA lectura
    // --mismo fichero, mismo LBA, mismo codigo-- funcionaba con el destino en un
    // estatico de `.bss` y fallaba con el destino en la pila. Si una lectura sale
    // bien o mal segun DONDE la pongas, el sospechoso no es el disco: es la
    // traduccion.
    //
    // === Lo que hay ahora ===
    //
    // El physmap es un espejo LINEAL: `virt = phys + HIGH_MEM_BASE`. Para una
    // direccion de esa ventana la fisica no hay que preguntarla, **se resta**. Y
    // dos direcciones seguidas ahi son dos fisicas seguidas por construccion, asi
    // que la continuidad tampoco hay que comprobarla pagina a pagina: la garantiza
    // el mapeo, no una comprobacion que un dia mira mal.
    //
    // Todo lo demas --la pila, `.bss`, la imagen del kernel-- **rebota**. Es mas
    // lento y es correcto, y son lecturas pequenas: el prologo son 2 KB.
    //
    // ** Y el camino rapido NO se pierde donde importa. La pieza B aterriza las
    // secciones en marcos recien pedidos al asignador, y a esos se llega por
    // `mm::phys_to_virt`, que **es** una direccion del physmap. O sea que el DMA
    // directo se queda exactamente en el sitio para el que se invento, y
    // desaparece de los sitios donde nadie podia garantizar nada.
    //
    // > No es que ahora se compruebe mejor. Es que **ya no hay nada que
    // > comprobar**: la respuesta se sabe sin preguntar.
    let fin_physmap = mm::HIGH_MEM_BASE.wrapping_add(mm::PHYSMAP_SIZE);
    if va < mm::HIGH_MEM_BASE || va >= fin_physmap {
        return None;
    }
    // ** LA ALINEACION VUELVE A COMPROBARSE, Y AHORA HACE FALTA DE VERDAD.
    //
    // AHCI pide que la base del PRD este alineada a 2 bytes. Mientras por aqui
    // solo pasaban marcos recien pedidos al asignador esto no podia fallar --una
    // pagina esta alineada a 4096-- y por eso la comprobacion se habia quedado
    // sin escribir en la version del physmap.
    //
    // Desde el 2026-08-11 pasa tambien **el espejo de un bloque de Ring 3**: un
    // `fread` a mitad de un lump puede caer en una direccion impar. Y sin este
    // `if` eso no seria un rebote, seria una LECTURA CORTA: `bmo_ahci` rechaza
    // la peticion con `BadRequest`, `read` devuelve lo que llevaba, y el fichero
    // llega a medias con un fault en el log que habla del disco.
    //
    // > Una condicion que "en la practica siempre se cumple" deja de cumplirse
    // > el dia que un camino nuevo entra por la misma puerta. Cuesta un `and`.
    if va & 1 != 0 {
        return None;
    }
    let phys = va - mm::HIGH_MEM_BASE;
    // Lo que quede de ventana, por si un buffer enorme llegara al tope. No puede
    // pasar --el asignador nunca entrega marcos por encima de `PHYSMAP_SIZE`, y
    // esta escrito en su cabecera-- pero acotarlo cuesta una resta.
    let bytes = max.min(fin_physmap - va);
    if bytes < SECTOR as u64 {
        return None;
    }
    Some((phys, bytes))
}

// -- ** PEDIR SIN ESPERAR ----------------------------------------------------
//
// === Que es y que no es ===
//
// [`read`] no bloquea la MAQUINA: el temporizador expropia mientras gira, asi
// que el escritorio sigue pintando. Lo que si bloquea es **al que llamo**, que
// se queda dentro de una funcion de Ring 0 hasta que el disco conteste.
//
// Estas dos operaciones parten eso en dos momentos: se PIDE, se vuelve, y se
// pregunta despues. Quien pida puede hacer otra cosa entre medias -- que es la
// definicion de E/S asincrona, y lo que hace falta para que un programa de Ring
// 3 lea un archivo grande sin quedarse mudo mientras tanto.
//
// === El dueno se queda TOMADO entre las dos llamadas ===
//
// Y es lo que las hace seguras. Un comando en vuelo es estado global del puerto
// (una ranura, un PRDT, un `PRDBC`); si el disco quedara libre entre pedir y
// recoger, cualquiera podria emitir encima y el que pidio recogeria lo del otro.
// Por eso [`pedir_lectura`] devuelve el testigo: **quien pide se lleva el disco
// hasta que recoge**, y si muere por el camino el siguiente se lo quita.
//
// === Lo que sigue faltando, dicho ===
//
// Esto es la mitad del escalon 4. La otra mitad es que el que espera pueda
// dormirse en vez de preguntar -- y eso pide una INTERRUPCION del HBA y un
// `wait_key` del planificador, no un driver distinto. El sondeo es lo que se
// puede tener hoy sin tocar el reparto de turnos, y es lo mismo que hace el
// compositor con la entrada sesenta veces por segundo.

// ** Y AQUI NO HAY UN `pedir_lectura` / `lectura_lista`, A PROPOSITO.
//
// Se escribieron, compilaban, y se quitaron antes de entrar: **no habia quien
// los llamara**. Este proyecto ya se ha tropezado tres veces con el mismo
// patron --el foco con sus doce pruebas y sin lector, el arrastre de dos
// ventanas que nadie invocaba, el `count` del contrato sin usar-- y en todas la
// version escrita y muerta dio la impresion de que la funcion existia.
//
// La pieza que falta para que tengan sentido NO es un driver: es que el que
// pide pueda irse a hacer otra cosa. Hoy quien lee un archivo lo lee entero
// dentro de `file::open`, asi que no hay ningun momento en el que "pedir y
// volver" cambie nada. Lo que hay que mover esta escrito en `LA_RAM.md`.

// -- El gate de identidad ----------------------------------------------------
