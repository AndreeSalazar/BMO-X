//! **FOLLOWING THE POINTERS** -- how ESTRATOS reads a chain of blocks.
//!
//! === Why this is a file of its own ===
//!
//! Because it is the part with no policy in it at all: given a block number,
//! bring the block; given a node, follow it. Every decision --what to mount,
//! what to seal, what is writable-- lives in the parent module, and everything
//! here just walks.
//!
//! That separation is worth having in a copy-on-write filesystem specifically:
//! **walking never writes**, so a bug in this file cannot corrupt a volume, and
//! knowing that shrinks the surface that has to be trusted.

use super::*;

// -- Seguir punteros ---------------------------------------------------------

/// Lee lo que un puntero promete y **lo comprueba**.
///
/// Devuelve los bytes dentro del scratch del nivel indicado. Un bloque que no
/// cuadra con su suma es un FAULT en CABINA, no un archivo raro: es el
/// principio 2 del diseno y la unica razon de que el puntero lleve la suma.
pub(crate) fn seguir(p: &BlockPtr, nivel: usize) -> Option<&'static [u8]> {
    if nivel >= NIVELES { return None; }
    let buf = unsafe {
        let s = core::ptr::addr_of_mut!(SCRATCH) as *mut [u8; BLOQUE];
        &mut *s.add(nivel)
    };
    if !read_block(p.lba, buf) {
        crate::ring0::cabina::fault("estratos", "no se pudo leer un bloque", p.lba);
        return None;
    }
    let ini = p.off as usize;
    let fin = ini + p.len as usize;
    if fin > BLOQUE { return None; }
    let datos = &buf[ini..fin];
    if !p.verifica(datos) {
        crate::ring0::cabina::fault("estratos", "un bloque no cuadra con su suma", p.lba);
        return None;
    }
    Some(unsafe { core::slice::from_raw_parts(datos.as_ptr(), datos.len()) })
}

/// Lee el nodo al que apunta `p`.
pub fn nodo(p: &BlockPtr) -> Option<Nodo> {
    let d = seguir(p, 0)?;
    match Nodo::decode(d) {
        Ok(n) => Some(n),
        Err(e) => { crate::ring0::cabina::fault("estratos", e.name(), p.lba); None }
    }
}

/// El disco visto como fuente de bloques para el recorrido compartido.
pub(crate) struct DelDisco;

impl es::Fuente for DelDisco {
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
        read_block(lba, dst)
    }
}

/// Reconstruye un flujo entero en `dst`. Devuelve los bytes escritos.
///
/// El recorrido del arbol NO vive aqui: es `bmo_estratos::descender`, el mismo
/// que usa el formateador del anfitrion. Tenerlo dos veces --una en cada lado--
/// era la trampa que casi cuesta el BLAKE3: dos copias que pueden separarse, y
/// el sintoma seria "un archivo que se lee mal" sin nada que apunte al
/// recorrido. Aqui solo se pone el disco, la memoria de trabajo y donde cae.
pub fn flujo(a: &Attr, dst: &mut [u8]) -> Option<usize> {
    if let Some(d) = a.datos_residentes() {
        if dst.len() < d.len() { return None; }
        dst[..d.len()].copy_from_slice(d);
        return Some(d.len());
    }
    let raiz = a.raiz()?;

    // El nivel 0 del scratch lo usa `seguir()` para nodos y estratos; el
    // recorrido se queda con los de abajo para no pisarselo.
    let scratch = unsafe {
        let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
        &mut s[1..]
    };

    let mut escritos = 0usize;
    let r = es::descender(&mut DelDisco, &raiz, a.levels, scratch, &mut |trozo| {
        let free_slot = dst.len().saturating_sub(escritos);
        let n = trozo.len().min(free_slot);
        if n > 0 {
            dst[escritos..escritos + n].copy_from_slice(&trozo[..n]);
            escritos += n;
        }
        // Parar en cuanto el buffer del llamante se llena: seguir leyendo
        // bloques que no caben es gastar disco para tirar los bytes.
        escritos < dst.len()
    });
    if let Err(e) = r {
        crate::ring0::cabina::fault("estratos", e.name(), raiz.lba);
        return None;
    }
    Some((a.size as usize).min(escritos))
}

/// **Lee el principio del archivo Y comprueba su firma, en UNA sola pasada.**
///
/// Devuelve `(copiados, tamano_real, veredicto)`.
///
/// === El problema que resuelve, que es el escalon 2 entero ===
///
/// El gate de firma compara el `:firma` del nodo contra el hash del CONTENIDO,
/// y hasta hoy eso obligaba a tener el contenido entero en RAM: `read` a un
/// buffer y `firma(&nd, buf)`. Con un paquete que lleva un WAD dentro, eso son
/// megabytes de bodega para comprobar unos pocos que se van a ejecutar.
///
/// ** Pero un hash no necesita el fichero: necesita **sus bytes, en orden**. Y
/// eso es exactamente lo que `descender` ya entrega, trozo a trozo. Asi que los
/// trozos pasan por el hasher **todos**, y de ellos solo se COPIA lo que cabe en
/// `dst`. La firma sigue cubriendo el archivo entero -- el gate no se relaja ni
/// un byte-- y la RAM solo guarda el principio.
///
/// > **Los bytes pasan por delante; no se quedan.**
///
/// === Por que una funcion y no dos ===
///
/// Porque leer dos veces el mismo archivo --una para copiar y otra para hashear--
/// es pagar el disco dos veces por un dato que ya paso por aqui. Y porque son
/// dos respuestas de la MISMA pasada: separarlas invita a comprobar la firma de
/// una lectura y a usar los bytes de otra.
pub fn read_and_sign(n: &Nodo, dst: &mut [u8]) -> Option<(usize, usize, Firma)> {
    if n.tipo != Tipo::Archivo {
        return None;
    }
    let a = n.attr(bmo_estratos::objects::ATTR_DATOS)?;

    // Lo que dice el `:firma`, si lo hay. Se mira ANTES de leer para no gastar
    // el disco en un archivo que el gate va a rechazar de todas formas.
    let guardada = match n.attr(bmo_estratos::objects::ATTR_FIRMA) {
        Some(f) => match f.datos_residentes() {
            Some(d) if d.len() == 32 => {
                let mut copia = [0u8; 32];
                copia.copy_from_slice(d);
                Some(copia)
            }
            _ => None,
        },
        None => None,
    };

    let mut h = bmo_estratos::Hasher::new();
    let mut copiados = 0usize;
    let mut vistos = 0usize;

    if let Some(d) = a.datos_residentes() {
        // Residente: cabe en el propio nodo, asi que ya esta en RAM. No hay
        // nada que ahorrar -- y aun asi va por el mismo camino, para que el
        // veredicto se calcule en un solo sitio.
        h.update(d);
        vistos = d.len();
        copiados = d.len().min(dst.len());
        dst[..copiados].copy_from_slice(&d[..copiados]);
    } else {
        let raiz = a.raiz()?;
        let scratch = unsafe {
            let s = &mut *core::ptr::addr_of_mut!(SCRATCH);
            &mut s[1..]
        };
        let tam = a.size as usize;
        let r = es::descender(&mut DelDisco, &raiz, a.levels, scratch, &mut |trozo| {
            // ** El hasher se come SOLO lo que el archivo mide. `descender`
            // entrega bloques enteros y el ultimo lleva relleno detras: hashear
            // ese relleno daria un digest que no cuadra con el que escribio
            // quien firmo, y el sintoma seria "la firma NO cuadra" en TODOS los
            // archivos cuyo tamano no sea multiplo del bloque.
            let util = trozo.len().min(tam.saturating_sub(vistos));
            if util == 0 {
                return false;
            }
            h.update(&trozo[..util]);
            vistos += util;
            let hueco = dst.len().saturating_sub(copiados);
            let n = util.min(hueco);
            if n > 0 {
                dst[copiados..copiados + n].copy_from_slice(&trozo[..n]);
                copiados += n;
            }
            // * Se sigue leyendo AUNQUE `dst` este lleno, al reves que `flujo`.
            // Parar ahi seria dejar el hash a medias, y un hash a medias no es
            // una firma mas barata: es una firma que no vale.
            vistos < tam
        });
        if let Err(e) = r {
            crate::ring0::cabina::fault("estratos", e.name(), raiz.lba);
            return None;
        }
    }

    let veredicto = match guardada {
        None => Firma::Ausente,
        Some(g) => {
            if h.finalize() == g {
                Firma::Cuadra
            } else {
                Firma::NoCuadra
            }
        }
    };
    Some((copiados, vistos, veredicto))
}
