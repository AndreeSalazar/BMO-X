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
    /// **Traer un fichero de FAT32.** El contenido lo lee el kernel.
    ///
    /// ** Es el unico gesto cuyo coste NO se sabe leyendo el gesto: depende del
    /// tamano del origen, que hay que ir a mirar al otro volumen. Por eso
    /// `aplicar` lo pregunta antes de reservar, y por eso este es el unico que
    /// puede fallar con "esa ruta no existe" hablando de OTRO sistema de
    /// ficheros.
    Copia { nombre: &'a str, origen: &'a str },
}

impl Gesto<'_> {
    /// Cuesta un bloque para el objeto nuevo, o ninguno.
    fn bloques_de_objeto(&self) -> u64 {
        match self {
            // La copia cuesta su nodo Y su arbol, y el arbol solo lo sabe
            // quien ha ido a medir el origen. Aqui se cuenta el nodo; el resto
            // lo suma `aplicar` con lo que le diga `copiar::coste`.
            Gesto::Fichero { .. } | Gesto::Carpeta { .. } | Gesto::Copia { .. } => 1,
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
        // El nodo de una copia se hace DESPUES de escribir su arbol: hasta que
        // no existe la raiz no hay puntero que meterle. Aqui solo se dice que
        // habra objeto.
        Gesto::Copia { .. } => true,
        _ => false,
    };

    // ** LA COPIA SE MIDE ANTES DE RESERVAR. Es lo unico que hay que ir a
    // preguntarle a otro volumen, y hacerlo aqui --antes de abrir la
    // transaccion-- es lo que permite que "ese origen no existe" se conteste
    // sin haber tocado un sector de este.
    let flujo = match &gesto {
        Gesto::Copia { origen, .. } => Some(super::copiar::coste(origen)?),
        _ => None,
    };
    let cuesta = gesto.bloques_de_objeto()
        + flujo.map(|(bloques, _, _)| bloques).unwrap_or(0)
        + 2 * niveles as u64
        + 1;
    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok())
        .map_err(WriteError::Rechazada)?;
    let base = t.reserve(cuesta).map_err(WriteError::Rechazada)?;

    // El objeto va el primero, para que su puntero exista antes de la lista que
    // lo nombra.
    let mut cursor = base;
    // El CONTENIDO de una copia va antes que su nodo, por lo mismo que el nodo
    // va antes que la entrada que lo nombra: un puntero se escribe cuando ya
    // existe aquello a lo que apunta.
    if let (Gesto::Copia { origen, .. }, Some((bloques, plan, size))) = (&gesto, flujo) {
        objeto = super::copiar::traer(origen, plan, size, cursor)?;
        cursor += bloques;
    }
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
                Gesto::Fichero { nombre, .. }
                | Gesto::Carpeta { nombre }
                | Gesto::Copia { nombre, .. } => entradas_con(
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
            // ** LOS DOS MOTIVOS SE SEPARAN AQUI, y antes eran uno solo.
            //
            // Todo esto contestaba `NoCabe` --*"no cabe: hoy un fichero entra
            // en 96 bytes"*-- y para una carpeta llena eso es MENTIRA: el
            // fichero cabia, la carpeta no. Un mensaje asi manda a encoger el
            // fichero, que no arregla nada, y esconde el limite de verdad.
            //
            // La cuenta se hace ANTES de mirar el resultado porque la crate del
            // formato rechaza los tres casos --lleno, repetido, ausente-- con el
            // mismo error. Lo que si se puede saber aqui es si estaba llena.
            .map_err(|_| {
                let cabian = n_previas / es::objects::ENTRADA_LEN;
                if cabian >= es::escritura::ENTRADAS_POR_BLOQUE {
                    WriteError::CarpetaLlena
                } else {
                    WriteError::NombreNoVale
                }
            })?
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
    // ** EL MOTIVO VA VACIO, Y ESO NO ES UN OLVIDO.
    //
    // `Estrato::con_nombre()` mira si el motivo esta puesto, y la section 9 dice
    // que **los estratos CON NOMBRE no los suelta el recolector jamas**. O sea
    // que el motivo no es una etiqueta descriptiva: es lo que hace PERMANENTE a
    // una version.
    //
    // Yo escribia "fichero nuevo" en todos. Con eso, cada gesto quedaba marcado
    // como permanente y **el recolector no habria podido soltar ni uno** -- el
    // volumen creceria para siempre y nadie lo notaria hasta que se llenara.
    // Justo lo contrario de lo que dice la prueba que ya existia:
    // `un_estrato_automatico_no_lleva_nombre`.
    //
    // Un gesto normal es AUTOMATICO y va sin nombre. Ponerle uno es un acto
    // aparte, de una persona, y es lo que convierte una version en un punto al
    // que se puede volver siempre.
    //
    // [!] Y `tiempo` sigue siendo un CERO. El campo esta en el formato desde el
    // primer dia y aqui no hay reloj cableado, asi que la historia no tiene
    // fechas todavia. Se dice en vez de que alguien lo descubra mirando una
    // lista de versiones todas a la misma hora.
    // ** LA HORA, QUE HASTA HOY ERA UN CERO.
    //
    // El campo estaba en el formato desde el primer dia y aqui se escribia
    // `0` -- o sea que la historia del volumen existia y **no tenia fechas**:
    // una lista de versiones todas a la misma hora.
    //
    // `clock::ahora()` contesta `0` si la placa no dio una hora creible, y eso
    // se guarda tal cual: **no se inventa una fecha**. Una version fechada en
    // 1970 miente con mas conviccion que una sin fechar.
    let cuando = crate::ring0::dev::clock::ahora();
    let estrato = es::Estrato::new(
        p_raiz,
        sb.estrato,
        cuando,
        es::Autor::Proceso(crate::ring0::task::scheduler::current_pid()),
        "",
    );
    let e_bytes = estrato.encode();
    let p_estrato = BlockPtr::nuevo(cursor, 0, &e_bytes);
    poner(cursor, &e_bytes)?;

    // ** SE ACABAN LOS DATOS. ESTA LINEA FALTABA, Y SIN ELLA NO SE ESCRIBIA NADA.
    //
    // La transaccion tiene cuatro fases --Datos, Barrera, Commit, Cerrada-- y
    // `barrera_hecha()` exige estar en Barrera. Sin este `cerrar_datos()` la
    // transaccion seguia en Datos, asi que la barrera devolvia `FueraDeOrden`
    // y **el commit no llegaba a ocurrir jamas**.
    //
    // [!] Faltaba desde el commit que estreno la escritura (`1c96b133`, 18-08),
    // no desde el refactor de ayer: `crear_fichero` NUNCA ha guardado un
    // fichero. `sellar` si funcionaba --y por eso se vio la generacion 3 en el
    // Ryzen-- porque aquel camino si la llamaba.
    //
    // ** Y la maquina de estados hizo EXACTAMENTE lo que tenia que hacer: dijo
    // que no, con su motivo, y la ventana contestaba "NO se hizo". Lo que
    // faltaba no era una comprobacion mas: era ejecutarlo una vez. Es la razon
    // entera de la casilla de metal que sigue sin marcarse.
    t.cerrar_datos().map_err(WriteError::Rechazada)?;

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
    // La fecha de la version que acaba de mandar. Se sabe aqui sin leer nada:
    // es la que acabamos de escribir.
    unsafe { super::ESTRATO_FECHA = cuando };
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
        Gesto::Copia { .. } => "fichero copiado de FAT32",
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

/// **MARCA la version en curso con un nombre.** Devuelve la generacion nueva.
///
/// === Que es un nombre aqui, y por que no es una etiqueta ===
///
/// `Estrato::con_nombre()` mira si el motivo esta puesto, y la section 9 dice que
/// **los estratos CON NOMBRE no los suelta el recolector jamas**. Asi que poner
/// un nombre no describe una version: la hace PERMANENTE.
///
/// Los gestos normales van sin nombre a proposito -- son automaticos, y el
/// volumen tiene que poder adelgazar. Esto es el acto aparte, de una persona,
/// que dice *"a esta quiero poder volver siempre"*.
///
/// === ** Y ES TAMBIEN LO QUE HACE POSIBLE UNA RAMA ===
///
/// El superbloque apunta a UN estrato: la punta. Si se vuelve a una version
/// vieja y se escribe encima, la cadena se bifurca --cada estrato guarda su
/// padre, asi que la historia ya es un arbol-- pero **no hay donde apuntar la
/// otra punta**, y lo que nadie alcanza es basura.
///
/// Un nombre es esa referencia. Es el `ref` de git, y sale gratis del mecanismo
/// que ya existia para no soltar lo importante.
///
/// === Lo que cuesta ===
///
/// UN bloque. No se copia nada: el estrato nuevo apunta a la MISMA raiz que el
/// de ahora. Marcar un volumen de 400 GiB cuesta lo mismo que marcar uno vacio,
/// y eso es consecuencia directa de no sobreescribir nunca.
pub fn marcar(nombre: &str) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    if nombre.is_empty() {
        return Err(WriteError::NoCabe);
    }
    // El arbol de AHORA. Lo que se marca es la version que manda, no una nueva:
    // por eso se lee su raiz y se vuelve a publicar tal cual.
    let actual = super::estrato().ok_or(WriteError::NoSeLeeLaRaiz)?;

    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok())
        .map_err(WriteError::Rechazada)?;
    let base = t.reserve(1).map_err(WriteError::Rechazada)?;

    let cuando = crate::ring0::dev::clock::ahora();
    let estrato = es::Estrato::new(
        actual.raiz,
        sb.estrato,
        cuando,
        es::Autor::Proceso(crate::ring0::task::scheduler::current_pid()),
        nombre,
    );
    let bytes = estrato.encode();
    poner(base, &bytes)?;
    let p_estrato = BlockPtr::nuevo(base, 0, &bytes);

    // El mismo orden de siempre, que es el unico que no pierde datos.
    t.cerrar_datos().map_err(WriteError::Rechazada)?;
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;
    let (destino, nuevo) = t.commit(p_estrato).map_err(WriteError::Rechazada)?;
    if !write_superblock(destino, &nuevo.encode()) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino);
        return Err(WriteError::NoEscribio);
    }
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino);
        return Err(WriteError::SinBarrera);
    }
    super::fijar_superbloque(nuevo);
    unsafe { super::ESTRATO_FECHA = cuando };
    crate::ring0::cabina::info("estratos", "version marcada: no se soltara", nuevo.generation);
    Ok(nuevo.generation)
}

/// **VUELVE a la version `n` pasos atras.** `0` es la de ahora y no hace nada.
///
/// === ** NO SE COPIA NADA, Y ESA ES LA FRASE ENTERA ===
///
/// Los bloques de aquella version **siguen todos en el disco**: nada se
/// sobreescribio nunca. Volver es publicar un estrato nuevo que apunta a la
/// MISMA raiz que tenia aquella.
///
/// UN BLOQUE. Volver un volumen de 400 GiB cuesta exactamente lo mismo que
/// volver uno vacio, y eso no es una optimizacion: es lo que significa
/// copy-on-write cuando se lleva hasta el final.
///
/// === Y LO DE EN MEDIO NO SE PIERDE ===
///
/// El estrato nuevo tiene por padre **la punta de ahora**, no la version a la
/// que se vuelve. Asi que la cadena queda:
///
/// ```text
///   nueva -> 50 -> 49 -> ... -> 20 -> ...
///   (el arbol de la 20)   (la historia entera, intacta)
/// ```
///
/// Es un *revert*, no un *reset*: se deshace el contenido y **se conserva el
/// registro de que se deshizo**. En un almacen eso no es un detalle -- borrar
/// la historia para volver atras es perder la unica prueba de lo que paso.
///
/// === Va SIN NOMBRE, como cualquier gesto ===
///
/// Un nombre hace permanente a una version, y eso lo decide una persona. Volver
/// es un gesto mas; si esta vuelta importa, se marca despues. Ponerle nombre
/// aqui seria decidir por el dueno que su disco no puede adelgazar.
pub fn volver(n: usize) -> Result<u64, WriteError> {
    let sb = superbloque().ok_or(WriteError::SinVolumen)?;
    if n == 0 {
        // Volver a donde ya estas no es un error, pero tampoco es una version
        // nueva: se dice que no en vez de gastar un bloque en no cambiar nada.
        return Err(WriteError::NoCabe);
    }
    // Se recorre la cadena hasta la que se pide. Cuesta un bloque por paso y se
    // paga UNA vez, aqui -- no en el panel que la ensena.
    let mut donde = sb.estrato;
    let mut destino = None;
    let mut k = 0usize;
    while k <= n {
        if donde.es_nulo() {
            break;
        }
        let d = super::seguir(&donde, 0).ok_or(WriteError::NoSeLeeLaRaiz)?;
        let e = es::Estrato::decode(d).map_err(|_| WriteError::NoSeLeeLaRaiz)?;
        if k == n {
            destino = Some(e.raiz);
            break;
        }
        donde = e.padre;
        k += 1;
    }
    // Pedir una version que no existe se dice, en vez de volver a la mas vieja
    // que si -- eso seria obedecer una orden distinta de la que se dio.
    let raiz = destino.ok_or(WriteError::RutaNoEsta)?;

    let mut t = es::escritura::Transaccion::open(&sb, copia_en_uso(), identidad_ok())
        .map_err(WriteError::Rechazada)?;
    let base = t.reserve(1).map_err(WriteError::Rechazada)?;

    let cuando = crate::ring0::dev::clock::ahora();
    let estrato = es::Estrato::new(
        raiz,
        sb.estrato,
        cuando,
        es::Autor::Proceso(crate::ring0::task::scheduler::current_pid()),
        "",
    );
    let bytes = estrato.encode();
    poner(base, &bytes)?;
    let p_estrato = BlockPtr::nuevo(base, 0, &bytes);

    t.cerrar_datos().map_err(WriteError::Rechazada)?;
    if !disk::flush() {
        t.abandonar();
        return Err(WriteError::SinBarrera);
    }
    t.barrera_hecha().map_err(WriteError::Rechazada)?;
    let (destino_sb, nuevo) = t.commit(p_estrato).map_err(WriteError::Rechazada)?;
    if !write_superblock(destino_sb, &nuevo.encode()) {
        crate::ring0::cabina::fault("estratos", "no se pudo escribir el superbloque", destino_sb);
        return Err(WriteError::NoEscribio);
    }
    if !disk::flush() {
        crate::ring0::cabina::warn("estratos", "el commit no se pudo vaciar al plato", destino_sb);
        return Err(WriteError::SinBarrera);
    }
    super::fijar_superbloque(nuevo);
    unsafe { super::ESTRATO_FECHA = cuando };
    crate::ring0::cabina::info("estratos", "vuelta a una version anterior", nuevo.generation);
    Ok(nuevo.generation)
}

/// **Crea la carpeta `nombre` dentro de `ruta`.**
pub fn crear_carpeta(ruta: &str, nombre: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Carpeta { nombre })
}

/// **Quita `nombre` de `ruta`.** No destruye: deja de nombrar.
pub fn quitar(ruta: &str, nombre: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Quitar { nombre })
}

/// **Trae `origen` de FAT32 y lo guarda como `nombre` dentro de `ruta`.**
pub fn copiar_fichero(ruta: &str, nombre: &str, origen: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Copia { nombre, origen })
}

/// **Renombra `viejo` a `nuevo` dentro de `ruta`.** El nodo no se toca.
pub fn renombrar(ruta: &str, viejo: &str, nuevo: &str) -> Result<u64, WriteError> {
    aplicar(ruta, Gesto::Renombrar { viejo, nuevo })
}

/// Las entradas de un nivel, ENTERAS o ninguna. `0` si el atributo esta vacio.
///
/// === ** POR QUE SE MIDE ANTES DE LEER ===
///
/// Porque `walk::flujo` **trunca en silencio** cuando el destino se llena, y
/// hace bien: su otro cliente es el panel que pinta un listado, y ahi lo que se
/// quiere es lo que quepa. Aqui no. Aqui la lista se vuelve a escribir, asi que
/// leer la mitad y publicar seria **dejar fuera del arbol vivo** todas las
/// entradas a partir de la 37 -- sin un error, sin un aviso, y con el gesto
/// contestando que fue bien.
///
/// El tope es NUESTRO --`:entradas` vive en un bloque mientras no tenga
/// indireccion-- y por eso lo dice el, con su propio motivo. Es la misma regla
/// que ya sigue `cursor::verify` con su buffer de 256 KiB: *un limite propio se
/// confiesa, no se disfraza de fallo del disco.*
///
/// [!] Y hasta hoy no reventaba por una casualidad aritmetica: 4096 no es
/// multiplo de 112, asi que el corte dejaba 64 bytes sueltos y la crate del
/// formato lo rechazaba con `BadField` -- que aqui se traducia a *"ese nombre no
/// vale"* o *"esa ruta no existe"*. Dos mensajes que mandan a mirar donde no es,
/// y una garantia de datos colgando de una division que no sale exacta.
fn leer_entradas(a: &Attr, dst: &mut [u8; BLOQUE]) -> Result<usize, WriteError> {
    // Lo que YA hay no cabe de una vez: se para ANTES de leer nada y antes de
    // abrir la transaccion, o sea sin haber tocado un solo sector.
    if a.size as usize > dst.len() {
        return Err(WriteError::CarpetaNoCabeEntera);
    }
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
pub(super) fn poner(bloque: u64, datos: &[u8]) -> Result<(), WriteError> {
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
