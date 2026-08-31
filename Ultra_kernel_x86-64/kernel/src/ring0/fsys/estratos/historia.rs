//! **LA HISTORIA DEL VOLUMEN**: la cadena de versiones, hacia atras.
//!
//! [carril]  VERDE     la cadena de versiones, hacia atras
//!
//! === Que hay que leer, y por que no hacia falta formato nuevo ===
//!
//! Cada estrato guarda un puntero a su PADRE. Recorrer esa cadena **es**
//! recorrer la historia -- y por eso el padre es un puntero y no un hash, que ya
//! esta escrito al lado del campo: *montar un estrato viejo es encontrarlo, no
//! reconocerlo*.
//!
//! O sea que la historia existia en el disco desde el primer dia y no habia
//! puerta. Esto es esa puerta, igual que el cursor lo fue para el arbol.
//!
//! === ** SE LEE UNA VEZ Y SE GUARDA ===
//!
//! Cada paso hacia atras es **una lectura de bloque**. Un panel con veinte
//! versiones a la vista que las releyera en cada repintado serian doscientas
//! lecturas por mover el raton -- que es exactamente el martillo sobre el disco
//! que ya costo el detalle de cada hijo del cursor.
//!
//! Asi que se recorre a mano ([`releer`]) y lo demas contesta de memoria. Se
//! pide al abrir la pestana y despues de escribir, que son los dos momentos en
//! los que la historia cambia.
//!
//! === Lo que se guarda de cada version, y lo que no ===
//!
//! ```text
//!   cuando    la fecha. `0` = sin fechar (ver `clock::ahora`)
//!   quien     el pid que la hizo
//!   nombre    vacio en las automaticas; puesto es PERMANENTE
//! ```
//!
//! No se guarda la raiz. Aqui se contesta *que paso y cuando*; **volver** a una
//! version es otra operacion, y la que la escriba tendra que recorrer la cadena
//! ella misma -- pedirle a un panel que guarde punteros a arboles seria darle a
//! la vista la llave del disco.
//!
//! [!] Y el TOPE es real: se guardan las [`MAX`] mas recientes. Un volumen con
//! mil versiones no cabe en un panel ni en `.bss`, y **se dice** en vez de
//! ensenar las veinte primeras como si fueran todas.

use bmo_estratos as es;
use bmo_estratos::objects::BlockPtr;

/// Cuantas versiones se guardan. Treinta y dos entran de sobra en cualquier
/// panel y son menos de 3 KiB de `.bss`.
pub const MAX: usize = 32;
/// Lo que cabe de un nombre. El mismo tope que el formato le da al motivo.
pub const NOMBRE_MAX: usize = 64;

#[derive(Clone, Copy)]
pub struct Version {
    pub cuando: u64,
    pub quien: u32,
    pub nombre: [u8; NOMBRE_MAX],
    pub nombre_len: usize,
}

impl Version {
    const VACIA: Self = Self { cuando: 0, quien: 0, nombre: [0; NOMBRE_MAX], nombre_len: 0 };
}

static mut VERSIONES: [Version; MAX] = [Version::VACIA; MAX];
static mut CUANTAS: usize = 0;
/// Se corto el recorrido por el tope? Se dice: una historia recortada en
/// silencio se ve igual que un volumen joven.
static mut RECORTADA: bool = false;

/// **Recorre la cadena hacia atras y guarda lo que encuentra.**
///
/// Devuelve cuantas versiones se pudieron leer. `VERSIONES[0]` es la de ahora.
///
/// Un puntero nulo es el final --el primer estrato del volumen-- y **no es un
/// error**. Un bloque que no se puede leer si lo es, y ahi se para: seguir
/// adivinando pintaria una historia que el disco no tiene.
pub fn releer() -> usize {
    let versiones = unsafe { &mut *core::ptr::addr_of_mut!(VERSIONES) };
    let mut n = 0usize;
    let mut recortada = false;

    let Some(sb) = super::superbloque() else {
        unsafe {
            CUANTAS = 0;
            RECORTADA = false;
        }
        return 0;
    };
    let mut donde: BlockPtr = sb.estrato;
    while n < MAX {
        if donde.es_nulo() {
            break;
        }
        let Some(d) = super::seguir(&donde, 0) else { break };
        let Ok(e) = es::Estrato::decode(d) else { break };
        let motivo = e.motivo_str().as_bytes();
        let k = motivo.len().min(NOMBRE_MAX);
        versiones[n].cuando = e.tiempo;
        versiones[n].quien = match e.autor {
            es::Autor::Proceso(p) => p,
            _ => 0,
        };
        versiones[n].nombre[..k].copy_from_slice(&motivo[..k]);
        versiones[n].nombre_len = k;
        n += 1;
        donde = e.padre;
        if n == MAX && !donde.es_nulo() {
            recortada = true;
        }
    }
    unsafe {
        CUANTAS = n;
        RECORTADA = recortada;
    }
    n
}

pub fn cuantas() -> u64 {
    unsafe { CUANTAS as u64 }
}

/// Se corto el recorrido por el tope?
pub fn recortada() -> u64 {
    unsafe { RECORTADA as u64 }
}

fn version(i: usize) -> Option<Version> {
    unsafe {
        if i >= CUANTAS {
            return None;
        }
        Some((*core::ptr::addr_of!(VERSIONES))[i])
    }
}

/// Cuando se hizo la version `i`. `0` = sin fechar.
pub fn cuando(i: usize) -> u64 {
    version(i).map_or(0, |v| v.cuando)
}

/// Que proceso la hizo.
pub fn quien(i: usize) -> u64 {
    version(i).map_or(0, |v| v.quien as u64)
}

/// Lleva nombre? Las que si son PERMANENTES: el recolector no las suelta.
pub fn con_nombre(i: usize) -> u64 {
    version(i).map_or(0, |v| (v.nombre_len > 0) as u64)
}

/// Ocho bytes del nombre de la version `i`. El mismo trato que el klog: la
/// superficie no acepta punteros, asi que un texto viaja de ocho en ocho.
pub fn nombre(i: usize, trozo: usize) -> u64 {
    let Some(v) = version(i) else { return 0 };
    let ini = trozo * 8;
    if ini >= v.nombre_len {
        return 0;
    }
    let fin = (ini + 8).min(v.nombre_len);
    let mut w = [0u8; 8];
    w[..fin - ini].copy_from_slice(&v.nombre[ini..fin]);
    u64::from_le_bytes(w)
}
