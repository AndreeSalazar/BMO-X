//! **CARRIL ROJO -- EL VUELO: quien esta escribiendo AHORA MISMO.**
//!
//! [carril]  ROJO      el nibble ALTO del byte: ponerlo, quitarlo y saber de
//!                     quien es
//!
//! [cuesta]  MAQUINA -- un bit mal puesto aqui deja un bufer reasignado
//!           mientras un aparato escribe dentro. No es una tarea perdida: es
//!           un dato ajeno apareciendo en la memoria de otro (L6e)
//!
//! [riesgo]  SILENCIO -- y es LA razon de que este sea el rojo. El marcado de
//!           al lado falla A GRITOS: `vmm::es_tabla` se niega a caminar y lo
//!           dice con nombre. Esto no avisa de nada, no da fault y no tiene
//!           sintoma; por eso las tres cuentas de `vuelos()` (L6f)
//!
//! # [!] ESTE CARRIL YA SE JUGO EN CONTRA UNA VEZ, Y DURO TRES HORAS
//!
//! `aterrizo` nacio pidiendo solo la direccion, asi que le quitaba el vuelo a
//! **quien fuera**. `vivos` habria llegado a cero con un vuelo todavia en el
//! aire: la mentira exacta que este bit existe para no contar. Lo encontro
//! una pregunta del dueno --*"si algo puede jugar en contra, aislar"*-- y no
//! una prueba. El arreglo esta escrito entero en el `///` de `aterrizo`.
//!
//! *** Que el fallo de este carril lo cazara una PREGUNTA en vez de un juez
//! es, el solo, el argumento de que sea el rojo.
//!
//! Las reglas que sostiene: `NEUTRO/DMA/REGLAS.txt`, R-DMA-2, R-DMA-3 y
//! R-DMA-4. La que le falta a la casa --el PLAZO, R-DMA-8-- se cablea aqui.

use super::{indice, tabla, CADUCADOS, EN_VUELO_CHOQUES, EN_VUELO_VIVOS,
            PEOR_SILENCIO, ULTIMA_NOTICIA, VUELOS_DE};


// == LOS APARATOS, NUMERADOS -- y el orden NO es de gusto ==================
//
// ** Son las filas de `NEUTRO/CENSO.txt` en su orden, y esa es toda la regla.
// Un numero que no sale de la lista de quien alcanza la RAM por su cuenta
// seria un aparato que nadie censo escribiendo en la memoria de alguien.
//
// [!] El CERO esta reservado a proposito: significa **nada en vuelo**. Por eso
// se empieza en 1, y por eso `en_vuelo` rechaza el 0 en vez de aceptarlo como
// un aparato mas.

/// El HBA del disco. Fila 1 del censo.
pub const APARATO_AHCI: u8 = 1;
/// La tarjeta de red. Fila 2.
pub const APARATO_NIC: u8 = 2;
/// El controlador USB. Fila 3.
pub const APARATO_XHCI: u8 = 3;
/// La grafica, cuando llegue. Fila 4, hoy sin tarjeta.
pub const APARATO_GPU: u8 = 4;

// == *** EL PLAZO -- EL PASO N5 ============================================
//
// # La octava regla era la unica sin juez, y este es
//
// `NEUTRO/DMA/REGLAS.txt` lo dejo escrito con su hueco a la vista:
//
// ```text
//    R-DMA-8   Todo vuelo tiene PLAZO: ninguno dura para siempre
//              juez    ** NO EXISTE TODAVIA. Es el paso N5
// ```
//
// ** Sin plazo, un aparato que se muere con un descriptor programado deja su
// marco en vuelo **para siempre**: `vivos` no baja, el marco no se puede
// reasignar sin romper R-DMA-3, y el sistema espera a alguien que ya no
// contesta. No es una fuga de memoria: es una fuga que ademas MIENTE, porque
// el contador dice que hay un DMA vivo que no existe.
//
// # *** EL PLAZO ES DEL APARATO, NO DEL MARCO -- y eso es lo que lo hace
// pagable
//
// La primera forma de escribir esto es un sello de tiempo por marco. No cabe:
//
// ```text
//    por MARCO     4.194.304 marcos x 8 bytes  =  32 MiB de BSS
//    por APARATO   15 aparatos x 24 bytes      =  360 bytes
// ```
//
// *** Y no es solo que sea mas barato: **es la pregunta correcta**. "Se murio
// este marco" no significa nada. Lo que se pregunta es *"se murio el DISCO"*, y
// eso es uno por aparato aunque tenga cien marcos en el aire.
//
// # LO QUE SE MIDE ES EL SILENCIO, NO LA DURACION
//
// La segunda forma que no vale es cronometrar desde el despegue. Un aparato
// que va a tope --el disco leyendo un fichero grande-- nunca se queda sin nada
// en el aire, asi que ese cronometro no para nunca y **un plazo lo mataria por
// estar sano**. Es exactamente lo que el dueno pidio buscar: algo que juega en
// contra.
//
// ** Asi que se mide **cuanto lleva callado teniendo trabajo pendiente**:
//
// ```text
//    despega un vuelo   -> hay noticia suya
//    aterriza un vuelo  -> hay noticia suya
//    silencio = ahora - la ultima noticia, SOLO si le quedan vuelos abiertos
// ```
//
// Un aparato ocupado da noticias todo el rato y su silencio no crece. Uno
// muerto deja de darlas con la cuenta en alto, y ahi si. Es un perro guardian,
// y es lo que hace el hardware desde siempre.
//
// # [!] Y `mm` NO PREGUNTA LA HORA. El que llama la trae
//
// `en_vuelo` y `aterrizo` reciben `cuando` en vez de leer el reloj, y aqui ese
// numero es **OPACO**: no se sabe de que reloj sale ni en que unidad viene.
// Solo se resta.
//
// ```text
//    si `mm` leyera el reloj   mm -> task::scheduler, una flecha NUEVA y al
//                              reves: el planificador se apoya en la memoria
//    con `cuando` de fuera     cero dependencias nuevas. `dev/` ya tiene el
//                              reloj a mano, y es quien programa el descriptor
// ```
//
// *** Es la misma decision que `bmo-dma-juicio`, que recibe un `bool` y un
// `u16` y jamas el enum del kernel. Un juez que se trae media casa para poder
// juzgar acaba siendo la casa.
//
// ** Y cambiar la FIRMA en vez de anadir una funcion aparte es a proposito: el
// que programa un descriptor tiene que decir cuando. Si se pudiera no decirlo,
// alguien no lo diria, y su aparato seria el unico sin perro guardian.
//
// # LO QUE ESTE PASO NO TRAE, Y ES LA MITAD: EL NUMERO
//
// ```text
//    N5a   el cronometro y su juez        <- ESTO. Hecho
//    N5b   CUANTO es el plazo             <- se MIDE, no se elige (LEY 24)
// ```
//
// [!] `PLAZO_SIN_MEDIR = 0` significa **no hay plazo**, y `caducados` con un
// plazo de cero no caduca a nadie. No es un valor por defecto prudente: es la
// negativa a inventarse un numero. Elegir "un segundo" porque suena bien seria
// la estimacion generica que LEY 24 prohibe por escrito -- una estimacion de
// OTRO proyecto.
//
// *** Y `ciclos.bex` acaba de ensenar por que ESTA medida no se hace como las
// demas. En el Ryzen, midiendo un bucle VACIO:
//
// ```text
//    bucle vacio    min 11 ticks    media 122 ticks    <- once veces
//    llamada normal min 30 ticks    media  31 ticks    <- clavada
// ```
//
// ** Todas las medidas de esta casa se quedan con el MINIMO, porque el minimo
// es lo que cuesta la maquina y la media es la maquina mas lo que pasaba
// alrededor. **Un plazo es la unica medida donde el minimo es la respuesta
// equivocada**: un plazo puesto en el mejor caso caduca vuelos sanos todo el
// rato. Un plazo se pone en la COLA, y por eso lo que se guarda aqui es el
// PEOR silencio visto y no la media de nada.
//
// > Para saber lo que cuesta algo se mira el minimo. Para saber cuanto esperar
// > se mira lo peor que ha pasado nunca, y despues se le anade margen.

/// **PONER UN MARCO EN VUELO PARA UN APARATO.** Se llama al programar el
/// descriptor, ANTES de tocar la campana.
///
/// Devuelve `false` y no toca nada si:
///
/// ```text
///    el marco cae fuera del espejo      no hay donde apuntarlo
///    `aparato` es 0 o pasa de 15        no cabe en el nibble
///    ya esta en vuelo para OTRO         *** dos aparatos, un bufer
/// ```
///
/// ** El tercero se RECHAZA en vez de sobreescribir, y esa es la decision de
/// esta funcion. Sobreescribir dejaria al primer aparato escribiendo en un
/// bufer que el sistema cree del segundo, y el aterrizaje del segundo borraria
/// la marca del primero: **dos fallos silenciosos por el precio de uno**.
///
/// [!] Y volver a ponerlo en vuelo para EL MISMO aparato SI vale, y no suma:
/// un driver que reprograma el mismo bufer antes de que el anterior termine
/// esta haciendo algo suyo, y contarlo dos veces dejaria la cuenta sin poder
/// llegar a cero nunca.
pub fn en_vuelo(phys: u64, aparato: u8, cuando: u64) -> bool {
    if aparato == 0 || aparato > 15 {
        return false;
    }
    let i = match indice(phys) {
        Some(i) => i,
        None => return false,
    };
    let antes = tabla()[i];
    let quien = (antes & 0xF0) >> 4;
    if quien != 0 && quien != aparato {
        unsafe { EN_VUELO_CHOQUES = EN_VUELO_CHOQUES.wrapping_add(1) };
        return false;
    }
    tabla()[i] = (antes & 0x0F) | (aparato << 4);
    if quien == 0 {
        // ** Se mira ANTES de sumar: lo que interesa no es cuantos hay ahora,
        // es si habia alguno DURANTE el silencio que se va a apuntar. Un
        // aparato que pasa de 0 a 1 estaba libre, no callado.
        let habia = unsafe { VUELOS_DE[aparato as usize] } != 0;
        unsafe {
            EN_VUELO_VIVOS += 1;
            VUELOS_DE[aparato as usize] += 1;
        }
        // ** HAY NOTICIA SUYA. Se apunta en CADA despegue y en CADA
        // aterrizaje, no solo al empezar: es un perro guardian, y lo que
        // vigila es el SILENCIO. Ver la nota del plazo, arriba.
        anoto(aparato, cuando, habia);
    }
    true
}

/// Deja constancia de que este aparato sigue vivo, y guarda el peor silencio
/// que se le ha visto teniendo trabajo pendiente.
///
/// ** El PEOR y no la media, y no el minimo: es la unica medida de esta casa
/// donde el minimo es la respuesta equivocada. Ver la nota del plazo.
/// Deja constancia de que este aparato dio senyales, y apunta el silencio
/// anterior **solo si durante ese silencio habia trabajo pendiente**.
///
/// # *** EL METAL LO CORRIGIO: 2,47 SEGUNDOS QUE NO ERAN UN SILENCIO
///
/// El arranque del 2026-09-10 saco `y callo 2467697 us` para el disco. Un
/// disco no tarda dos segundos y medio en contestar: **eso no era un aparato
/// callado, era un aparato OCIOSO**.
///
/// ** La primera version media el hueco entre dos noticias sin mirar si habia
/// algo abierto en medio. O sea que una lectura a las 12:00 y otra a las
/// 12:00:02 apuntaban dos segundos de *silencio* con el disco sin nada que
/// hacer -- y de ese numero iba a salir el plazo de R-DMA-8.
///
///   > Un aparato que no tiene trabajo no esta callado. Esta libre. Medir las
///   > dos cosas con el mismo reloj da un plazo que nunca caduca a nadie.
///
/// [!] Y el fallo no lo caza ninguna prueba: el numero SALIA, era plausible, y
/// solo se vio raro al mirarlo en la maquina con el disco parado. Es la clase
/// exacta que `LEY 24` describe -- lo que no se perfila se supone.
fn anoto(aparato: u8, cuando: u64, habia_trabajo: bool) {
    let a = aparato as usize;
    unsafe {
        let antes = ULTIMA_NOTICIA[a];
        // [!] `antes == 0` es la PRIMERA noticia de este aparato en toda la
        // vida de la maquina. No hay silencio que medir contra el arranque:
        // medirlo daria un `peor` enorme el primer dia y el plazo saldria de
        // ahi. Un numero que sale de no tener con que comparar es peor que
        // no tener numero.
        if antes != 0 && habia_trabajo {
            let callado = cuando.wrapping_sub(antes);
            if callado > PEOR_SILENCIO[a] {
                PEOR_SILENCIO[a] = callado;
            }
        }
        ULTIMA_NOTICIA[a] = cuando;
    }
}

/// **ATERRIZO: el aparato dijo que termino.** Se llama al consumir el fin.
///
/// # *** POR QUE PIDE EL APARATO, Y NO SOLO LA DIRECCION
///
/// La primera version era `aterrizo(phys)` y **le quitaba el vuelo a quien
/// fuera**. El dueno pidio buscar *"algo que pueda jugar en contra"* y era
/// esto, tres horas despues de escribirlo:
///
/// ```text
///    el AHCI aterriza un tramo de N paginas
///    una de ellas la tenia la NIC en vuelo
///    -> el AHCI le borra la bandera a la NIC, y nadie se entera
/// ```
///
/// ** El juez de arriba caza el caso al PROGRAMAR --`DeOtroAparato`-- pero
/// aterrizar no pasaba por ningun juez. Un contador que puede bajar por el
/// aparato equivocado deja de ser un contador: `vivos` llegaria a cero **con
/// un vuelo todavia en el aire**, que es la mentira exacta que este bit
/// existe para no contar.
///
/// > Poner el bit se comprueba. Quitarlo tambien tiene que comprobarse. Una
/// > barrera que solo mira en un sentido es una puerta.
///
/// # Que devuelve
///
/// ```text
///    true    era suyo y aterrizo
///    false   no habia vuelo, o **era de OTRO** -- y eso se CUENTA
/// ```
///
/// [!] `false` no es inocente en ninguno de los dos casos: o se consumio un
/// fin que nadie pidio, o alguien esta aterrizando lo ajeno. Quien llama
/// decide si le importa; aqui se contesta y se apunta.
pub fn aterrizo(phys: u64, aparato: u8, cuando: u64) -> bool {
    let i = match indice(phys) {
        Some(i) => i,
        None => return false,
    };
    let antes = tabla()[i];
    let quien = (antes & 0xF0) >> 4;
    if quien == 0 {
        return false;
    }
    if quien != aparato {
        // *** ATERRIZAR LO AJENO. Se cuenta con los choques porque es el
        // mismo fallo por el otro lado: dos aparatos creyendose duenos del
        // mismo marco. Y NO se toca la tabla: el vuelo del otro sigue vivo,
        // que es lo unico correcto que se puede hacer aqui.
        unsafe { EN_VUELO_CHOQUES = EN_VUELO_CHOQUES.wrapping_add(1) };
        return false;
    }
    tabla()[i] = antes & 0x0F;
    unsafe {
        EN_VUELO_VIVOS = EN_VUELO_VIVOS.saturating_sub(1);
        VUELOS_DE[aparato as usize] = VUELOS_DE[aparato as usize].saturating_sub(1);
    }
    // ** Aqui `habia_trabajo` es SIEMPRE cierto y no hay que preguntarlo: se
    // acaba de cerrar un vuelo, asi que durante el silencio anterior ese
    // vuelo estaba abierto. Este es el unico sitio que mide un silencio de
    // verdad -- lo que tardo el aparato en contestar.
    // ** Aterrizar TAMBIEN es dar noticias. Un aparato que aterriza no esta
    // callado, por muchos vuelos que le queden abiertos.
    anoto(aparato, cuando, true);
    true
}


/// **EL PLAZO QUE TODAVIA NO SE HA MEDIDO.** Cero significa SIN PLAZO, y con
/// el `caducados` no caduca a nadie.
///
/// [!] No es un valor por defecto prudente: es la negativa a inventarse un
/// numero. El plazo sale de [`super::verde::peor_silencio`] despues de varios
/// arranques, con margen -- LEY 24, y ademas hay que medirlo en la COLA y no
/// en el minimo. Ver la nota del plazo, arriba.
pub const PLAZO_SIN_MEDIR: u64 = 0;

/// **EL JUEZ DE R-DMA-8: quien lleva demasiado callado con trabajo abierto.**
///
/// Devuelve un mapa de bits --bit N = aparato N-- de los que pasaron de plazo,
/// y lo cuenta. `plazo` y `ahora` vienen en las mismas unidades que el `cuando`
/// de [`en_vuelo`]: **aqui no se sabe cuales son**, solo se restan.
///
/// ```text
///    plazo = 0             no caduca nadie. Es lo que hay hoy
///    sin vuelos abiertos   no caduca: callarse estando libre no es morirse
///    con vuelos abiertos   caduca si lleva mas de `plazo` sin dar noticias
/// ```
///
/// [!] Y **NO SE HACE NADA CON LA RESPUESTA**, todavia. Igual que las otras
/// siete reglas: primero el numero, y la barrera cuando el numero lleve
/// arranques diciendo cero. Dar por perdido un vuelo es soltar un marco en el
/// que un aparato podria estar escribiendo -- que es el fallo que todo esto
/// existe para no cometer.
pub fn caducados(ahora: u64, plazo: u64) -> u16 {
    if plazo == 0 {
        return 0;
    }
    let mut mapa = 0u16;
    for a in 1..16usize {
        let (abiertos, ultima) = unsafe { (VUELOS_DE[a], ULTIMA_NOTICIA[a]) };
        if abiertos == 0 || ultima == 0 {
            continue;
        }
        if ahora.wrapping_sub(ultima) > plazo {
            mapa |= 1 << a;
            unsafe { CADUCADOS = CADUCADOS.wrapping_add(1) };
        }
    }
    mapa
}

/// **QUIEN tiene un DMA en vuelo hacia este marco**, si es que alguno.
///
/// Es lo que `bmo-dma-juicio` pide como `en_vuelo_para`: el campo que hasta hoy
/// nadie podia rellenar con la verdad.
pub fn en_vuelo_de(phys: u64) -> Option<u8> {
    let i = indice(phys)?;
    match (tabla()[i] & 0xF0) >> 4 {
        0 => None,
        a => Some(a),
    }
}
