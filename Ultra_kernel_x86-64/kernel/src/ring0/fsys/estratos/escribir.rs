//! **CREAR UN FICHERO EN ESTRATOS.** La mitad que toca el disco.
//!
//! [eje]     CORRECCION -- lo pide una persona y escribe en el almacen
//! [exige]   la seccion 5 del diseno (el paso que falta para 1.0), L7 (el
//!           formato no se decide aqui)
//!
//! # Que hace, y por que es corto
//!
//! Junta cuatro cosas que ya existian y nunca se habian llamado juntas:
//!
//! ```text
//!   bmo_estratos::escritura   QUE bytes    nodo, entradas, directorio
//!   Transaccion               CUANDO       reservar, barrera, commit
//!   dir + walk                lo de HOY    la raiz actual y sus entradas
//!   disk                      el aparato   write_block y FLUSH CACHE
//! ```
//!
//! Aqui no se decide el formato ni el orden: los dos vienen dados. Lo unico
//! propio es **el reparto de los cuatro bloques**, y esta escrito abajo.
//!
//! # ** POR QUE CUATRO BLOQUES PARA UN FICHERO DE 16 BYTES
//!
//! ```text
//!   base+0   el nodo del FICHERO      con su contenido dentro (residente)
//!   base+1   el bloque de ENTRADAS    las de antes + la nueva
//!   base+2   el nodo del DIRECTORIO   que apunta al bloque de entradas
//!   base+3   el ESTRATO               que apunta al directorio
//! ```
//!
//! Los tres ultimos no son del fichero: son **la version nueva del arbol**. En
//! un sistema que sobreescribe, anadir una entrada toca un bloque; aqui no se
//! toca ninguno, se copian los que cambian. Eso es el copy-on-write, y es lo que
//! hace que el arbol de ayer siga entero y alcanzable -- que es la razon de que
//! este sistema de ficheros exista.
//!
//! [!] Son cuatro y no dos porque **un objeto no comparte bloque**. El
//! formateador del anfitrion si los empaqueta, y hace bien: tiene el volumen
//! entero delante. Aqui gastar 16 KiB donde caben 1,4 es correcto y es simple, y
//! la contabilidad lo dice en voz alta -- el log_head sube de cuatro en cuatro.
//!
//! # El orden, que es el unico que no pierde datos
//!
//! ```text
//!   1  escribir los CUATRO bloques      todavia no los alcanza nadie
//!   2  FLUSH CACHE                      hasta aqui, el volumen es el de antes
//!   3  commit: el superbloque ALTERNO   el punto de no retorno, UN sector
//!   4  FLUSH CACHE otra vez             o el commit se queda en la cache
//! ```
//!
//! ** Si se corta la corriente en 1 o 2, el volumen monta exactamente igual que
//! antes: los bloques nuevos estan ahi y **no hay nada que los alcance**. Si se
//! corta en 3, se estropea la copia del superbloque que NO manda. No hay ninguna
//! ventana en la que se pierda algo.

use bmo_estratos as es;
use bmo_estratos::escritura::{
    entradas_con, entradas_renombrando, entradas_repuntando, entradas_sin, nodo_de_directorio,
    nodo_de_directorio_vacio, nodo_de_fichero,
};
use bmo_estratos::objects::{Attr, BlockPtr, ATTR_ENTRADAS, BLOQUE, NODO_LEN};

use super::{
    copia_en_uso, dir, identidad_ok, superbloque, walk, write_block, write_superblock, WriteError,
};
use crate::ring0::dev::disk;

/// Bloques que cuesta un fichero: el suyo, las entradas, el directorio, el estrato.
const BLOQUES_POR_FICHERO: u64 = 4;

/// Las entradas que YA tiene la raiz. Estatico y no en la pila: son 4 KiB, y la
/// pila del kernel son 64 para todo.
static mut PREVIAS: [u8; BLOQUE] = [0u8; BLOQUE];
/// El bloque de entradas NUEVO. Tiene que ser otro buffer: se lee del de arriba
/// mientras se escribe en este.
static mut ENTRADAS: [u8; BLOQUE] = [0u8; BLOQUE];
/// Un bloque de paso para escribir cada objeto pequeno con su relleno.
static mut BLOQUE_TMP: [u8; BLOQUE] = [0u8; BLOQUE];

/// **Lo que se le hace a la lista de entradas del directorio del final.**
///
/// Cuatro verbos y UNA maquina. El codigo lo dijo antes que nadie:
/// `entradas_con` es "la lista con una mas", y borrar es esa misma funcion con
/// una menos, renombrar con una cambiada, y crear carpeta la misma mas un nodo
/// de directorio. Tenerlos como cuatro caminos distintos habria sido escribir
/// cuatro veces la parte peligrosa -- la que toca el disco.
pub enum Gesto<'a> {
    /// Un fichero nuevo con su contenido dentro (hasta 96 bytes).
    Fichero { nombre: &'a str, datos: &'a [u8] },
    /// Una carpeta vacia.
    Carpeta { nombre: &'a str },
    /// Quitar una entrada. **No destruye**: deja de nombrar.
    Quitar { nombre: &'a str },
    /// Cambiarle el nombre a una entrada, sin tocar su nodo.
    Renombrar { viejo: &'a str, nuevo: &'a str },
}

impl Gesto<'_> {
    /// Cuesta un bloque para el objeto nuevo, o ninguno.
    fn bloques_de_objeto(&self) -> u64 {
        match self {
            Gesto::Fichero { .. } | Gesto::Carpeta { .. } => 1,
            Gesto::Quitar { .. } | Gesto::Renombrar { .. } => 0,
        }
    }
}

/// Lo hondo que se puede tocar.
///
/// ** Ocho niveles, y el tope NO es prudencia general: **tocar una hoja
/// republica la rama entera hasta la raiz**, asi que la profundidad ES el precio
/// de la operacion -- dos bloques por nivel. Un tope explicito lo dice antes en
/// vez de que lo descubra una reserva que no cabe.
pub const HONDO_MAX: usize = 8;

/// El camino, de la raiz hacia abajo. `NODOS[0]` es la raiz.
///
/// [!] Se guardan los NODOS y no sus entradas. Un bloque de entradas son 4 KiB;
/// nueve niveles serian 36 KiB de `.bss` para tenerlos todos a la vez, y no
/// hacen falta: al volver hacia arriba se relee el de cada nivel cuando le toca.
/// Dos lecturas por nivel en vez de 36 KiB parados.
static mut NODOS: [Option<es::objects::Nodo>; HONDO_MAX + 1] = [None; HONDO_MAX + 1];
/// Con que nombre se bajo a cada nivel. `NOMBRES[0]` no se usa: la raiz no tiene.
static mut NOMBRES: [[u8; 64]; HONDO_MAX + 1] = [[0; 64]; HONDO_MAX + 1];
static mut NOMBRES_LEN: [usize; HONDO_MAX + 1] = [0; HONDO_MAX + 1];

/// Recorre `ruta` desde la raiz y deja el camino en los estaticos.
///
/// Devuelve lo hondo que se bajo: `0` es la raiz. **No escribe nada** -- si algo
/// falla aqui, no se ha abierto ninguna transaccion ni tocado un sector.
fn resolver(ruta: &str) -> Result<usize, WriteError> {
    let (_, raiz) = dir::raiz().ok_or(WriteError::NoSeLeeLaRaiz)?;
    let nodos = unsafe { &mut *core::ptr::addr_of_mut!(NODOS) };
    let nombres = unsafe { &mut *core::ptr::addr_of_mut!(NOMBRES) };
    let largos = unsafe { &mut *core::ptr::addr_of_mut!(NOMBRES_LEN) };
    nodos[0] = Some(raiz);
    let mut hondo = 0usize;
    for tramo in ruta.split(['/', '\\']) {
        if tramo.is_empty() {
            continue;
        }
        if hondo + 1 > HONDO_MAX {
            return Err(WriteError::MuyHondo);
        }
        let padre = nodos[hondo].ok_or(WriteError::RutaNoEsta)?;
        let ptr = super::buscar_en(&padre, tramo).ok_or(WriteError::RutaNoEsta)?;
        let n = walk::nodo(&ptr).ok_or(WriteError::RutaNoEsta)?;
        // Un fichero no es un sitio donde poner cosas. Se dice, en vez de
        // dejarlo pasar y publicar un arbol donde un `:datos` hace de carpeta.
        if n.tipo != es::objects::Tipo::Directorio {
            return Err(WriteError::RutaNoEsta);
        }
        hondo += 1;
        nodos[hondo] = Some(n);
        let b = tramo.as_bytes();
        let k = b.len().min(64);
        nombres[hondo][..k].copy_from_slice(&b[..k]);
        largos[hondo] = k;
    }
    Ok(hondo)
}

/// **APLICA `gesto` al directorio que hay en `ruta`.** Devuelve la generacion.
///
/// # Por que una sola funcion para los cuatro verbos
///
/// Porque lo caro y lo peligroso --reservar, escribir, la barrera, el commit--
/// es identico en los cuatro. Lo unico que cambia es **que lista de entradas se
/// escribe**, y eso son tres lineas de `match` sobre funciones que ya estan
/// probadas en el anfitrion.
///
/// # ** LO QUE CUESTA, Y POR QUE NO ES UN DEFECTO
///
/// ```text
///   1 bloque    el objeto nuevo, si lo hay
///   2 bloques   POR CADA NIVEL de la ruta, la raiz incluida
///   1 bloque    el estrato
/// ```
///
/// Crear un fichero en `/a/b` son siete bloques: el fichero, las entradas y el
/// nodo de `b`, los de `a`, los de la raiz, y el estrato. **Tocar una hoja
/// republica la rama entera hasta la raiz.**
///
/// No es un coste que se pueda evitar: es lo que significa no sobreescribir. La
/// entrada `b` de `a` apuntaba al nodo viejo de `b`, asi que `a` cambia; y
/// entonces la raiz cambia. A cambio, **el arbol de ayer sigue completo**,
/// porque ninguno de sus bloques se ha tocado.
///
/// # El orden es el mismo de siempre, y es el unico que no pierde datos
///
/// Todos los bloques, barrera, superbloque alterno, barrera. Un corte antes del
/// commit deja el volumen exactamente como estaba: lo escrito no lo alcanza
/// nadie.
pub fn aplicar(ruta: &str, gesto: Gesto) -> Result<u64, WriteError> {
    match publicar(ruta, &gesto) {
        Ok(g) => Ok(g),
        Err(e) => {
            // ** TODO FALLO PASA POR AQUI, Y POR ESO CABINA ESTA AQUI Y NO EN
            // CADA VERBO.
            //
            // Cuatro verbos y una sola caja negra: si el aviso viviera en cada
            // uno, el dia que se anada el quinto se olvidaria -- y un gesto que
            // falla en silencio sobre un disco es la peor sorpresa que hay.
            crate::ring0::cabina::warn("estratos", e.name(), 0);
            // Y lo segundo es lo que de verdad tranquiliza al que lee el log
            // despues: **el commit es el UNICO punto en el que el volumen
            // cambia**, y no se llego a el. Lo escrito antes de fallar no lo
            // alcanza nadie -- son bloques sueltos, que es justo el trabajo del
            // recolector, no una corrupcion.
            crate::ring0::cabina::info(
                "estratos",
                "el gesto NO se hizo: el volumen sigue en su generacion",
                0,
            );
            Err(e)
        }
    }
}

/// El trabajo de verdad. Lo envuelve [`aplicar`], que es quien avisa.
fn publicar(ruta: &str, gesto: &Gesto) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;

    // -- Lo de HOY, antes de abrir nada. Si esto falla, no se ha tocado nada.
    let hondo = resolver(ruta)?;
    let niveles = hondo + 1;

    // -- El objeto nuevo, si el gesto trae uno. Se codifica ANTES de reservar
    // para que "no cabe" se diga sin haber pedido un solo bloque.
    let mut objeto = [0u8; NODO_LEN];
    let hay_objeto = match gesto {
        Gesto::Fichero { datos, .. } => {
            objeto = nodo_de_fichero(datos).map_err(|_| WriteError::NoCabe)?;
            true
        }
        Gesto::Carpeta { .. } => {
            objeto = nodo_de_directorio_vacio();
            true
        }
        _ => false,
    };

    let cuesta = gesto.bloques_de_objeto() + 2 * niveles as u64 + 1;
    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok())
        .map_err(WriteError::Rechazada)?;
    let base = t.reserve(cuesta).map_err(WriteError::Rechazada)?;

    // El objeto va el primero, para que su puntero exista antes de la lista que
    // lo nombra.
    let mut cursor = base;
    let p_objeto = if hay_objeto {
        let p = BlockPtr::nuevo(cursor, 0, &objeto);
        poner(cursor, &objeto)?;
        cursor += 1;
        Some(p)
    } else {
        None
    };

    // -- DE ABAJO HACIA ARRIBA. Cada nivel publica su nodo, y el de encima
    // repunta su entrada hacia el.
    let previas = unsafe { &mut *core::ptr::addr_of_mut!(PREVIAS) };
    let entradas = unsafe { &mut *core::ptr::addr_of_mut!(ENTRADAS) };

    let mut hijo: Option<BlockPtr> = None;
    let mut nivel = hondo as isize;
    while nivel >= 0 {
        let k = nivel as usize;
        let este = unsafe { (*core::ptr::addr_of!(NODOS))[k] }.ok_or(WriteError::NoSeLeeLaRaiz)?;
        let n_previas = match este.attr(ATTR_ENTRADAS) {
            None => 0,
            Some(a) => leer_entradas(a, previas)?,
        };

        let n_ent = if k == hondo {
            // El nivel del final: aqui pasa lo que el gesto pedia.
            match gesto {
                Gesto::Fichero { nombre, .. } | Gesto::Carpeta { nombre } => entradas_con(
                    &previas[..n_previas],
                    nombre,
                    p_objeto.ok_or(WriteError::NoCabe)?,
                    entradas,
                ),
                Gesto::Quitar { nombre } => entradas_sin(&previas[..n_previas], nombre, entradas),
                Gesto::Renombrar { viejo, nuevo } => {
                    entradas_renombrando(&previas[..n_previas], viejo, nuevo, entradas)
                }
            }
            .map_err(|_| WriteError::NoCabe)?
        } else {
            // Un nivel de paso: su hijo tiene nodo nuevo, asi que su entrada
            // tiene que apuntar ahi. El nombre es el que se uso para bajar.
            let largo = unsafe { (*core::ptr::addr_of!(NOMBRES_LEN))[k + 1] };
            // La referencia se saca del array ENTERO y se indexa despues: coger
            // `&(*ptr)[i]` es una autoref sobre un puntero crudo, y el
            // compilador la rechaza con razon.
            let todos = unsafe { &*core::ptr::addr_of!(NOMBRES) };
            let bytes = &todos[k + 1][..largo];
            let nom = core::str::from_utf8(bytes).map_err(|_| WriteError::RutaNoEsta)?;
            entradas_repuntando(
                &previas[..n_previas],
                nom,
                hijo.ok_or(WriteError::RutaNoEsta)?,
                entradas,
            )
            .map_err(|_| WriteError::RutaNoEsta)?
        };

        // El nodo del nivel. Si se quedo sin entradas, es una carpeta vacia --
        // el MISMO nodo que una recien nacida, no un estado nuevo.
        //
        // [!] El bloque de entradas reservado se escribe igual, aunque quede a
        // cero y nadie lo apunte. Devolverlo obligaria a que la reserva
        // dependiera del RESULTADO de la transformacion, o sea a pedir el sitio
        // despues de saber cuanto hace falta. Un bloque suelto es exactamente el
        // trabajo del recolector.
        poner(cursor, &entradas[..n_ent])?;
        let p_ent = BlockPtr::nuevo(cursor, 0, &entradas[..n_ent]);
        cursor += 1;

        let nodo_d = if n_ent == 0 {
            nodo_de_directorio_vacio()
        } else {
            nodo_de_directorio(p_ent, n_ent as u64).map_err(|_| WriteError::NoCabe)?
        };
        poner(cursor, &nodo_d)?;
        hijo = Some(BlockPtr::nuevo(cursor, 0, &nodo_d));
        cursor += 1;

        nivel -= 1;
    }

    // -- El estrato, apuntando a la raiz NUEVA.
    let p_raiz = hijo.ok_or(WriteError::NoSeLeeLaRaiz)?;
    let estrato = es::Estrato::new(
        p_raiz,
        sb.estrato,
        0,
        es::Autor::Proceso(crate::ring0::task::scheduler::current_pid()),
        motivo(gesto),
    );
    let e_bytes = estrato.encode();
    let p_estrato = BlockPtr::nuevo(cursor, 0, &e_bytes);
    poner(cursor, &e_bytes)?;

    // -- LA BARRERA. Hasta aqui el volumen sigue siendo el de antes.
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;

    // -- EL COMMIT: un sector, en la copia que no manda.
    let (destino, nuevo) = t.commit(p_estrato).map_err(WriteError::Rechazada)?;
    if !write_superblock(destino, &nuevo.encode()) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }

    // -- Y VACIAR OTRA VEZ, o el commit se queda en la cache del disco.
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }

    super::fijar_superbloque(nuevo);
    crate::ring0::cabina::info("estratos", motivo(gesto), nuevo.generation);
    Ok(nuevo.generation)
}

/// Lo que queda escrito EN EL ESTRATO como motivo del cambio.
///
/// ** Se guarda dentro del propio estrato, no en un log aparte: recorrer la
/// cadena hacia atras es recorrer la historia, y una historia que no dice que
/// paso en cada paso es una lista de numeros.
fn motivo(g: &Gesto) -> &'static str {
    match g {
        Gesto::Fichero { .. } => "fichero nuevo",
        Gesto::Carpeta { .. } => "carpeta nueva",
        Gesto::Quitar { .. } => "entrada quitada",
        Gesto::Renombrar { .. } => "entrada renombrada",
    }
}

/// **Crea `nombre` con `datos` dentro.** Devuelve la generacion nueva.
///
/// El contenido va DENTRO del nodo mientras quepa (96 bytes). Mas grande pide un
/// arbol de bloques con sus niveles, y eso es otra funcion -- se rechaza en vez
/// de partirlo a escondidas, para que el que llama sepa lo que cuesta.
pub fn crear_fichero(nombre: &str, datos: &[u8]) -> Result<u64, WriteError> {
    // ** ESTO ERA NOVENTA LINEAS Y AHORA ES UNA, y el reparto de bloques que
    // publica no ha cambiado ni un indice: con la ruta vacia, `aplicar` reserva
    // 1 + 2*1 + 1 = cuatro y los escribe en el mismo orden que antes --nodo,
    // entradas, directorio, estrato--. Lo que se probo en metal sigue siendo
    // literalmente el mismo camino, y eso no es casualidad: la maquina general
    // se escribio para que el caso de la raiz cayera EXACTAMENTE donde estaba.
    aplicar("", Gesto::Fichero { nombre, datos })
}

/// **Crea la carpeta `nombre` dentro de `ruta`.**
pub fn crear_carpeta(ruta: &str, nombre: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Carpeta { nombre })
}

/// **Quita `nombre` de `ruta`.** No destruye: deja de nombrar.
pub fn quitar(ruta: &str, nombre: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Quitar { nombre })
}

/// **Renombra `viejo` a `nuevo` dentro de `ruta`.** El nodo no se toca.
pub fn renombrar(ruta: &str, viejo: &str, nuevo: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Renombrar { viejo, nuevo })
}

/// Las entradas de la raiz, en `dst`. `0` si el atributo esta vacio.
fn leer_entradas(a: &Attr, dst: &mut [u8; BLOQUE]) -> Result<usize, WriteError> {
    match walk::flujo(a, dst) {
        Some(n) => Ok(n),
        // No poder leer lo que YA hay es lo peor que puede pasar aqui: escribir
        // sin ello dejaria un directorio con una sola entrada y el resto
        // huerfano. Se para antes de abrir la transaccion.
        None => Err(WriteError::NoSeLeeLaRaiz),
    }
}

/// Escribe un trozo en su bloque, con el relleno a cero.
///
/// ** El relleno va a CERO y no se deja lo que hubiera: el `BlockPtr` solo
/// verifica los `len` bytes del objeto, asi que la basura de detras no rompe
/// nada -- pero un bloque recien reservado con datos viejos dentro es justo lo
/// que no se quiere encontrar el dia que alguien mire el disco a mano.
fn poner(bloque: u64, datos: &[u8]) -> Result<(), WriteError> {
    if datos.len() > BLOQUE {
        return Err(WriteError::NoCabe);
    }
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(BLOQUE_TMP) };
    *buf = [0u8; BLOQUE];
    buf[..datos.len()].copy_from_slice(datos);
    if write_block(bloque, buf) {
        Ok(())
    } else {
        crate::ring0::cabina::fault("estratos", "el disco no acepto un bloque", bloque);
        Err(WriteError::NoEscribio)
    }
}

/// **Lo que va a costar un gesto**, para poder decirlo antes de escribir.
///
/// `hondo` es lo que baja la ruta (`0` = la raiz) y `objeto` si el gesto crea
/// algo. Dos bloques por nivel, uno por el objeto y uno por el estrato.
pub const fn coste(hondo: usize, objeto: bool) -> u64 {
    (objeto as u64) + 2 * (hondo as u64 + 1) + 1
}

/// [!] **EL CASO DE LA RAIZ SIGUE COSTANDO CUATRO BLOQUES.**
///
/// Es la garantia que protege lo unico que se ha probado en metal: crear un
/// fichero en la raiz publica los mismos cuatro bloques, en el mismo orden, que
/// antes de que existiera la maquina general. No es un comentario que promete
/// eso -- es el compilador negandose a construir si deja de ser verdad.
const _: () = assert!(
    coste(0, true) == BLOQUES_POR_FICHERO,
    "crear en la raiz tiene que seguir costando los mismos cuatro bloques"
);

/// Cabe el contenido dentro del nodo? Lo pregunta el que propone, antes de nada.
pub fn cabe(datos: &[u8]) -> bool {
    datos.len() <= es::objects::RESIDENTE_MAX
}

const _: () = assert!(NODO_LEN <= BLOQUE, "un nodo tiene que caber en su bloque");
