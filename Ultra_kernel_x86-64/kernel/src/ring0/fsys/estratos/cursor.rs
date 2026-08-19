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
    use super::nivel::{Detalle, Nivel};
    use super::*;

    pub use super::nivel::LEVEL_NAME;

    /// Cuanto se puede bajar. Dieciseis niveles de directorio es mas de lo que
    /// tiene ningun volumen razonable, y un tope explicito es mejor que una
    /// pila que crece hasta que algo se rompe.
    pub const HONDO_MAX: usize = 16;

    /// **La ruta desde la raiz, con el listado de CADA nivel.**
    ///
    /// `PILA[0]` es la raiz y `PILA[HONDO]` donde estas. Lo que cambio es que
    /// los de en medio **ya no estan vacios**: cada uno se queda con su nodo y
    /// con lo que se leyo al pasar por el. El porque, y lo que cuesta, en la
    /// cabecera de `nivel.rs`.
    ///
    /// [!] Ya no hay un `ACTUAL` aparte. Lo hubo, y era el mismo nodo que
    /// `PILA[HONDO]` guarda ahora: dos copias del mismo dato, y una de las dos
    /// podia quedarse atras.
    static mut PILA: [Nivel; HONDO_MAX] = [const { Nivel::VACIO }; HONDO_MAX];
    static mut HONDO: usize = 0;

    fn pila() -> &'static mut [Nivel; HONDO_MAX] {
        unsafe { &mut *core::ptr::addr_of_mut!(PILA) }
    }

    /// El nivel `k`, o `None` si esta por debajo de donde estamos.
    ///
    /// * Preguntar por un nivel mas hondo que `HONDO` no es un error del
    /// llamante: el panel de arbol pinta de arriba abajo y se entera de donde
    /// acaba la rama justo asi. Se contesta "no hay" y se sigue.
    fn nivel_k(k: usize) -> Option<&'static Nivel> {
        unsafe {
            if k > HONDO || k >= HONDO_MAX {
                return None;
            }
            Some(&(*core::ptr::addr_of!(PILA))[k])
        }
    }

    /// El nivel donde esta el cursor.
    fn aqui() -> &'static Nivel {
        unsafe { &(*core::ptr::addr_of!(PILA))[HONDO] }
    }

    /// Pone el cursor en la raiz del volumen. `false` si no hay volumen.
    pub fn a_la_raiz() -> bool {
        unsafe { HONDO = 0 };
        let raiz = super::raiz();
        let nv = &mut pila()[0];
        nv.nombre_len = 0;
        nv.elegido = None;
        match raiz {
            Some((ptr, n)) => {
                nv.ptr = Some(ptr);
                nv.nodo = Some(n);
                nv.listar()
            }
            None => {
                nv.ptr = None;
                nv.nodo = None;
                nv.listar();
                false
            }
        }
    }

    /// Cuantos hijos tiene el nodo actual.
    pub fn hijos() -> u64 {
        aqui().cuantas as u64
    }

    /// 1 si el listado no cabia entero. **Se dice en vez de callarse**: un
    /// directorio truncado en silencio se ve igual que uno corto.
    pub fn truncado() -> u64 {
        aqui().truncado as u64
    }

    /// Cuantos niveles se ha bajado desde la raiz.
    pub fn hondo() -> u64 {
        unsafe { HONDO as u64 }
    }

    /// El tipo del nodo actual: 0 archivo, 1 directorio, 2 no hay nada.
    pub fn tipo() -> u64 {
        match aqui().nodo {
            Some(n) => {
                if n.tipo == Tipo::Directorio {
                    1
                } else {
                    0
                }
            }
            None => 2,
        }
    }

    pub(crate) fn entry_i(i: usize) -> Option<bmo_estratos::objects::Entrada> {
        aqui().entrada(i)
    }

    /// El tipo del hijo `i`: 0 archivo, 1 directorio, 2 no se pudo leer.
    ///
    /// * **Ya no salta al disco.** El tipo vive en el nodo del hijo y la entrada
    /// del directorio solo guarda el nombre y a donde apunta, asi que esto era
    /// una lectura de bloque -- por fila y por repintado. Ahora se leyo al
    /// listar el nivel. Ver la cabecera de `nivel.rs`.
    pub fn hijo_tipo(i: usize) -> u64 {
        aqui().detalle(i).tipo as u64
    }

    /// Ocho bytes del nombre del hijo `i`, empaquetados en LE. `trozo` los
    /// numera. Es el mismo trato que `klog_texto`: la superficie no acepta
    /// punteros, asi que un nombre viaja de ocho en ocho.
    pub fn child_name(i: usize, trozo: usize) -> u64 {
        nombre_de(aqui(), i, trozo)
    }

    /// El motor de los dos nombres de hijo: el del nivel actual y el de
    /// cualquier nivel. Un solo sitio donde se parte un nombre en trozos.
    fn nombre_de(nv: &Nivel, i: usize, trozo: usize) -> u64 {
        let Some(e) = nv.entrada(i) else { return 0 };
        let name = e.nombre_str().as_bytes();
        let ini = trozo * 8;
        if ini >= name.len() {
            return 0;
        }
        let fin = (ini + 8).min(name.len());
        let mut w = [0u8; 8];
        w[..fin - ini].copy_from_slice(&name[ini..fin]);
        u64::from_le_bytes(w)
    }

    /// Baja al hijo `i`. `false` si no existe, si no es directorio, o si ya no
    /// se puede bajar mas.
    pub fn entrar(i: usize) -> bool {
        let Some(e) = entry_i(i) else { return false };
        let h = unsafe { HONDO };
        if h + 1 >= HONDO_MAX {
            return false;
        }
        // Que sea directorio sale del detalle que ya se leyo: bajar no vuelve a
        // preguntarle al disco algo que se supo al listar. El NODO si hay que
        // traerlo -- es el que se va a listar ahora.
        if aqui().detalle(i).tipo != 1 {
            return false;
        }
        let Some(n) = super::nodo(&e.nodo) else { return false };
        // Por donde se bajo, anotado en el nivel de ARRIBA: es el dato con el
        // que el panel de arbol sabe que rama esta abierta.
        pila()[h].elegido = Some(i);
        unsafe { HONDO = h + 1 };
        let nv = &mut pila()[h + 1];
        nv.ptr = Some(e.nodo);
        nv.nodo = Some(n);
        nv.elegido = None;
        // El nombre se anota AL PASAR. Despues ya no se sabe: la entrada que lo
        // lleva es del padre y aqui ya no la tenemos delante.
        nv.nombrar(e.nombre_str().as_bytes());
        nv.listar()
    }

    /// Vuelve al padre. `false` si ya se esta en la raiz.
    ///
    /// * **Ya no toca el disco.** Antes subir traia otra vez el nodo del padre
    /// y volvia a listarlo entero; ahora el nivel de arriba sigue leido desde
    /// que se paso por el, asi que subir es restar uno. Es la propiedad que
    /// permite recorrer un arbol con el raton sin que el disco se entere.
    pub fn subir() -> bool {
        unsafe {
            if HONDO == 0 {
                return false;
            }
            HONDO -= 1;
            pila()[HONDO].elegido = None;
        }
        true
    }

    /// **Vuelve a leer el arbol y se queda DONDE ESTABA.**
    ///
    /// === Por que hace falta, y por que es una sorpresa si no esta ===
    ///
    /// Cada nivel guarda su nodo y su listado desde que se paso por el. Eso es
    /// lo que hace que pintar el arbol no toque el disco -- y es exactamente lo
    /// que lo deja MINTIENDO en cuanto alguien escribe: el volumen tiene un
    /// estrato nuevo y la pila sigue apuntando al de antes.
    ///
    /// El sintoma no seria un error. Seria borrar un fichero y verlo ahi.
    ///
    /// === Por que se rehace el CAMINO y no solo la raiz ===
    ///
    /// Volver a la raiz seria correcto y seria molesto: escribes en
    /// `/datos/notas` y te devuelve arriba en cada gesto. Aqui se guardan los
    /// nombres, se baja a la raiz nueva y se vuelve a bajar por ellos.
    ///
    /// ** Y si un tramo ya no existe --porque lo que se acaba de borrar era la
    /// carpeta donde estabas-- **se para ahi**. No es un fallo: es el sitio mas
    /// hondo que sigue existiendo, que es donde uno quiere quedarse.
    pub fn recargar() -> bool {
        // El camino se copia ANTES de tocar la pila: `a_la_raiz` la reescribe.
        let hondo = unsafe { HONDO };
        let mut camino = [[0u8; LEVEL_NAME]; HONDO_MAX];
        let mut largos = [0usize; HONDO_MAX];
        for k in 1..=hondo {
            if let Some(nv) = nivel_k(k) {
                camino[k] = nv.nombre;
                largos[k] = nv.nombre_len;
            }
        }
        if !a_la_raiz() {
            return false;
        }
        for k in 1..=hondo {
            let n = largos[k];
            if n == 0 {
                break;
            }
            let Ok(nombre) = core::str::from_utf8(&camino[k][..n]) else { break };
            // Se busca por NOMBRE y no por indice: el indice de ayer puede ser
            // otra cosa hoy -- justo lo que pasa al quitar una entrada de en
            // medio, que es la operacion que mas veces va a llamar aqui.
            let mut i = 0usize;
            let mut encontrado = false;
            while i < aqui().cuantas {
                if let Some(e) = aqui().entrada(i) {
                    if e.se_llama(nombre) {
                        encontrado = entrar(i);
                        break;
                    }
                }
                i += 1;
            }
            if !encontrado {
                break;
            }
        }
        true
    }

    /// Ocho bytes del nombre del nivel `nivel` de la ruta. `nivel = 0` es la
    /// raiz, que no tiene nombre y contesta vacio -- la ventana pinta `/`.
    pub fn level_name(nivel: usize, trozo: usize) -> u64 {
        if nivel == 0 {
            return 0;
        }
        let Some(nv) = nivel_k(nivel) else { return 0 };
        let n = nv.nombre_len;
        let ini = trozo * 8;
        if ini >= n {
            return 0;
        }
        let fin = (ini + 8).min(n);
        let mut w = [0u8; 8];
        w[..fin - ini].copy_from_slice(&nv.nombre[ini..fin]);
        u64::from_le_bytes(w)
    }

    // -- ** EL ARBOL: preguntar por un nivel que NO es donde estas ----------
    //
    // Las cuatro de abajo son lo unico que el panel de arbol necesita y que
    // antes no se podia pedir. Ninguna lee nada nuevo: contestan desde el
    // listado que cada nivel guarda desde que se paso por el.
    //
    // * Y no hace falta ni una operacion mas. Con "cuantos hay", "como se
    // llama", "que es" y "por cual se bajo" se pinta el arbol entero. La
    // tentacion era exponer cada nivel como un cursor secundario -- diez
    // operaciones y una tabla de handles para lo que hacen cuatro preguntas.

    /// Cuantos hijos tiene el nivel `nivel`. `0` si ese nivel no existe.
    pub fn nivel_hijos(nivel: usize) -> u64 {
        nivel_k(nivel).map(|nv| nv.cuantas as u64).unwrap_or(0)
    }

    /// El tipo del hijo `i` del nivel `nivel`. `2` si no hay tal cosa.
    pub fn nivel_hijo_tipo(nivel: usize, i: usize) -> u64 {
        nivel_k(nivel)
            .map(|nv| nv.detalle(i).tipo as u64)
            .unwrap_or(Detalle::ILEGIBLE.tipo as u64)
    }

    /// Por que hijo se bajo desde el nivel `nivel`.
    ///
    /// `u64::MAX` es "por ninguno", y marca el nivel donde acaba la rama
    /// abierta. Se contesta con un valor imposible y no con cero porque **cero
    /// es un hijo perfectamente valido** -- el primero.
    pub fn nivel_elegido(nivel: usize) -> u64 {
        match nivel_k(nivel).and_then(|nv| nv.elegido) {
            Some(i) => i as u64,
            None => u64::MAX,
        }
    }

    /// Ocho bytes del nombre del hijo `i` del nivel `nivel`.
    pub fn nivel_child_name(nivel: usize, i: usize, trozo: usize) -> u64 {
        match nivel_k(nivel) {
            Some(nv) => nombre_de(nv, i, trozo),
            None => 0,
        }
    }

    // -- El DETALLE de un hijo -------------------------------------------
    //
    // * Un grafo que solo ensena nombres contesta *que hay*; no contesta *que
    // es esto*. Lo de abajo es lo que el nodo ya lleva dentro y la ventana no
    // podia pedir: cuanto mide, cuantos atributos tiene y si va firmado.
    //
    // Los tres se leyeron al listar el nivel y aqui solo se sacan. Antes cada
    // uno era una lectura de bloque, por fila y en cada repintado.

    /// Bytes del contenido del hijo `i`. Un directorio contesta el tamano de su
    /// lista de entries, que tambien es un dato: dice cuanto ocupa el propio
    /// directorio, no lo que hay dentro.
    pub fn hijo_bytes(i: usize) -> u64 {
        aqui().detalle(i).bytes
    }

    /// Cuantos atributos lleva el hijo `i`.
    ///
    /// Es el numero que dice que ESTRATOS no es un sistema de archivos de
    /// carpetas: un nodo es **un conjunto de atributos**, y la diferencia entre
    /// un archivo y un directorio es cual lleva, no dos estructuras distintas.
    pub fn hijo_atributos(i: usize) -> u64 {
        aqui().detalle(i).atributos as u64
    }

    /// Lleva `:firma` el hijo `i`? `1` si, `0` no.
    ///
    /// **Solo dice si LA LLEVA, no si cuadra.** Comprobarlo exige leer el
    /// contenido entero y hacerle el BLAKE3, y eso no puede pasar en cada
    /// repintado de una ventana. Para eso esta [`verify`], que se pide.
    pub fn hijo_firmado(i: usize) -> u64 {
        aqui().detalle(i).firmado as u64
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
