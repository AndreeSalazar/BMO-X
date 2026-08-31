//! **CARRIL AMARILLO** -- va a cambiar, y al cambiar ARRASTRA A OTRO.
//!
//! [carril]  AMARILLO  el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- escribe entradas de tabla de paginas. Un bit de mas
//!           en `map_page_tipo` es una ventana, no un fallo.
//!
//! [riesgo]  ESPEJO -- los tres envoltorios (`map_page`, `map_page_propia`,
//!           `map_page_wc`) son la MISMA decision escrita tres veces sobre
//!           `map_page_tipo`. Tocar uno sin los otros es como se separan.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # *** POR QUE ESTO ES AMARILLO Y NO ROJO: EL CAMBIO YA ESTA ESCRITO
//!
//! No es que pueda cambiar. **Esta pendiente, con nombre y con cuenta**, en la
//! cabecera de `PTE_NX` de aqui abajo:
//!
//! > *"Hacen falta TRES estados, y eso es un parametro mas en `map_page_tipo` y
//! > en sus cuatro llamantes. No es dificil; es que no se puede hacer a medias."*
//!
//! Ese es el carril amarillo entero: una pieza que **se sabe que se va a tocar**
//! y que **arrastra a cuatro sitios** cuando se toque. Sin este fichero, quien
//! venga a cerrar `rodata` tendria que descubrir los cuatro por su cuenta -- y
//! la cabecera de `PTE_NX` avisa de que el arreglo obvio abre un agujero peor
//! que el que cierra.

use super::roja::{get_or_create, table};
use super::verde::{ADDR_MASK, PTE_HUGE, PTE_PRESENT, PTE_USER, PTE_WRITABLE};
use super::super::{phys, phys_to_virt, PAGE, PHYSMAP_SIZE};
use bmo_fisica_juicio::se_puede_caminar;

/// **Esta hoja es NUESTRA: liberarla al destruir el espacio de direcciones.**
///
/// El bit 9 es uno de los tres que la arquitectura deja libres al sistema
/// operativo (9, 10 y 11) -- el hardware no lo mira nunca.
///
/// # Por que un bit y no una lista
///
/// Al morir un proceso hay que devolver sus marcos, y hasta el 14-08 no se
/// devolvia ninguno: la imagen, la pila de usuario y las tablas se quedaban
/// puestas para siempre. La pregunta que lo bloqueaba no era "donde estan" sino
/// **"cuales son mios"** -- porque en el mismo espacio conviven el framebuffer
/// (que es MMIO: devolverlo al asignador de RAM es corrupcion) y los marcos
/// prestados, que por diseno sobreviven al que los presto.
///
/// La salida obvia era llevar una lista por proceso. Pero **la tabla de paginas
/// YA ES esa lista**: tiene todos los marcos, uno por entrada, mantenida por el
/// hardware y sin poder desincronizarse. Lo unico que le faltaba era una columna
/// que dijera de quien es cada uno. Es una columna mas en una tabla que ya
/// existe, no una estructura nueva que mantener en dos sitios.
///
/// # Y el valor por defecto es NO, a proposito
///
/// [`map_page`] no lo pone; hay que pedirlo con [`map_page_propia`]. O sea que
/// olvidarse de marcar algo **fuga un marco**, y marcar de mas **lo libera dos
/// veces**. Lo primero se ve en `mem` y se arregla; lo segundo entrega memoria
/// viva a otro programa y el fallo aparece tres arranques despues y en otro
/// sitio. La duda se resuelve por el lado que solo cuesta RAM.
pub const PTE_NUESTRA: u64 = 1 << 9;

/// * En una PTE de 4 KiB el bit 7 **no** es "pagina grande": es el bit alto
/// del indice de PAT. Con `PWT`(3) y `PCD`(4) a cero, ponerlo selecciona la
/// entrada **4** de la tabla -- la que `s1_cpu` deja en Write-Combining.
///
/// El mismo numero significa dos cosas distintas segun el nivel de tabla, y
/// por eso lleva nombre propio: en una PDE seria `PS` y convertiria la entrada
/// en una pagina de 2 MiB.
pub const PTE_PAT_4K: u64 = 1 << 7;

/// **El bit 63: esta pagina NO se ejecuta.**
///
/// # *** W^X, Y AQUI NO HACE FALTA NI UN PARAMETRO NUEVO (2026-08-24)
///
/// La regla se llama `W^X` --escribible O ejecutable, nunca las dos-- y esa
/// frase es literalmente la condicion que ya se pasa a esta funcion:
///
/// ```text
///    map_page(.., writable = true)   -> datos, pila, canal, framebuffer
///    map_page(.., writable = false)  -> el codigo del .bex, y solo el
/// ```
///
/// ** El cargador ya lo calculaba: `writable = flags & SECTION_FLAG_EXEC == 0`.
/// O sea que la informacion de que es codigo lleva ahi desde el principio, y
/// solo faltaba escribir el otro lado de la moneda en la tabla de paginas.
///
/// *** LO QUE COMPRA, Y ES LA MITAD DE UNA EXPLOTACION: sin esto, quien
/// consiga escribir en cualquier sitio escribe instrucciones y salta a ellas.
/// Con esto tiene que construir la cadena con trozos de codigo que YA existen
/// --ROP-- que es un trabajo de otro orden de magnitud.
///
/// [!] Y el codigo se escribe POR EL PHYSMAP, no por la VA del proceso
/// (`admitir.rs` usa `phys_to_virt`), asi que mapear la seccion de codigo sin
/// permiso de escritura no estorba a cargarla. Esa decision ya estaba tomada.
///
/// # [!!] LA TRAMPA PARA EL QUE VENGA A CERRAR `rodata`
///
/// Hoy hay DOS estados y por eso basta un `bool`: escribible-y-no-ejecutable, o
/// ejecutable-y-no-escribible. `rodata` cae en el primero -- **se mapea
/// escribible**, que no es correcto pero como mucho deja que un programa
/// corrompa sus propias constantes; no cruza ninguna frontera.
///
/// *** Y quien vaya a arreglarlo tiene que saber esto ANTES de tocarlo:
///
/// ```text
///    "rodata no deberia ser escribible"   ->  writable = false
///    y con la regla de aqui, eso lo vuelve EJECUTABLE
/// ```
///
/// ** O sea que el arreglo obvio abre un agujero peor que el que cierra: una
/// region de datos que el programa controla y que ademas se puede ejecutar es
/// exactamente lo que W^X existe para impedir.
///
/// Hacen falta TRES estados, y eso es un parametro mas en `map_page_tipo` y en
/// sus cuatro llamantes. No es dificil; es que no se puede hacer a medias.
pub const PTE_NX: u64 = 1 << 63;

/// collision (which would mean the VA overlaps the kernel identity map).
pub fn map_page(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, false, false)
}

/// Igual, pero declarando que **el marco es de este espacio de direcciones** y
/// hay que devolverlo cuando el espacio se destruya. Ver [`PTE_NUESTRA`].
///
/// Se pide con esto lo que sale de `phys::alloc_frame` para un proceso concreto
/// y no lo sabe nadie mas: **la imagen y la pila de usuario**. No se pide para:
///
/// - el **framebuffer**, que es MMIO y no salio del asignador de RAM;
/// - lo **prestado**, que sobrevive al que lo presto por diseno;
/// - los bloques de `KIND_MEMORIA`, que tienen dueno explicito -- `obj::memory`
///   los ficha con su fisica y los libera el mismo, **y ademas pregunta antes
///   si estan prestados**. Marcarlos aqui seria liberarlos dos veces y saltarse
///   esa pregunta.
pub fn map_page_propia(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, false, true)
}

/// Igual, pero eligiendo **Write-Combining** para esta pagina.
///
/// Se usa para el framebuffer y nada mas: es donde se escriben millones de
/// pixeles seguidos y donde juntar las escrituras cambia el orden de magnitud.
/// Para memoria normal seria lo contrario de lo que se quiere -- WC no garantiza
/// el orden de las escrituras, y eso en una estructura de datos es un bug.
pub fn map_page_wc(pml4: u64, va: u64, pa: u64, user: bool, writable: bool) -> Result<(), ()> {
    map_page_tipo(pml4, va, pa, user, writable, true, false)
}

/// **Se puede usar el bit NX?** Se lee `EFER.NXE` una vez y se recuerda.
///
/// ** Si sale que NO, se dice por CABINA con gravedad de fallo y se deja de
/// marcar. Degradar en silencio seria lo peor de las dos opciones: ni protege
/// ni se entera nadie -- y este arbol ya tiene escrito lo que pasa con eso en

/// `bmo_cripto::azar`, que por lo mismo se niega a tener respaldo.
pub(super) fn nx_disponible() -> bool {
    use core::sync::atomic::{AtomicU8, Ordering};
    static ESTADO: AtomicU8 = AtomicU8::new(0); // 0 sin mirar, 1 si, 2 no
    match ESTADO.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    // `IA32_EFER` = 0xC000_0080, y el bit 11 es NXE.
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi,
                         options(nomem, nostack));
    }
    let _ = hi;
    let hay = lo & (1 << 11) != 0;
    if hay {
        ESTADO.store(1, Ordering::Relaxed);
    } else {
        ESTADO.store(2, Ordering::Relaxed);
        crate::ring0::cabina::fault(
            "mm",
            "EFER.NXE APAGADO: W^X no se puede aplicar y toda pagina sera ejecutable",
            lo as u64,
        );
    }
    hay
}


pub(super) fn map_page_tipo(
    pml4: u64,
    va: u64,
    pa: u64,
    user: bool,
    writable: bool,
    combinar_escrituras: bool,
    nuestra: bool,
) -> Result<(), ()> {
    if va % PAGE != 0 || pa % PAGE != 0 {
        return Err(());
    }
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;
    let mid = PTE_PRESENT | PTE_WRITABLE | if user { PTE_USER } else { 0 };

    let pt_phys = {
        let p = table(pml4);
        let pdpt_phys = get_or_create(p, i4, mid)?;
        let pdpt = table(pdpt_phys);
        let pd_phys = get_or_create(pdpt, i3, mid)?;
        let pd = table(pd_phys);
        get_or_create(pd, i2, mid)?
    };
    let pt = table(pt_phys);

    let mut entry = (pa & ADDR_MASK) | PTE_PRESENT;
    if writable {
        entry |= PTE_WRITABLE;
    }
    if user {
        entry |= PTE_USER;
    }
    if combinar_escrituras {
        entry |= PTE_PAT_4K;
    }
    if nuestra {
        entry |= PTE_NUESTRA;
    }
    // *** W^X. Ver `PTE_NX`: escribible y ejecutable son excluyentes, y la
    // condicion ya venia calculada desde el cargador.
    //
    // [!] Se pregunta si `EFER.NXE` esta puesto, y no se supone. Con NXE en
    // cero el bit 63 es RESERVADO: cada pagina marcada asi daria `#PF` por bit
    // reservado y **no arrancaria nada**. `s1_cpu` lo enciende, asi que esto
    // tendria que ser siempre cierto -- razon de mas para preguntarlo, porque
    // lo que "tendria que ser siempre cierto" es lo que nadie mira el dia que
    // deja de serlo. Si no esta, `nx_disponible()` lo GRITA por CABINA.
    if writable && nx_disponible() {
        entry |= PTE_NX;
    }
    let old = pt[i1];
    pt[i1] = entry;
    if old & PTE_PRESENT != 0 {
        unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    }
    Ok(())
}

/// Remove a mapping. Returns the physical address that was mapped, if any.

/// Does not free table frames (they are recycled by the address space).
pub fn unmap_page(pml4: u64, va: u64) -> Option<u64> {
    let i4 = ((va >> 39) & 0x1FF) as usize;
    let i3 = ((va >> 30) & 0x1FF) as usize;
    let i2 = ((va >> 21) & 0x1FF) as usize;
    let i1 = ((va >> 12) & 0x1FF) as usize;

    let p = table(pml4);
    let e = p[i4];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pdpt = table(e & ADDR_MASK);
    let e = pdpt[i3];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pd = table(e & ADDR_MASK);
    let e = pd[i2];
    if e & PTE_PRESENT == 0 || e & PTE_HUGE != 0 {
        return None;
    }
    let pt = table(e & ADDR_MASK);
    let e = pt[i1];
    if e & PTE_PRESENT == 0 {
        return None;
    }
    pt[i1] = 0;
    unsafe { core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack)) };
    Some(e & ADDR_MASK)
}

/* -- ** VUELVE A CASA (2026-08-31) ------------------------------------------
 *
 * Esto vivio dos dias en `ring0/critic/amarilla.rs`, una carpeta GLOBAL que fue
 * mi primera lectura --equivocada-- de L6g. El dueno la corrigio dos veces y la
 * segunda fue con el nombre: *"no me gusta esa palabra ahi"*.
 *
 * *** Y tenia razon por debajo del nombre. `amarilla` es el color de un carril,
 * y **un color solo significa algo DENTRO de un modulo**: amarilla respecto a
 * que? Una carpeta global con nombre de carril es la senal ilegible justo en el
 * sitio donde la senal era el objetivo.
 *
 * ** Lo que sujetaba a las dos juntas ya no hace falta que las junte. Estaban
 * en el mismo fichero porque cada una tenia SU techo y divergieron --16 GiB
 * contra 64 TiB, dos pantallas azules el 30-08--. Hoy las dos preguntan a
 * `bmo-fisica-juicio`, que **no tiene ni una constante de tamano**: el espejo se
 * le pasa en cada llamada. No hay dos numeros que mantener de acuerdo, hay uno,
 * y por eso pueden vivir cada una en su casa.
 *
 * [!] Amarilla y no roja: no es que sea peligrosa de ejecutar --rechaza y
 * apunta--, es que **arrastra**. Su gemela es `phys::zero_frame`, en
 * `mm/phys/amarilla.rs`, y el dia que este techo se toque hay que abrir las dos.
 */

/// **Se puede caminar por esta direccion fisica?**
///
/// # *** POR QUE ESTO EXISTE: el #GP del 2026-08-25
///
/// El dueno multiplico en la calculadora, la app murio, y **el kernel murio
/// detras** con un `#GP` en esta funcion:
///
/// ```text
///    vec=0x0D  err=0x00000000
///    rip=0x0000000000410849     <- 313 bytes dentro de destroy_address_space
/// ```
///
/// `err=0` en un `#GP` de Ring 0 dentro de una funcion que solo calcula
/// direcciones significa una cosa: **una direccion NO CANONICA**. Y aqui las
/// unicas direcciones que se calculan salen de `phys_to_virt` sobre valores
/// leidos de las tablas de pagina.
///
/// ## La aritmetica que lo permite, y no es obvia
///
/// ```text
///    ADDR_MASK        cubre 52 bits   -> hasta ~4 PB
///    HIGH_MEM_BASE    0xFFFF_8000_..  -> canonico solo si phys < 2^47
/// ```
///
/// *** **`ADDR_MASK` deja pasar direcciones que el physmap no puede alcanzar.**
/// Una entrada con basura en los bits 48-51 sobrevive a la mascara, se suma a
/// `HIGH_MEM_BASE`, cae en el agujero no canonico, y el procesador para la
/// maquina entera.
///
/// [!] **Y esto NO dice de donde sale la basura.** No se sabe todavia: las siete
/// banderas de este fichero viven en los bits 0-9 y el 63, asi que una entrada
/// bien formada no puede tener nada en el 48. Lo que esto hace es convertir una
/// maquina muerta en **una linea que dice el nivel y el valor** -- que es lo
/// unico que permitira averiguarlo.
///
/// > Un kernel que se cae desmontando a un muerto no deja autopsia: se lleva por
/// > delante al que la iba a escribir.
///
/// # *** Y VOLVIO A MATAR LA MAQUINA EL 2026-08-30. El techo estaba mal.
///
/// Misma funcion, segunda pantalla azul. El dueno abrio DOOM y salio esto:
///
/// ```text
///    vec=0x0E  err=0x00000000   no-presente  leyendo  desde el KERNEL
///    rip=0x00000000004111B5     <- +0x385 dentro de destroy_address_space
///    cr2=0xFFFFBD352B3AC000
///    corria tid=02  (Ring 0)    <- `reap`, el que desmonta al muerto
/// ```
///
/// Y la resta lo dice entero:
///
/// ```text
///    cr2 - HIGH_MEM_BASE  =  0x3D352B3AC000  =  61,2 TiB
///    FISICA_MAX (antes)   =  1 << 46         =  64   TiB   <- LO DEJABA PASAR
///    PHYSMAP_SIZE         =  0x4_0000_0000   =  16   GiB   <- lo que hay
/// ```
///
/// *** **El guardian del 25-08 no cerro el agujero: cambio la excepcion.** Con
/// `2^46` se acaban las direcciones NO CANONICAS --y con ellas el `#GP`-- pero
/// queda abierto todo el tramo de **16 GiB a 64 TiB**, donde la direccion SI es
/// canonica, `phys_to_virt` la calcula sin quejarse, y no la mapea nadie. Eso
/// es un `#PF` de no-presente leyendo desde el kernel, que es exactamente la
/// pantalla de arriba. Un techo 4.096 veces mas alto de lo que existe no es un
/// techo.
///
/// ## Y el numero correcto ya estaba escrito en DOS sitios
///
/// ```text
///    mm/mod.rs      "the allocator MUST never hand out a frame at or above
///                    this address -- the kernel could not touch it through
///                    phys_to_virt"
///    phys.rs        MAX_PHYS = PHYSMAP_SIZE, y `free_frame` YA rechaza con el
/// ```
///
/// ** O sea que `free_frame` y `caminable` juzgan LA MISMA direccion fisica con
/// dos techos distintos --16 GiB y 64 TiB-- y **el flojo era el que
/// dereferencia**. El que solo apunta un bit en un mapa era el estricto.
///
/// [!] Y la frase que esta funcion imprime ya decia el techo bueno desde el
/// primer dia: *"entrada fuera del physmap"*. El mensaje y la comprobacion no
/// hablaban de lo mismo, y gano el mensaje.
// *** Y AQUI YA NO HAY NINGUN TECHO. (2026-08-30, la mudanza)
//
// Habia una constante local --`FISICA_MAX`-- y ESA constante fue el bug: decia
// `1 << 46` donde `PHYSMAP_SIZE` ya existia. Ahora no hay ninguna: la
// comparacion se la hace `bmo-fisica-juicio`, que **no tiene ni un numero de
// tamano propio** --se le pasa el espejo en cada llamada-- y que si se puede
// probar en el anfitrion.
//
// ** Un fichero sin constante de tamano no puede tener una constante de tamano
// mal. Es la regla 3 de L6g cumplida quitando la posibilidad, no vigilandola.

/// ** Y LAS CUATRO FRASES SE ACORTARON EL 2026-08-25, POR UN MOTIVO MEDIDO.
///
/// La primera version decia `una entrada de PD no apunta a memoria alcanzable`:
/// 48 columnas. La bitacora del panel tiene 80, el prefijo --secuencia, tick,
/// severidad, modulo-- gasta 26, y el `=` otras dos. Al numero le quedaban
/// **cuatro digitos de dieciseis**, y esto es lo que se vio en el Ryzen:
///
/// ```text
///    FAULT vmm: una entrada de PD no apunta a memoria alcanzable =1100
/// ```
///
/// *** `1100` no es la entrada: son los cuatro primeros digitos de la entrada.
/// Toda esta funcion existe para decir ESE numero --lo dice su propia cabecera,
/// *"convertir una maquina muerta en una linea que dice el nivel y el valor"*--
/// y la linea llego a la pantalla sin el.
///
/// [!] El reparto de la fila ya esta arreglado donde tenia que estarlo (el valor
/// no cede nunca; ver `cabina/cockpit.rs`), asi que esto no es la reparacion: es
/// no volver a gastar el ancho que ahora se reparte bien. **El nivel va primero
/// en la frase a proposito** -- si algun dia hay que recortar otra vez, lo que
/// sobrevive tiene que ser lo que distingue `PD` de `PDPT`.
pub(crate) fn caminable(
    fisica: u64,
    nivel: &'static str,
    cruda: u64,
    tabla: u64,
    casilla: usize,
) -> bool {
    if se_puede_caminar(fisica, PHYSMAP_SIZE).se_puede() {
        return true;
    }
    crate::ring0::cabina::fault("vmm", nivel, cruda);
    // *** Y LA SEGUNDA LINEA, QUE ES LA QUE DECIDE ENTRE LAS DOS CAUSAS.
    //
    // La entrada cruda dice QUE hay ahi. No dice **donde estaba**, y sin eso las
    // dos explicaciones posibles se ven exactamente igual:
    //
    // ```text
    //    tres casillas malas en LA MISMA tabla   -> ese marco NO es una tabla:
    //                                               se esta leyendo el dato de
    //                                               otro como si fuera un PD
    //    tres casillas malas en TRES tablas      -> las tablas son tablas y lo
    //                                               que esta mal es lo que se
    //                                               escribio en ellas
    // ```
    //
    // ** La primera apunta al ASIGNADOR (un marco entregado dos veces); la
    // segunda al que ESCRIBE las entradas. Son dos ficheros distintos, y hasta
    // hoy habia que elegir a ciegas.
    //
    // Los dos numeros viajan en uno: una tabla esta alineada a 4 KiB, asi que
    // sus doce bits bajos estan a cero, y una casilla de 0..511 cabe en nueve.
    // Empaquetar aqui es lo mismo que hace `pci` con `bus:dev.func` y el MMIO.
    //
    // [!] Y esto refuerza la sospecha que ya hay sobre la mesa: `get_or_create`
    // escribe `fisica | 0x7` (PRESENT|WRITABLE|USER) o `| 0x3` sin usuario. **No
    // hay ningun camino que escriba un `1` pelado**, y el Ryzen enseno dos. Un
    // valor que este fichero no sabe producir no salio de este fichero.
    crate::ring0::cabina::fault("vmm", "y estaba en tabla|casilla", tabla | casilla as u64);
    // *** Y AQUI SE LEVANTA LA PATADA (2026-08-26).
    //
    // Esto no es una app portandose mal: son **las tablas de pagina del kernel
    // diciendo algo imposible**. `get_or_create` escribe `fisica | 0x7` o `| 0x3`
    // y nada mas, asi que un valor que este fichero no sabe producir no salio de
    // este fichero -- y mientras no se sepa de donde sale, seguir dejandole la
    // pantalla a Ring 3 es apostar.
    //
    // ** Solo se APUNTA. Esto corre desde `reap`, o sea con el cerrojo del
    // planificador en la mano y las interrupciones apagadas: hacer el rescate
    // aqui volveria a tomar ese cerrojo y seria un abrazo mortal. Quien lo
    // recoge es el hilo del bus. Ver `core/emergencia.rs`.
    crate::ring0::core::emergencia::declarar(nivel, cruda);
    false
}
