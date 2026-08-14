//! **THE CURSOR** -- ESTRATOS walked from Ring 3.
//!
//! === Why this is a file of its own ===
//!
//! Because it is the FRONTIER. Everything else in this folder is the kernel
//! talking to itself; this is the part a program outside the kernel can drive,
//! through two operations (`TASK_OP_ES_NODO` and `TASK_OP_ES_TEXTO`) and not
//! ten.
//!
//! ** That count is the design. Exposing a filesystem to Ring 3 usually means
//! open/read/seek/stat/readdir/close and a descriptor table to hold them;
//! here it is a cursor the kernel owns and two questions. Whatever is added
//! later gets added as an operation, and the surface does not move.

use super::*;

// -- El CURSOR: ESTRATOS recorrido desde Ring 3 ------------------------------
//
// === Por que un cursor y no un handle por nodo ===
//
// Un `KIND_ESTRATOS_NODO` con su capability por cada nodo abierto seria lo
// ortodoxo, y es exactamente lo que no hace falta: la ventana de Datos mira UN
// sitio a la vez y lo que quiere es *bajar, subir y listar*. Un handle por nodo
// pediria tabla, ciclo de vida y revocacion para modelar un puntero que se
// mueve -- y un puntero que se mueve es un cursor.
//
// === Por que esto no concede nada ===
//
// Es el mismo trato que `OP_INFO` y que el klog: **contesta, no autoriza**.
// Leer los nombres de un directorio no ejerce ningun poder que Ring 3 no tenga
// ya --`ls` sobre FAT32 hace justo eso--, y ESCRIBIR sigue sin existir aqui: el
// cursor no tiene ninguna operacion que cambie el volumen.
//
// === Lo que faltaba, dicho ===
//
// `raiz`, `nodo`, `entries` y `entrada` llevaban desde el principio siendo
// funciones de Ring 0 sin puerta. La ventana F12 podia ensenar los NUMEROS del
// volumen --generacion, ocupacion, nivel-- y no podia ensenar **que hay dentro**,
// porque no tenia de donde sacarlo. Esto es esa puerta.
pub mod cursor {
    use super::*;

    /// Cuanto se puede bajar. Dieciseis niveles de directorio es mas de lo que
    /// tiene ningun volumen razonable, y un tope explicito es mejor que una
    /// pila que crece hasta que algo se rompe.
    pub const HONDO_MAX: usize = 16;
    /// Lo que se guarda del nombre de cada nivel de la ruta. Un nombre mas
    /// largo se recorta **y se dice** -- ver `level_name`.
    pub const LEVEL_NAME: usize = 32;

    /// El buffer del cursor, SUYO. Ver [`super::listar_en`].
    static mut BUF: [u8; MAX_ENTRIES * ENTRADA_LEN] = [0u8; MAX_ENTRIES * ENTRADA_LEN];
    /// La ruta desde la raiz: `PILA[0]` es la raiz y `PILA[HONDO]` el actual.
    static mut PILA: [Option<BlockPtr>; HONDO_MAX] = [None; HONDO_MAX];
    /// El NAME de cada nivel, para poder ensenar la ruta de verdad.
    ///
    /// * Se guarda al bajar y no se reconstruye despues, y ese es el motivo de
    /// que exista: un `BlockPtr` sabe DONDE esta un nodo y **no sabe como se
    /// llama** -- el nombre vive en la entrada del padre, no en el hijo. Para
    /// sacarlo a posteriori habria que releer el directorio de arriba y buscar
    /// que entrada apunta aqui. Anotarlo al pasar cuesta 64 bytes por nivel.
    ///
    /// Sin esto, la ventana solo puede decir `profundidad 2`, y dos carpetas
    /// distintas con los mismos nombres dentro se ven identicas.
    static mut NAMES: [[u8; LEVEL_NAME]; HONDO_MAX] = [[0; LEVEL_NAME]; HONDO_MAX];
    static mut NAMES_LEN: [usize; HONDO_MAX] = [0; HONDO_MAX];
    static mut HONDO: usize = 0;
    static mut ACTUAL: Option<Nodo> = None;
    static mut CUANTAS: usize = 0;
    static mut TRUNCADO: bool = false;

    /// Relista el nodo actual. Todo lo que mueve el cursor acaba aqui.
    pub(crate) fn relistar() -> bool {
        unsafe {
            let Some(n) = ACTUAL else {
                CUANTAS = 0;
                TRUNCADO = false;
                return false;
            };
            let buf = &mut *core::ptr::addr_of_mut!(BUF);
            match listar_en(&n, buf) {
                Some((c, t)) => {
                    CUANTAS = c.min(MAX_ENTRIES);
                    TRUNCADO = t;
                    true
                }
                // Un archivo no tiene `:entries`, y eso no es un fallo: es que
                // no tiene hijos. Se contesta cero y se sigue.
                None => {
                    CUANTAS = 0;
                    TRUNCADO = false;
                    true
                }
            }
        }
    }

    /// Pone el cursor en la raiz del volumen. `false` si no hay volumen.
    pub fn a_la_raiz() -> bool {
        let Some((ptr, n)) = super::raiz() else {
            unsafe { ACTUAL = None; HONDO = 0; CUANTAS = 0; }
            return false;
        };
        unsafe {
            PILA[0] = Some(ptr);
            HONDO = 0;
            ACTUAL = Some(n);
        }
        relistar()
    }

    /// Cuantos hijos tiene el nodo actual.
    pub fn hijos() -> u64 {
        unsafe { CUANTAS as u64 }
    }

    /// 1 si el listado no cabia entero. **Se dice en vez de callarse**: un
    /// directorio truncado en silencio se ve igual que uno corto.
    pub fn truncado() -> u64 {
        unsafe { TRUNCADO as u64 }
    }

    /// Cuantos niveles se ha bajado desde la raiz.
    pub fn hondo() -> u64 {
        unsafe { HONDO as u64 }
    }

    /// El tipo del nodo actual: 0 archivo, 1 directorio, 2 no hay nada.
    pub fn tipo() -> u64 {
        unsafe {
            match ACTUAL {
                Some(n) => if n.tipo == Tipo::Directorio { 1 } else { 0 },
                None => 2,
            }
        }
    }

    pub(crate) fn entry_i(i: usize) -> Option<bmo_estratos::objects::Entrada> {
        unsafe {
            if i >= CUANTAS { return None; }
            let buf = &*core::ptr::addr_of!(BUF);
            bmo_estratos::objects::Entrada::decode(&buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
        }
    }

    /// El tipo del hijo `i`: 0 archivo, 1 directorio, 2 no se pudo leer.
    ///
    /// Cuesta un salto al disco porque el tipo vive en el NODO y la entrada
    /// solo guarda el nombre y a donde apunta. Es lo que hay: meter el tipo en
    /// la entrada seria duplicar un dato que el nodo ya tiene, y dos copias de
    /// un dato es una que puede mentir.
    pub fn hijo_tipo(i: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 2 };
        match super::nodo(&e.nodo) {
            Some(n) => if n.tipo == Tipo::Directorio { 1 } else { 0 },
            None => 2,
        }
    }

    /// Ocho bytes del nombre del hijo `i`, empaquetados en LE. `trozo` los
    /// numera. Es el mismo trato que `klog_texto`: la superficie no acepta
    /// punteros, asi que un nombre viaja de ocho en ocho.
    pub fn child_name(i: usize, trozo: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 0 };
        let name = e.nombre_str().as_bytes();
        let ini = trozo * 8;
        if ini >= name.len() { return 0; }
        let fin = (ini + 8).min(name.len());
        let mut w = [0u8; 8];
        w[..fin - ini].copy_from_slice(&name[ini..fin]);
        u64::from_le_bytes(w)
    }

    /// Baja al hijo `i`. `false` si no existe, si no es directorio, o si ya no
    /// se puede bajar mas.
    pub fn entrar(i: usize) -> bool {
        let Some(e) = entry_i(i) else { return false };
        unsafe {
            if HONDO + 1 >= HONDO_MAX { return false; }
        }
        let Some(n) = super::nodo(&e.nodo) else { return false };
        if n.tipo != Tipo::Directorio { return false; }
        unsafe {
            HONDO += 1;
            PILA[HONDO] = Some(e.nodo);
            // El nombre se anota AL PASAR. Despues ya no se sabe: la entrada
            // que lo lleva es del padre y aqui ya no la tenemos delante.
            let b = e.nombre_str().as_bytes();
            let k = b.len().min(LEVEL_NAME);
            NAMES[HONDO][..k].copy_from_slice(&b[..k]);
            NAMES_LEN[HONDO] = k;
            ACTUAL = Some(n);
        }
        relistar()
    }

    /// Ocho bytes del nombre del nivel `nivel` de la ruta. `nivel = 0` es la
    /// raiz, que no tiene nombre y contesta vacio -- la ventana pinta `/`.
    pub fn level_name(nivel: usize, trozo: usize) -> u64 {
        unsafe {
            if nivel == 0 || nivel > HONDO || nivel >= HONDO_MAX {
                return 0;
            }
            let n = NAMES_LEN[nivel];
            let ini = trozo * 8;
            if ini >= n {
                return 0;
            }
            let fin = (ini + 8).min(n);
            let mut w = [0u8; 8];
            w[..fin - ini].copy_from_slice(&NAMES[nivel][ini..fin]);
            u64::from_le_bytes(w)
        }
    }

    // -- El DETALLE de un hijo -------------------------------------------
    //
    // * Un grafo que solo ensena nombres contesta *que hay*; no contesta *que
    // es esto*. Lo de abajo es lo que el nodo ya lleva dentro y la ventana no
    // podia pedir: cuanto mide, cuantos atributos tiene y si va firmado.

    /// Bytes del contenido del hijo `i`. Un directorio contesta el tamano de su
    /// lista de entries, que tambien es un dato: dice cuanto ocupa el propio
    /// directorio, no lo que hay dentro.
    pub fn hijo_bytes(i: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        let cual = if n.tipo == Tipo::Directorio {
            bmo_estratos::objects::ATTR_ENTRADAS
        } else {
            bmo_estratos::objects::ATTR_DATOS
        };
        n.attr(cual).map(|a| a.size).unwrap_or(0)
    }

    /// Cuantos atributos lleva el hijo `i`.
    ///
    /// Es el numero que dice que ESTRATOS no es un sistema de archivos de
    /// carpetas: un nodo es **un conjunto de atributos**, y la diferencia entre
    /// un archivo y un directorio es cual lleva, no dos estructuras distintas.
    pub fn hijo_atributos(i: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        n.attrs().count() as u64
    }

    /// Lleva `:firma` el hijo `i`? `1` si, `0` no.
    ///
    /// **Solo dice si LA LLEVA, no si cuadra.** Comprobarlo exige leer el
    /// contenido entero y hacerle el BLAKE3, y eso no puede pasar en cada
    /// repintado de una ventana. Para eso esta [`verify`], que se pide.
    pub fn hijo_firmado(i: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 0 };
        let Some(n) = super::nodo(&e.nodo) else { return 0 };
        n.attr(bmo_estratos::objects::ATTR_FIRMA).is_some() as u64
    }

    /// El buffer donde se lee un archivo para verificarlo. Un tope honesto:
    /// mas grande que esto no se puede comprobar y **se dice** en vez de
    /// contestar "no cuadra", que mandaria a buscar una corrupcion que no hay.
    const VERIFY_MAX: usize = 256 * 1024;
    static mut VERIFY_BUF: [u8; VERIFY_MAX] = [0u8; VERIFY_MAX];

    /// **Lee el hijo `i` y compara su BLAKE3 con su `:firma`.**
    ///
    /// `0` no lleva firma - `1` CUADRA - `2` NO CUADRA - `3` no se pudo leer -
    /// `4` **no cabe** en el buffer de verificacion.
    ///
    /// * El `4` no estaba y hacia falta: "no cabe" contestaba `3`, o sea **el
    /// mismo codigo que un fallo de lectura**. El panel lo pintaba en rojo como
    /// *"no se pudo leer"*, y en esa ventana el rojo significa "hay un problema
    /// en el disco". Un archivo sano de 300 KiB acusaba al disco de una averia
    /// que no existia. El tope es NUESTRO y ahora lo dice el.
    ///
    /// Se pide a mano y no se calcula al pintar: leer un archivo entero y
    /// hacerle un hash sesenta veces por segundo convertiria un panel en un
    /// martillo sobre el disco.
    ///
    /// [!] Lo que esto demuestra y lo que no, dicho aqui como en `super::firma`:
    /// demuestra que **los bytes son los que se guardaron** --caza corrupcion,
    /// una escritura a medias, un bloque mal leido--. NO demuestra
    /// autenticidad: quien pueda escribir en el volumen puede cambiar el
    /// archivo *y* recalcular su hash.
    pub fn verify(i: usize) -> u64 {
        let Some(e) = entry_i(i) else { return 3 };
        let Some(n) = super::nodo(&e.nodo) else { return 3 };
        if n.attr(bmo_estratos::objects::ATTR_FIRMA).is_none() {
            return 0;
        }
        let a = match n.attr(bmo_estratos::objects::ATTR_DATOS) {
            Some(a) => a,
            None => return 3,
        };
        if a.size as usize > VERIFY_MAX {
            return 4; // no cabe -- el limite es nuestro, no del disco
        }
        let buf = unsafe { &mut *core::ptr::addr_of_mut!(VERIFY_BUF) };
        let leidos = match super::flujo(a, buf) {
            Some(k) => k,
            None => return 3,
        };
        match super::firma(&n, &buf[..leidos]) {
            super::Firma::Cuadra => 1,
            super::Firma::NoCuadra => 2,
            super::Firma::Ausente => 0,
        }
    }

    /// Vuelve al padre. `false` si ya se esta en la raiz.
    pub fn subir() -> bool {
        unsafe {
            if HONDO == 0 { return false; }
            HONDO -= 1;
            let Some(ptr) = PILA[HONDO] else { return false };
            let Some(n) = super::nodo(&ptr) else { return false };
            ACTUAL = Some(n);
        }
        relistar()
    }
}

/// Busca un hijo por nombre dentro de un directorio, sin distinguir mayusculas.
pub(crate) fn buscar_en(dir: &Nodo, name: &str) -> Option<BlockPtr> {
    let (n, _) = entries(dir)?;
    for i in 0..n {
        let e = entrada(i)?;
        if e.se_llama(name) { return Some(e.nodo); }
    }
    None
}

/// Busca un nodo por ruta: `c/holac.bex`.
pub fn open(ruta: &str) -> Option<Nodo> {
    let (_, mut actual) = raiz()?;
    let mut resto = ruta.trim_start_matches('/');
    loop {
        match resto.as_bytes().iter().position(|&c| c == b'/' || c == b'\\') {
            Some(i) => {
                let ptr = buscar_en(&actual, &resto[..i])?;
                let n = nodo(&ptr)?;
                if n.tipo != Tipo::Directorio { return None; }
                actual = n;
                resto = &resto[i + 1..];
            }
            None => break,
        }
    }
    let ptr = buscar_en(&actual, resto)?;
    nodo(&ptr)
}

/// Lee el `:datos` de un nodo. Devuelve los bytes leidos.
pub fn read(n: &Nodo, dst: &mut [u8]) -> Option<usize> {
    if n.tipo != Tipo::Archivo { return None; }
    let a = n.attr(bmo_estratos::objects::ATTR_DATOS)?;
    flujo(a, dst)
}

/// Que dijo la firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firma {
    /// El `:firma` del nodo cuadra con el contenido leido.
    Cuadra,
    /// Hay `:firma` y NO cuadra: el archivo no es el que se guardo.
    NoCuadra,
    /// El nodo no lleva `:firma`.
    Ausente,
}

/// El gate del section 7: `open(nodo, EJECUTAR)`.
///
/// Compara el atributo `:firma` con el BLAKE3 del contenido que se acaba de
/// leer. **Lo que esto demuestra**: que los bytes son los que se guardaron --
/// caza corrupcion del disco, una escritura a medias o un bloque mal leido.
///
/// **Lo que NO demuestra**: autenticidad. Quien pueda escribir en el volumen
/// puede cambiar el archivo *y* recalcular su hash; no hay clave por medio.
/// Para eso hace falta firmar el hash con una clave que el kernel conozca y el
/// atacante no (esqueleto en `bmo-abi/src/bef/signing.rs`). Se dice en vez de
/// dejar que la palabra "firma" prometa de mas.
///
/// Y esto es lo que un `.bex` en FAT32 **no puede tener**: un sistema de
/// ficheros sin atributos con nombre obliga a un `.sig` suelto que se pierde
/// al copiar. Aqui la firma viaja dentro del mismo nodo que los datos.
pub fn firma(n: &Nodo, datos: &[u8]) -> Firma {
    let a = match n.attr(bmo_estratos::objects::ATTR_FIRMA) {
        Some(a) => a,
        None => return Firma::Ausente,
    };
    let guardada = match a.datos_residentes() {
        Some(d) if d.len() == 32 => d,
        _ => return Firma::Ausente,
    };
    if bmo_estratos::blake3(datos) == guardada { Firma::Cuadra } else { Firma::NoCuadra }
}
