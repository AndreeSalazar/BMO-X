//! **`KIND_MEMORIA`** -- pedirle memoria al kernel, y que sea tuya.
//!
//! [carril]  ROJO      pedirle memoria al kernel, y que sea tuya
//!
//! generacion: nieto -- CADENA DE LLAMADAS, no tuberia: esta etiqueta dice
//! cuanto SABE esta pieza, no quien importa a quien, y por eso el
//! guardian de L7 no la juzga (ver L7c en `META-KERNEL_HARD.md`).
//! no sabe: quien lo llamo ni por que
//!
//! === El hueco que tapa ===
//!
//! Hasta ahora un proceso recibia su imagen y 64 KiB de pila, **y no podia
//! pedir mas**. Eso bloqueaba dos cosas a la vez y por eso lleva tanto tiempo
//! en la hoja de ruta: cualquier lenguaje con recolector de basura, y cualquier
//! programa que no sepa de antemano cuanto va a necesitar.
//!
//! === * Por que NO es un `malloc` ===
//!
//! Aqui se entrega **un bloque grande, entero y contiguo**, y se acabo. No hay
//! listas de libres, ni troceado, ni fusion de huecos. Y no es una version
//! recortada de un asignador: es que **el asignador no es trabajo del kernel**.
//!
//! El caso que lo ensena es DOOM, y por eso es el que se uso para decidir:
//! pide ~8 MiB **una vez** al arrancar y se los administra el con su propio
//! `Z_Zone`. Un `malloc` general en el kernel habria sido escribir un asignador
//! que ese programa no usa, para que encima lo llame a traves de un syscall.
//!
//! El reparto queda asi, y es el mismo de siempre en este sistema:
//!
//! ```text
//!   el kernel  entrega paginas y dice donde estan
//!   el proceso decide que hace con ellas
//! ```
//!
//! Un `malloc` de C se escribe **encima** de esto en Ring 3, con la politica
//! que quiera cada lenguaje -- y COBOL, Ada o un GC futuro pueden traer otra
//! sin pedirle permiso al kernel.
//!
//! === Lo que NO hay, dicho entero ===
//!
//! - **No se devuelve.** No hay `liberar`. Lo pedido vive hasta que el proceso
//!   muere, y entonces se destruye su espacio de direcciones entero. Para un
//!   programa que pide un bloque al arrancar eso es exactamente lo correcto;
//!   para uno que pida y suelte en un bucle, no -- y por eso hay un tope de
//!   peticiones, para que ese caso falle **pronto y diciendolo** en vez de
//!   comerse la RAM en silencio.
//! - **Contiguo en fisico.** Se piden marcos seguidos porque un bloque que el
//!   programa recorre como un array tiene que serlo. Si la RAM esta
//!   fragmentada y no hay hueco, se rechaza y **se dice** -- entregar memoria a
//!   trozos sin avisar seria peor.
//! - **Sin tocar los dos syscalls.** Esto es una capability mas y una
//!   operacion mas sobre `CURRENT_TASK`, como el framebuffer o la consola.

use crate::ring0::mm::{self, vmm};
use crate::ring0::obj::cap;

/// Cuanto puede pedir un proceso de una vez.
///
/// 64 MiB: ocho veces lo que pide DOOM, y aun asi un numero que no se puede
/// pedir por accidente. Un tope alto y explicito es mejor que ninguno -- sin
/// el, un `request(-1)` mal calculado se lleva la maquina entera.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Cuantas veces puede pedir un proceso.
///
/// Cuatro. No hay forma de devolver memoria, asi que el numero de peticiones
/// ES el numero de fugas posibles. Con esto, un programa que pida en un bucle
/// falla a la cuarta vuelta con un motivo, en vez de agotar la RAM y tumbar lo
/// que estuviera corriendo al lado.
pub const MAX_PETICIONES: usize = 4;

/// Donde empieza el bloque. Espejo de `bmo_abi::...::MEM_OP_BASE`.
pub const MEM_OP_BASE: u64 = 0x01;
/// Cuantos bytes se le han entregado a este proceso.
pub const MEM_OP_BYTES: u64 = 0x02;

/// **La direccion FISICA del bloque.** Pieza S2 del suelo de Ring 3.
///
/// # Para que sirve, y para que NO
///
/// Un driver de Ring 3 que quiera hablar por DMA tiene que escribir en un
/// descriptor la direccion **que ve la tarjeta**, y la tarjeta no pasa por la
/// MMU: ve fisicas. Sin esto no hay forma de construir un anillo de recepcion,
/// que es literalmente el paso 2 de `docs/maestro/RED_MAESTRO.md`.
///
/// ** Y no vale para nada mas. Una fisica es UN NUMERO: solo es peligrosa si
/// algo la acepta como orden, y en este sistema lo unico que lo haria es un
/// aparato haciendo DMA -- que es un problema que ya existe y no uno nuevo. Ver
/// la parte 4 de `docs/plan/PLAN_SUELO_RING3.md`.
///
/// # *** LAS DOS COSAS QUE HACEN QUE ESTE NUMERO NO SEA UNA MENTIRA
///
/// 1. **El bloque es CONTIGUO.** Sale de `alloc_frames_contig`, asi que la
///    fisica del primer marco mas un desplazamiento es la fisica de ese
///    desplazamiento. Si los marcos estuvieran sueltos, contestar "la fisica del
///    bloque" seria cierto para la primera pagina y falso para el resto -- y el
///    fallo lo pagaria la tarjeta escribiendo en memoria de otro.
///
/// 2. **No se mueve.** Hoy nada mueve un marco despues de asignarlo: no hay
///    intercambio a disco, ni compactacion, ni paginas grandes que se partan.
///    ** Y por eso justamente se escribe: la promesa cuesta CERO hoy, y el dia
///    que alguien anada cualquiera de esas tres, esta linea es lo que le dice
///    que hay un contrato que respetar. Una propiedad verdadera por accidente
///    deja de serlo sin que nadie lo note.
///
/// [!] Solo la contesta el DUENO del bloque, porque es una operacion sobre su
/// propia capability. Saber donde vive la memoria del vecino no le hace falta a
/// nadie.
pub const MEM_OP_FISICA: u64 = 0x04;

pub const ERROR_TOO_BIG: u32 = 0xE001;
pub const ERROR_NO_RAM: u32 = 0xE002;
pub const ERROR_TOO_MANY: u32 = 0xE003;
/// No queda ranura en la tabla de contabilidad: ya hay [`MAX_PROCS`] procesos
/// con memoria pedida. **Es un motivo distinto de `ERROR_TOO_MANY`** y por eso
/// tiene codigo propio -- uno dice "tu has pedido demasiado", el otro "el sistema
/// esta lleno", y confundirlos manda a buscar el fallo al programa equivocado.
pub const ERROR_NO_SLOT: u32 = 0xE004;

/// Cuantos procesos pueden tener memoria pedida **a la vez**.
///
/// * Y "a la vez" es la correccion entera. Esto era el tope de *pids* -- la
/// tabla se indexaba con `pid as usize` y se rechazaba `pid >= 16`. Pero el pid
/// es un **contador que solo sube y nunca se reutiliza** (`proc::next_pid`), asi
/// que a partir del programa numero 16 de un arranque **ningun proceso podia
/// volver a pedir memoria jamas**, y encima con el motivo equivocado:
/// `ERROR_TOO_MANY`, que quiere decir "has pedido demasiadas veces".
///
/// No era teorico: en la foto del 2026-07-30, `info` decia **17 lanzados** en
/// una sola sesion.
///
/// Es el **patron 17** otra vez --indexar algo VIVO con un contador HISTORICO--,
/// el mismo que dejo la maquina sin poder lanzar nada cuando `has_room()`
/// miraba una bitacora de ocho entries. Ahora la tabla es de ranuras, se busca
/// por pid y **se libera al morir el proceso**: el tope vuelve a ser lo que
/// dice ser, un limite de recursos concurrentes.
const MAX_PROCS: usize = 16;

/// **Un bloque entregado: donde lo ve el proceso y donde esta de verdad.**
///
/// === Por que se guarda la fisica, si nadie la pedia ===
///
/// Porque el kernel **acaba de reservarla** y tirarla es tirar una respuesta
/// segura. El caso que la pide es leer un fichero dentro de este bloque: el HBA
/// no sabe lo que es una direccion virtual, asi que sin este dato la unica
/// forma de escribir aqui es rebotar por una pagina del kernel y copiar.
///
/// La alternativa era **preguntarle a las tablas de pagina** donde vive el
/// bloque. `dev/disk.rs` ya recorrio ese camino y volvio: una lectura que sale
/// bien o mal segun donde pongas el destino no tiene el fallo en el disco, lo
/// tiene en la traduccion. Aqui no hay nada que traducir -- `alloc_frames_contig`
/// devolvio esta direccion hace tres lineas, y los marcos son **contiguos por
/// construccion**, que es justo lo que un PRDT de una entrada necesita.
#[derive(Clone, Copy)]
struct Bloque {
    /// La VA con la que se le entrego al proceso. `0` = ranura sin usar.
    base: u64,
    /// El primer marco fisico. Los `bytes` siguientes van seguidos.
    fisica: u64,
    bytes: u64,
}

const SIN_BLOQUE: Bloque = Bloque { base: 0, fisica: 0, bytes: 0 };

/// La contabilidad de un proceso que tiene memoria pedida.
#[derive(Clone, Copy)]
struct Count {
    /// `0` = ranura libre. El pid 0 no existe: `NEXT_PID` empieza en 1.
    pid: u32,
    /// Donde va el proximo bloque. Empieza en [`vmm::MEMORIA_VA_BASE`] y
    /// avanza; asi dos peticiones del mismo proceso no se pisan.
    cursor: u64,
    peticiones: usize,
    /// Bytes entregados. Para el panel: la memoria que un proceso pidio es la
    /// unica que el kernel no puede deducir mirando su imagen.
    entregados: u64,
    /// Los bloques vivos de este proceso. Son [`MAX_PETICIONES`] como mucho
    /// porque ese es el tope de peticiones, asi que la tabla no puede
    /// desbordarse por definicion.
    bloques: [Bloque; MAX_PETICIONES],
}

const FREE_SLOT: Count = Count {
    pid: 0,
    cursor: 0,
    peticiones: 0,
    entregados: 0,
    bloques: [SIN_BLOQUE; MAX_PETICIONES],
};

static mut CUENTAS: [Count; MAX_PROCS] = [FREE_SLOT; MAX_PROCS];

/// Total entregado desde el arranque, para `info`. **No baja al morir un
/// proceso**, y es a proposito: es "cuanta memoria ha pedido Ring 3 en esta
/// sesion", no "cuanta hay pedida ahora". Un contador historico dicho como tal
/// no engana a nadie; el problema es usarlo como indice.
static mut TOTAL: u64 = 0;

/// La ranura de este proceso, si tiene una.
fn slot(pid: u32) -> Option<usize> {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().position(|c| c.pid == pid && pid != 0)
    }
}

/// La ranura de este proceso, tomando una libre si aun no tiene.
fn slot_or_new(pid: u32) -> Option<usize> {
    if pid == 0 {
        return None;
    }
    if let Some(i) = slot(pid) {
        return Some(i);
    }
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(CUENTAS);
        let i = t.iter().position(|c| c.pid == 0)?;
        t[i] = Count { pid, ..FREE_SLOT };
        Some(i)
    }
}

pub fn handed_over_by(pid: u32) -> u64 {
    match slot(pid) {
        Some(i) => unsafe { (*core::ptr::addr_of!(CUENTAS))[i].entregados },
        None => 0,
    }
}

/// Cuantos procesos tienen memoria pedida ahora mismo. Para el panel y para
/// que "no queda ranura" se pueda distinguir de "has pedido demasiadas veces".
pub fn processes_with_memory() -> usize {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter().filter(|c| c.pid != 0).count()
    }
}

pub fn total_handed_over() -> u64 {
    unsafe { TOTAL }
}

/// **La ranura `n` de las ocupadas: `(pid, bytes, peticiones)`.**
///
/// `None` cuando ya no hay mas. `n` cuenta **solo las ocupadas**, no los huecos:
/// quien enumera pide 0, 1, 2... y para cuando le contestan `None`, sin tener
/// que saber que la tabla tiene agujeros dentro.
///
/// # Por que hacia falta, y es lo que pidio el dueno
///
/// Los datos ya estaban: `handed_over_by(pid)` contesta desde julio. Lo que no
/// habia era forma de preguntarlos **sin saber el pid de antemano** -- o sea que
/// se podia contestar *"cuanto come el proceso 4"* y no *"quien esta comiendo"*.
///
/// Y esa segunda es la que hace falta para una vista tipo administrador de
/// tareas: la gracia no es mirar a un sospechoso, es **descubrir cual lo es**.
///
/// * Se enumera aqui y no en Ring 3 con una lista de pids porque la tabla es del
/// kernel y cambia sola: un proceso puede morir entre la pregunta y la
/// respuesta. Contestando por indice, lo peor que pasa es que una fila salga
/// vacia un fotograma.
pub fn ranura(n: usize) -> Option<(u32, u64, usize)> {
    unsafe {
        let t = &*core::ptr::addr_of!(CUENTAS);
        t.iter()
            .filter(|c| c.pid != 0)
            .nth(n)
            .map(|c| (c.pid, c.entregados, c.peticiones))
    }
}

/// **Pide `bytes` de memoria.** Devuelve el handle de la capability; la
/// direccion se pregunta despues con `MEM_OP_BASE`.
///
/// `aspace` es el espacio de direcciones del llamante -- durante un syscall
/// desde Ring 3, CR3 **sigue siendo el suyo**: el cambio solo ocurre en un
/// cambio de contexto, y aqui todavia no ha habido ninguno. Es la misma nota
/// que lleva el framebuffer, y por el mismo motivo.
pub fn request(pid: u32, aspace: u64, bytes: u64) -> Result<u64, u32> {
    if bytes == 0 || bytes > MAX_BYTES {
        return Err(ERROR_TOO_BIG);
    }
    // La ranura se toma AQUI, despues de validar el tamano: una peticion
    // absurda no debe gastar una ranura de la tabla. Sin ranura libre el motivo
    // es otro y se dice con su nombre -- antes esto contestaba
    // `ERROR_TOO_MANY` a un proceso que no habia pedido nunca nada.
    let slot = match slot_or_new(pid) {
        Some(s) => s,
        None => {
            crate::ring0::cabina::warn(
                "mem",
                "no queda ranura de contabilidad para otro proceso",
                MAX_PROCS as u64,
            );
            return Err(ERROR_NO_SLOT);
        }
    };
    unsafe {
        if (*core::ptr::addr_of!(CUENTAS))[slot].peticiones >= MAX_PETICIONES {
            return Err(ERROR_TOO_MANY);
        }
    }

    let paginas = (bytes + mm::PAGE - 1) / mm::PAGE;
    // Contiguo: ver la cabecera. Un bloque que el programa recorre como un
    // array no puede llegarle a trozos.
    let fisica = match mm::phys::alloc_frames_contig(paginas) {
        Some(f) => f,
        None => {
            crate::ring0::cabina::warn("mem", "sin RAM contigua para la peticion", bytes);
            return Err(ERROR_NO_RAM);
        }
    };

    let base = unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        if c.cursor == 0 {
            c.cursor = vmm::MEMORIA_VA_BASE;
        }
        c.cursor
    };

    let mut off = 0u64;
    while off < paginas * mm::PAGE {
        if vmm::map_page(aspace, base + off, fisica + off, true, true).is_err() {
            // Mapeo a medias = paginas sueltas en el espacio del usuario. Se
            // deshace lo hecho: quedarse con la mitad mapeada y sin handle es
            // peor que no tener nada. Mismo criterio que el framebuffer.
            let mut undo = 0u64;
            while undo < off {
                vmm::unmap_page(aspace, base + undo);
                undo += mm::PAGE;
            }
            return Err(ERROR_NO_RAM);
        }
        off += mm::PAGE;
    }

    let handle = match cap::grant(
        pid,
        cap::KIND_MEMORIA,
        cap::RIGHT_READ | cap::RIGHT_WRITE,
        base,
    ) {
        Some(h) => h,
        None => {
            let mut undo = 0u64;
            while undo < paginas * mm::PAGE {
                vmm::unmap_page(aspace, base + undo);
                undo += mm::PAGE;
            }
            return Err(cap::ERROR_PERMISSION_DENIED);
        }
    };

    unsafe {
        let c = &mut (*core::ptr::addr_of_mut!(CUENTAS))[slot];
        c.cursor = base + paginas * mm::PAGE;
        // El bloque se apunta ANTES de subir el contador de peticiones, que es
        // lo que hace que el indice sea siempre uno libre.
        if c.peticiones < MAX_PETICIONES {
            c.bloques[c.peticiones] = Bloque { base, fisica, bytes: paginas * mm::PAGE };
        }
        c.peticiones += 1;
        c.entregados += paginas * mm::PAGE;
        TOTAL += paginas * mm::PAGE;
    }
    crate::ring0::cabina::info("mem", "bloque entregado a Ring 3", paginas * mm::PAGE);
    Ok(handle)
}

/// El proceso murio: **se devuelven sus marcos y su ranura queda libre.**
///
/// === La fuga, y como se encontro ===
///
/// Esta funcion soltaba la RANURA y nada mas, con un comentario que decia *"no
/// se desmapea nada -- el espacio de direcciones entero se destruye con el
/// proceso"*. **Esa frase describia una intencion que no implementaba ningun
/// codigo**: `vmm::destroy_address_space` solo se llama desde `vmm::self_test`,
/// y `scheduler::reap` libera unicamente la pila de kernel. O sea que los
/// marcos entregados a Ring 3 **no volvian jamas**.
///
/// Para DOOM eso son 12 MiB por cada vez que se lanza. Lo vio el dueno mirando
/// `mem` en el Ryzen el 2026-08-14 --*"vi que DOOM.bex esta comiendo RAM"*-- y
/// no lo vio ningun contador nuestro, porque **el que dice `fugas 0` cuenta
/// CAPABILITIES, no marcos**. La autopsia decia `recursos todo devuelto` y era
/// verdad: de los handles. La RAM no la miraba nadie.
///
/// === Por que se liberan SOLO estos bloques ===
///
/// La tentacion es recorrer las tablas de paginas y soltar todas las hojas.
/// Seria peor que la fuga: ahi dentro estan **el framebuffer** (que es MMIO, y
/// devolverlo al asignador de RAM es corrupcion) y **los marcos prestados**, que
/// por diseno sobreviven al que los presto. Lo que si es inequivocamente
/// nuestro es esto: bloques que salieron de `alloc_frames_contig` tres lineas
/// mas arriba, con su fisica apuntada, y de un solo dueno. El resto --imagen,
/// pila de usuario, tablas de paginas-- sigue sin devolverse y **eso es deuda
/// declarada**, no un descuido.
///
/// === Y SE PONEN A CERO, que no es opcional ===
///
/// `alloc_frames_contig` **no limpia**. Mientras los marcos no se reutilizaban
/// eso no tenia consecuencias; en cuanto se reutilizan, el siguiente programa
/// que pida memoria recibe **lo que habia dentro del anterior**. La fuga tapaba
/// un agujero de confidencialidad, asi que arreglar una sin la otra habria
/// cambiado un desperdicio por una filtracion. Se limpia aqui --al soltar, que
/// es cuando se sabe de quien era-- y no al pedir.
///
/// [!] Se libera sin desmapear, y se puede: esto corre dentro de
/// `cap::revoke_all`, o sea en el borde del syscall de un proceso que **no
/// vuelve a ejecutar ni una instruccion**. No hay reprogramacion entre el
/// `free_frame` y el cambio de contexto, asi que nadie puede tomar el marco
/// mientras su mapeo muerto sigue en pie.
///
/// [!] Y aqui **NO hace falta la danza de CR3** que si necesita
/// `fb::process_died`, aunque las dos corran en el mismo sitio. Aquella pinta
/// en el framebuffer, que vive a ~3,5 GiB en la mitad BAJA y no esta mapeado
/// bajo el CR3 del moribundo. `zero_frame` escribe por `phys_to_virt`, que es
/// `phys + HIGH_MEM_BASE`: el physmap vive en la mitad ALTA (indices 256..512
/// del PML4) y `vmm::new_address_space` la copia entera en **todo** espacio de
/// direcciones -- por eso los syscalls funcionan bajo el CR3 del usuario.
/// Comprobado antes de escribir esto, no supuesto: es la mina que ya costo dos
/// sesiones y no se pisa dos veces.
pub fn process_died(pid: u32) {
    let Some(slot) = slot(pid) else { return };
    let mut devueltos = 0u64;
    let mut retenidos = 0u64;
    unsafe {
        let bloques = (*core::ptr::addr_of!(CUENTAS))[slot].bloques;
        for b in bloques.iter() {
            if b.base == 0 || b.bytes == 0 {
                continue;
            }
            // ** LA PREGUNTA QUE EVITA LLEVARSE EL ESCRITORIO POR DELANTE.
            if crate::ring0::obj::loan::hay_prestado_en(pid, b.base, b.bytes) {
                retenidos += b.bytes;
                continue;
            }
            let paginas = b.bytes / mm::PAGE;
            for p in 0..paginas {
                let marco = b.fisica + p * mm::PAGE;
                mm::phys::zero_frame(marco);
                mm::phys::free_frame(marco);
            }
            devueltos += b.bytes;
        }
        (*core::ptr::addr_of_mut!(CUENTAS))[slot] = FREE_SLOT;
    }
    if devueltos > 0 {
        // ** DECIA "marcos" Y CONTABA BYTES, Y ADEMAS EN HEXADECIMAL.
        //
        // En el Ryzen del 25-08 salio `marcos devueltos al morir el pid
        // =870000`. Ochocientos setenta mil marcos serian 3,4 GiB; son
        // 0x870000 BYTES, o sea 8,4 MiB, que es lo que gasta la calculadora.
        // La palabra decia una unidad y el numero traia otra, en una base que
        // no se anuncia. `bytes` es el mismo dato dicho como lo que es.
        crate::ring0::cabina::bytes("mem", "memoria devuelta al morir el pid", devueltos);
    }
    // ** Y si algo NO se pudo devolver, se dice. Un bloque retenido es correcto
    // --lo sostiene el que lo tomo prestado-- pero es RAM que sigue fuera, y una
    // retencion que nadie suelta se ve como una fuga tres arranques despues.
    if retenidos > 0 {
        crate::ring0::cabina::warn("mem", "no devuelto: sigue PRESTADO a otro", retenidos);
    }
}

/// **Donde esta DE VERDAD el rango `[va, va + len)` de un bloque de `pid`.**
///
/// `None` si ese rango no cae entero dentro de un solo bloque entregado a ese
/// proceso -- y entonces quien pregunta hace lo de siempre, que es correcto y
/// mas lento. Nunca se contesta "casi": media respuesta aqui es el HBA
/// escribiendo en memoria de otro.
///
/// Lo usa `syscall.rs` para que **el disco escriba dentro del bloque del
/// programa sin escala**: es el escalon 3 de `docs/identidad/LA_RAM.md` aplicado a leer
/// ficheros, y la razon por la que [`Bloque`] guarda la fisica.
/// **Cuantos bytes mide ESTE bloque**, el que empieza en `base`. `None` si ese
/// proceso no tiene ninguno que empiece ahi.
///
/// # *** POR QUE EXISTE, Y ES UN FALLO REAL QUE ESTABA VIVO (2026-08-24)
///
/// El limite de `ARCH_OP_LEER_EN` y `ARCH_OP_ESCRIBIR_DE` se comprobaba contra
/// [`handed_over_by`], que es **la suma de todos los bloques del proceso**:
///
/// ```text
///    bloque A   4 KiB      entregados = 8192
///    bloque B   4 KiB
///
///    leer_en(handle_de_A, desde=0, cuantos=8192)   ->  8192 <= 8192, PASA
///    y el kernel escribe 8 KiB empezando en la base de A
/// ```
///
/// ** Cuatro mil noventa y seis bytes fuera del bloque que el handle autoriza.
/// El camino rapido no se traga esto --[`fisica_de`] valida bloque a bloque--
/// pero el de respaldo escribe en la VA del proceso, y ahi no habia limite.
///
/// [!] Y lo peor no es que el proceso se corrompa a si mismo: si el rango cae en
/// VA **sin mapear**, quien falla es el KERNEL, porque durante una syscall el
/// kernel escribe con las tablas del llamante. Una app sin privilegios podia
/// tumbar la maquina con dos numeros.
///
/// *** Es exactamente lo que la doctrina de `syscall/ops.rs` queria impedir --
/// *"aqui no hay `copy_from_user`"*-- y se colo igual, no por un puntero sino
/// por una **pareja desplazamiento+largo** comprobada contra el total
/// equivocado. Un limite es tan bueno como el numero con el que se compara.
pub fn bytes_de_bloque(pid: u32, base: u64) -> Option<u64> {
    let slot = slot(pid)?;
    unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        for b in c.bloques.iter() {
            if b.base != 0 && b.base == base {
                return Some(b.bytes);
            }
        }
    }
    None
}

pub fn fisica_de(pid: u32, va: u64, len: u64) -> Option<u64> {
    let slot = slot(pid)?;
    let fin = va.checked_add(len)?;
    unsafe {
        let c = &(*core::ptr::addr_of!(CUENTAS))[slot];
        for b in c.bloques.iter() {
            if b.base != 0 && va >= b.base && fin <= b.base + b.bytes {
                return Some(b.fisica + (va - b.base));
            }
        }
    }
    None
}

/// Las operaciones sobre el handle. `base` es la VA con la que se concedio.
pub fn operation(base: u64, operation: u64, pid: u32) -> Option<u64> {
    match operation {
        MEM_OP_BASE => Some(base),
        MEM_OP_BYTES => Some(handed_over_by(pid)),
        // ** `None` y no `Some(0)` si no se encuentra el bloque: cero es una
        // fisica valida --el primer marco de la maquina-- asi que devolverlo
        // como "no lo se" seria mandar a un driver a programar un DMA contra el
        // vector de interrupciones. `None` sale por la puerta como "esa
        // operacion no existe aqui", que es lo que de verdad pasa.
        // ** SE REUSA `fisica_de`, que ya existia para el prestamo del audio.
        //
        // Y trae de regalo lo unico que hacia falta y no se ve: comprueba que
        // `[va, va+len)` **cae dentro del bloque**. Una version escrita a mano
        // aqui habria devuelto la fisica del primer marco sin mirar el largo, y
        // eso es correcto para una pagina y falso para un anillo de DMA.
        //
        // `len = 1` porque lo que se pregunta es "donde empieza": el largo del
        // bloque ya lo contesta `MEM_OP_BYTES`, y pedir dos numeros por dos
        // caminos distintos es como se acaban desacoplando.
        MEM_OP_FISICA => fisica_de(pid, base, 1),
        _ => None,
    }
}
