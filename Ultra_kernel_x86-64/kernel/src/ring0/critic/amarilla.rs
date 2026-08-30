//! **CARRIL AMARILLO** -- va a cambiar, y al cambiar ARRASTRA A OTRO.
//!
//! [cuesta]  MAQUINA -- las dos funciones de aqui deciden si el kernel
//!           dereferencia una direccion fisica. Equivocarse para la maquina.
//!
//! [riesgo]  ESPEJO AJENO
//!           ESPEJO -- son DOS y juzgan el mismo numero. El 2026-08-30 no
//!                     coincidian --16 GiB contra 64 TiB-- y **cambiar una sin
//!                     la otra fue el bug**. Por eso comparten fichero.
//!           AJENO  -- el numero que juzgan no lo escribe este fichero: sale
//!                     de una tabla de paginas o de la ranura de una tarea
//!                     muerta.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # *** POR QUE ESTAS DOS ESTAN JUNTAS Y EN AMARILLO
//!
//! Porque no son dos piezas criticas: son **una pieza partida en dos sitios**.
//! Las dos preguntan *"cae esta fisica dentro de lo que el espejo alcanza?"* y
//! las dos actuan segun la respuesta -- una bajando por una tabla de paginas,
//! la otra escribiendo 4 KiB.
//!
//! El 2026-08-30 la maquina se paro dos veces por eso: el techo de una era
//! `1 << 46` y el de la otra `PHYSMAP_SIZE`, y **la floja era la que
//! dereferencia**. Un letrero que dijera *"estas dos van juntas"* habria
//! costado un minuto y ahorrado dos pantallas azules.
//!
//! > No es que sean peligrosas. Es que **no se pueden tocar por separado**, y
//! > eso no estaba escrito en ningun sitio.
//!
//! # ** Y EL TECHO YA NO VIVE AQUI
//!
//! La comparacion se la hace `bmo-fisica-juicio`, que **no tiene ni una
//! constante de tamano**: el espejo se le pasa en cada llamada. Un juez que no
//! puede inventarse el techo no puede equivocarse en el techo -- es la regla 3
//! de L6g cumplida por construccion, y ahi si hay banco de pruebas, con la
//! direccion exacta de la pantalla del 30-08 dentro.

use super::super::mm::{phys, phys_to_virt, PAGE, PHYSMAP_SIZE};
use bmo_fisica_juicio::se_puede_caminar;


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

/// Zero a frame through the physmap.
///
/// # *** LA MISMA COTA QUE `free_frame`, Y AQUI SE ESCRIBE (2026-08-30)
///
/// Esto no tenia ninguna. Y las dos funciones se llaman **una detras de otra**,
/// en cuatro sitios distintos, sobre el mismo numero:
///
/// ```text
///    mm::phys::zero_frame(marco);   <- 4 KiB ESCRITOS, sin comprobar nada
///    mm::phys::free_frame(marco);   <- rechaza >= MAX_PHYS
/// ```
///
/// ** Dos jueces del mismo valor con dos criterios, y el que NO comprobaba era
/// el que escribe. Es la forma exacta del fallo que paro la maquina el mismo
/// dia --`caminable` a 64 TiB contra `free_frame` a 16 GiB-- encontrada a
/// proposito buscando el gemelo. La ley que lo nombra es L6f, clase `ESPEJO`.
///
/// ## Lo que costaria, y son dos cosas distintas
///
/// ```text
///    fuera del physmap    #PF de escritura desde el kernel -> pantalla azul
///    dentro y del vecino  4 KiB de memoria viva a cero, EN SILENCIO
/// ```
///
/// *** La segunda es la peor y es la que no da ninguna pantalla. Por eso la
/// cota va aqui dentro y no en los ocho llamantes: un guardian que hay que
/// acordarse de poner no es un guardian.
///
/// [!] No puede estorbar a nadie: el asignador **no entrega** marcos por encima
/// de `MAX_PHYS`, asi que un `phys` que no pase por aqui no salio de el.
pub fn zero_frame(phys: u64) {
    // ** EL MISMO JUEZ QUE `caminable`, y eso es el fichero entero.
    //
    // El 30-08 esta comparaba contra `MAX_PHYS` y la de arriba contra un
    // `1 << 46` local. Ahora las dos preguntan lo mismo al mismo sitio, asi que
    // **no pueden volver a discrepar**: no hay dos numeros que mantener de
    // acuerdo, hay uno.
    if phys % PAGE != 0 || !se_puede_caminar(phys, PHYSMAP_SIZE).se_puede() {
        crate::ring0::cabina::fault("mm", "zero_frame sobre un marco que no existe", phys);
        return;
    }
    unsafe {
        core::ptr::write_bytes(phys_to_virt(phys) as *mut u8, 0, PAGE as usize);
    }
}
