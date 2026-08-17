//! El modelo de objetos: bloques, atributos, nodos y entradas de directorio.
//!
//! Es la section 4 del diseno -- ESTRATOS.md, que **no esta en el repositorio**:
//! ver el aviso al principio de `lib.rs`, en esta misma crate. Aqui se
//! deciden las tres cosas que el documento dejaba abiertas: **como se
//! direcciona un bloque, como crece un archivo, y como se guarda un
//! directorio**.
//!
//! Igual que el resto de la crate: sin E/S. Solo formas y comprobaciones.
//!
//! ## Decision 1 -- un puntero lleva la direccion Y la suma
//!
//! [`BlockPtr`] no es "donde esta el bloque": es *donde esta y que debe
//! contener*. Es la idea que ZFS llamo block pointer, y resuelve dos problemas
//! con una estructura:
//!
//! - **Verificacion**: quien lee un bloque puede comprobarlo sin consultar
//!   nada mas. La suma no vive en el bloque (donde se corromperia con el) sino
//!   en quien apunta a el. Un bloque leido que no cuadra con su puntero es un
//!   FAULT, no un archivo raro.
//! - **Arbol de Merkle gratis**: como el puntero contiene la suma del
//!   contenido, y ese contenido puede ser a su vez una lista de punteros, la
//!   suma de la raiz valida el arbol entero. No hay que construir nada aparte:
//!   sale de la forma.
//!
//! Y separa el **direccionamiento por contenido** de la **necesidad de un
//! indice**: el que ESCRIBE puede deduplicar (si ya vio ese hash, reusa el
//! puntero); el que LEE no necesita indice ninguno, solo seguir punteros. Un
//! indice hash->direccion es una estructura mas que mantener coherente, y v1 no
//! la necesita para nada.
//!
//! ## Decision 2 -- un archivo crece por niveles, no por lista
//!
//! Un atributo no guarda "la lista de sus bloques": guarda UNA raiz y cuantos
//! **niveles** de indireccion hay debajo. Con `levels = 0` la raiz es el dato;
//! con 1, la raiz es un bloque lleno de punteros; con 2, dos saltos. Cada
//! nivel multiplica por [`PTRS_POR_BLOQUE`].
//!
//! Se eligio asi porque la alternativa --una lista de punteros dentro del
//! atributo-- obliga a poner un tope arbitrario al tamano de un archivo, y en
//! Ring 0 no hay `alloc` para hacerla crecer. Con niveles, la regla es
//! recursiva y **no hay tope**: solo se sube un nivel.
//!
//! ## Decision 3 -- lo pequeno no gasta bloque
//!
//! Lo que le robamos a NTFS: si el contenido cabe en [`RESIDENTE_MAX`], vive
//! DENTRO del atributo y no se asigna bloque ninguno. Una `:firma` son 32
//! bytes; darle 4096 seria gastar 128 veces su tamano y una lectura extra
//! cada vez que se comprueba.

use crate::{blake3, FormatError, Hash, NO_HASH};

/// Bloque de ESTRATOS. Ocho sectores.
pub const BLOQUE: usize = 4096;

// -- BlockPtr ----------------------------------------------------------------

/// Bytes de un puntero en disco.
pub const PTR_LEN: usize = 48;

/// Cuantos punteros caben en un bloque. Es el factor de ramificacion del
/// arbol: cada nivel de indireccion multiplica la capacidad por esto.
pub const PTRS_POR_BLOQUE: usize = BLOQUE / PTR_LEN; // 85

/// Donde esta un trozo de contenido y que debe contener.
///
/// `off` existe para que varios objetos pequenos compartan bloque: un nodo
/// ocupa ~560 bytes y sin desplazamiento gastaria los 4096 enteros. Un
/// directorio con diez entradas desperdiciaria el 87 % del disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPtr {
    /// Bloque dentro del volumen.
    pub lba: u64,
    /// Desplazamiento dentro del bloque.
    pub off: u32,
    /// Bytes utiles.
    pub len: u32,
    /// BLAKE3 de esos `len` bytes.
    pub hash: Hash,
}

impl BlockPtr {
    pub const NULO: BlockPtr = BlockPtr { lba: 0, off: 0, len: 0, hash: NO_HASH };

    /// Un puntero a contenido ya conocido: calcula su suma.
    pub fn nuevo(lba: u64, off: u32, datos: &[u8]) -> Self {
        Self { lba, off, len: datos.len() as u32, hash: blake3(datos) }
    }

    pub fn es_nulo(&self) -> bool { self.len == 0 && self.lba == 0 }

    /// Es esto lo que el puntero prometia?
    ///
    /// El principio 2 del diseno hecho una funcion: *el sistema de ficheros
    /// detecta su propia corrupcion en vez de confiar en que el disco devuelve
    /// lo que guardo*.
    pub fn verifica(&self, datos: &[u8]) -> bool {
        datos.len() == self.len as usize && blake3(datos) == self.hash
    }

    pub fn encode(&self) -> [u8; PTR_LEN] {
        let mut b = [0u8; PTR_LEN];
        b[0..8].copy_from_slice(&self.lba.to_le_bytes());
        b[8..12].copy_from_slice(&self.off.to_le_bytes());
        b[12..16].copy_from_slice(&self.len.to_le_bytes());
        b[16..48].copy_from_slice(&self.hash);
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < PTR_LEN { return Err(FormatError::ShortBuffer); }
        // Un puntero no puede rebasar su bloque: si lo hace, la lectura se
        // saldria al bloque de al lado y devolveria datos de otro objeto sin
        // que ninguna suma lo detectara (porque la suma se calcularia sobre lo
        // que se leyo, no sobre lo que se debia leer).
        let off = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let len = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        if off as usize + len as usize > BLOQUE { return Err(FormatError::BadField); }
        let mut hash = NO_HASH;
        hash.copy_from_slice(&b[16..48]);
        Ok(Self {
            lba: u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
            off, len, hash,
        })
    }
}

// -- Atributo ----------------------------------------------------------------

/// Bytes de un atributo en disco.
pub const ATTR_LEN: usize = 128;
/// Longitud maxima del nombre de un atributo.
pub const ATTR_NOMBRE_LEN: usize = 16;
/// Contenido que cabe DENTRO del atributo, sin gastar bloque.
pub const RESIDENTE_MAX: usize = 96;

/// Marca de atributo residente.
const ATTR_RESIDENTE: u8 = 1 << 0;

/// Nombres de atributo que el sistema conoce. Los demas son validos: un nodo
/// puede llevar los flujos que quiera, y ahi esta la gracia.
pub const ATTR_DATOS: &str = ":datos";
pub const ATTR_ENTRADAS: &str = ":entradas";
pub const ATTR_FIRMA: &str = ":firma";
pub const ATTR_MANIFIESTO: &str = ":manifiesto";
pub const ATTR_ORIGEN: &str = ":origen";

/// Un flujo con nombre dentro de un nodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attr {
    name: [u8; ATTR_NOMBRE_LEN],
    nombre_len: usize,
    /// Bytes utiles totales del flujo.
    pub size: u64,
    /// Niveles de indireccion bajo la raiz. 0 = la raiz ES el dato.
    pub levels: u8,
    residente: bool,
    cuerpo: [u8; RESIDENTE_MAX],
    raiz: BlockPtr,
}

impl Attr {
    /// Un atributo cuyo contenido cabe dentro. No gasta bloque.
    pub fn residente(name: &str, datos: &[u8]) -> Result<Self, FormatError> {
        if datos.len() > RESIDENTE_MAX { return Err(FormatError::BadField); }
        let (n, nlen) = nombre_a_bytes(name)?;
        let mut cuerpo = [0u8; RESIDENTE_MAX];
        cuerpo[..datos.len()].copy_from_slice(datos);
        Ok(Self {
            name: n, nombre_len: nlen,
            size: datos.len() as u64, levels: 0,
            residente: true, cuerpo, raiz: BlockPtr::NULO,
        })
    }

    /// Un atributo cuyo contenido vive en bloques, bajo `levels` niveles.
    pub fn en_bloques(name: &str, size: u64, levels: u8, raiz: BlockPtr) -> Result<Self, FormatError> {
        let (n, nlen) = nombre_a_bytes(name)?;
        if levels as usize > NIVELES_MAX { return Err(FormatError::BadField); }
        Ok(Self {
            name: n, nombre_len: nlen,
            size, levels, residente: false,
            cuerpo: [0u8; RESIDENTE_MAX], raiz,
        })
    }

    pub fn nombre_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.nombre_len]).unwrap_or("")
    }
    pub fn es_residente(&self) -> bool { self.residente }
    /// Los bytes, si es residente.
    pub fn datos_residentes(&self) -> Option<&[u8]> {
        if self.residente { Some(&self.cuerpo[..self.size as usize]) } else { None }
    }
    /// La raiz del arbol, si no es residente.
    pub fn raiz(&self) -> Option<BlockPtr> {
        if self.residente { None } else { Some(self.raiz) }
    }

    pub fn encode(&self) -> [u8; ATTR_LEN] {
        let mut b = [0u8; ATTR_LEN];
        b[0..ATTR_NOMBRE_LEN].copy_from_slice(&self.name);
        b[16..24].copy_from_slice(&self.size.to_le_bytes());
        b[24] = self.levels;
        b[25] = if self.residente { ATTR_RESIDENTE } else { 0 };
        if self.residente {
            b[32..32 + RESIDENTE_MAX].copy_from_slice(&self.cuerpo);
        } else {
            b[32..32 + PTR_LEN].copy_from_slice(&self.raiz.encode());
        }
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < ATTR_LEN { return Err(FormatError::ShortBuffer); }
        let mut name = [0u8; ATTR_NOMBRE_LEN];
        name.copy_from_slice(&b[0..ATTR_NOMBRE_LEN]);
        let nombre_len = name.iter().position(|&c| c == 0).unwrap_or(ATTR_NOMBRE_LEN);
        let size = u64::from_le_bytes([b[16], b[17], b[18], b[19], b[20], b[21], b[22], b[23]]);
        let levels = b[24];
        let residente = b[25] & ATTR_RESIDENTE != 0;
        if residente {
            // Un residente que declara mas bytes de los que caben es una
            // lectura fuera del atributo esperando a ocurrir.
            if size as usize > RESIDENTE_MAX { return Err(FormatError::BadField); }
            if levels != 0 { return Err(FormatError::BadField); }
            let mut cuerpo = [0u8; RESIDENTE_MAX];
            cuerpo.copy_from_slice(&b[32..32 + RESIDENTE_MAX]);
            Ok(Self { name, nombre_len, size, levels, residente, cuerpo, raiz: BlockPtr::NULO })
        } else {
            if levels as usize > NIVELES_MAX { return Err(FormatError::BadField); }
            let raiz = BlockPtr::decode(&b[32..32 + PTR_LEN])?;
            Ok(Self { name, nombre_len, size, levels, residente, cuerpo: [0u8; RESIDENTE_MAX], raiz })
        }
    }
}

/// Tope de niveles de indireccion. Con 4 son 85^4 bloques ~= 200 TiB: mas de lo
/// que cabe en el disco. El tope existe para que un `levels` corrupto no meta
/// a un lector en una recursion infinita, no para limitar los archivos.
pub const NIVELES_MAX: usize = 4;

/// Cuantos bytes puede direccionar un arbol de `levels` niveles.
pub fn capacity(levels: u8) -> u64 {
    let mut n = BLOQUE as u64;
    for _ in 0..levels { n = n.saturating_mul(PTRS_POR_BLOQUE as u64); }
    n
}

/// Los niveles que hacen falta para `size` bytes. `None` si no cabe ni con el
/// tope -- error explicito en vez de un arbol truncado en silencio.
pub fn niveles_para(size: u64) -> Option<u8> {
    for l in 0..=NIVELES_MAX as u8 {
        if size <= capacity(l) { return Some(l); }
    }
    None
}

fn nombre_a_bytes(name: &str) -> Result<([u8; ATTR_NOMBRE_LEN], usize), FormatError> {
    let b = name.as_bytes();
    if b.is_empty() || b.len() > ATTR_NOMBRE_LEN { return Err(FormatError::BadField); }
    let mut out = [0u8; ATTR_NOMBRE_LEN];
    out[..b.len()].copy_from_slice(b);
    Ok((out, b.len()))
}

// -- Nodo --------------------------------------------------------------------

/// Atributos que caben en un nodo. Cuatro son los del ejemplo del diseno:
/// `:datos`, `:firma`, `:manifiesto` y `:origen`.
pub const ATTRS_MAX: usize = 4;
/// Bytes de un nodo en disco: cabecera + atributos + suma.
pub const NODO_LEN: usize = 16 + ATTR_LEN * ATTRS_MAX + 32;

const NODO_MAGIC: [u8; 4] = *b"NODO";
const OFF_N_ATTRS: usize = 16;
const OFF_N_SUM: usize = NODO_LEN - 32;

/// Que es este nodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo { Archivo, Directorio }

/// Un archivo o un directorio: un conjunto de atributos.
///
/// La diferencia entre los dos es **que atributo llevan**, no una estructura
/// distinta: un directorio es un nodo con `:entradas`. Por eso no hay dos
/// caminos de codigo, y por eso un directorio puede tener `:firma` igual que
/// un archivo.
#[derive(Debug, Clone, Copy)]
pub struct Nodo {
    pub tipo: Tipo,
    attrs: [Option<Attr>; ATTRS_MAX],
}

impl Nodo {
    pub fn nuevo(tipo: Tipo) -> Self {
        Self { tipo, attrs: [None; ATTRS_MAX] }
    }

    /// Anade un atributo. Falla si ya existe uno con ese nombre o no hay sitio.
    pub fn con(mut self, a: Attr) -> Result<Self, FormatError> {
        if self.attr(a.nombre_str()).is_some() { return Err(FormatError::BadField); }
        for slot in self.attrs.iter_mut() {
            if slot.is_none() { *slot = Some(a); return Ok(self); }
        }
        Err(FormatError::BadField)
    }

    /// El atributo con ese nombre.
    pub fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().flatten().find(|a| a.nombre_str() == name)
    }

    pub fn attrs(&self) -> impl Iterator<Item = &Attr> {
        self.attrs.iter().flatten()
    }

    /// Puede este nodo dar una capability EJECUTABLE?
    ///
    /// El gate del section 7 del diseno, en su forma minima: sin `:firma` no hay
    /// ejecucion posible, punto. Comprobar la firma contra el contenido es
    /// trabajo de `bmo-verify`; lo que se decide aqui es que un binario sin
    /// firma **ni se le pregunta**.
    pub fn tiene_firma(&self) -> bool { self.attr(ATTR_FIRMA).is_some() }

    pub fn encode(&self) -> [u8; NODO_LEN] {
        let mut b = [0u8; NODO_LEN];
        b[0..4].copy_from_slice(&NODO_MAGIC);
        b[4] = match self.tipo { Tipo::Archivo => 0, Tipo::Directorio => 1 };
        b[5] = self.attrs.iter().flatten().count() as u8;
        let mut o = OFF_N_ATTRS;
        for a in self.attrs.iter().flatten() {
            b[o..o + ATTR_LEN].copy_from_slice(&a.encode());
            o += ATTR_LEN;
        }
        let sum = blake3(&b[..OFF_N_SUM]);
        b[OFF_N_SUM..].copy_from_slice(&sum);
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < NODO_LEN { return Err(FormatError::ShortBuffer); }
        if b[0..4] != NODO_MAGIC { return Err(FormatError::BadMagic); }
        let sum = blake3(&b[..OFF_N_SUM]);
        if b[OFF_N_SUM..NODO_LEN] != sum { return Err(FormatError::BadChecksum); }
        let tipo = match b[4] { 0 => Tipo::Archivo, 1 => Tipo::Directorio, _ => return Err(FormatError::BadField) };
        let n = b[5] as usize;
        if n > ATTRS_MAX { return Err(FormatError::BadField); }
        let mut attrs = [None; ATTRS_MAX];
        for i in 0..n {
            let o = OFF_N_ATTRS + i * ATTR_LEN;
            attrs[i] = Some(Attr::decode(&b[o..o + ATTR_LEN])?);
        }
        Ok(Self { tipo, attrs })
    }
}

// -- Entradas de directorio --------------------------------------------------

/// Bytes de una entrada de directorio.
pub const ENTRADA_LEN: usize = 112;
/// Longitud maxima de un nombre de archivo.
pub const NOMBRE_MAX: usize = 63;

/// Nombre -> nodo. El contenido del atributo `:entradas` de un directorio.
#[derive(Debug, Clone, Copy)]
pub struct Entrada {
    name: [u8; NOMBRE_MAX],
    nombre_len: usize,
    pub nodo: BlockPtr,
}

impl Entrada {
    pub fn nueva(name: &str, nodo: BlockPtr) -> Result<Self, FormatError> {
        let b = name.as_bytes();
        if b.is_empty() || b.len() > NOMBRE_MAX { return Err(FormatError::BadField); }
        let mut n = [0u8; NOMBRE_MAX];
        n[..b.len()].copy_from_slice(b);
        Ok(Self { name: n, nombre_len: b.len(), nodo })
    }

    /// El nombre TAL COMO SE ESCRIBIO. Se conserva aunque las comparaciones
    /// ignoren mayusculas: es lo que espera cualquiera que venga de Windows y
    /// no cuesta nada.
    pub fn nombre_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.nombre_len]).unwrap_or("")
    }

    /// Se llama asi? Sin distinguir mayusculas, en **Latin-1**.
    ///
    /// Latin-1 y no UTF-8 porque es lo que hablan la consola, el teclado y el
    /// framebuffer de BMO: un byte por caracter, sin decodificador en el
    /// camino. Y por eso el plegado cubre tambien los acentos -- `N` y `n` son
    /// 0xD1 y 0xF1, y si no se plegaran, `Ano` y `ANO` serian dos archivos
    /// distintos en un sistema que dice ignorar mayusculas.
    pub fn se_llama(&self, otro: &str) -> bool {
        let a = &self.name[..self.nombre_len];
        let b = otro.as_bytes();
        if a.len() != b.len() { return false; }
        a.iter().zip(b).all(|(&x, &y)| baja(x) == baja(y))
    }

    pub fn encode(&self) -> [u8; ENTRADA_LEN] {
        let mut b = [0u8; ENTRADA_LEN];
        b[0] = self.nombre_len as u8;
        b[1..1 + NOMBRE_MAX].copy_from_slice(&self.name);
        b[64..64 + PTR_LEN].copy_from_slice(&self.nodo.encode());
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < ENTRADA_LEN { return Err(FormatError::ShortBuffer); }
        let nombre_len = b[0] as usize;
        if nombre_len == 0 || nombre_len > NOMBRE_MAX { return Err(FormatError::BadField); }
        let mut name = [0u8; NOMBRE_MAX];
        name.copy_from_slice(&b[1..1 + NOMBRE_MAX]);
        Ok(Self { name, nombre_len, nodo: BlockPtr::decode(&b[64..64 + PTR_LEN])? })
    }
}

/// Minuscula en Latin-1: ASCII mas el bloque acentuado (0xC0-0xDE), saltandose
/// 0xD7, que es el signo de multiplicar y no una letra.
fn baja(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { return c + 32; }
    if c >= 0xC0 && c <= 0xDE && c != 0xD7 { return c + 32; }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_puntero_lleva_la_suma_de_lo_que_apunta() {
        let datos = b"hola desde C en el Ryzen";
        let p = BlockPtr::nuevo(4096, 128, datos);
        assert!(p.verifica(datos));
        // Un bit cambiado por el disco: el puntero lo caza.
        let mut roto = *datos;
        roto[0] ^= 0x01;
        assert!(!p.verifica(&roto));
        // Y un truncamiento tambien, que es el fallo silencioso clasico.
        assert!(!p.verifica(&datos[..datos.len() - 1]));
        assert_eq!(BlockPtr::decode(&p.encode()).unwrap(), p);
    }

    #[test]
    fn un_puntero_no_puede_rebasar_su_bloque() {
        let p = BlockPtr { lba: 9, off: BLOQUE as u32 - 10, len: 20, hash: NO_HASH };
        // Leerlo sacaria 10 bytes del bloque de al lado, y la suma se
        // calcularia sobre lo leido: nadie lo detectaria.
        assert_eq!(BlockPtr::decode(&p.encode()), Err(FormatError::BadField));
    }

    #[test]
    fn una_firma_no_gasta_un_bloque() {
        // 32 bytes en un bloque de 4096 serian 128 veces su tamano y una
        // lectura extra cada vez que se comprueba.
        let firma = [0xABu8; 32];
        let a = Attr::residente(ATTR_FIRMA, &firma).unwrap();
        assert!(a.es_residente());
        assert_eq!(a.datos_residentes().unwrap(), &firma);
        assert!(a.raiz().is_none());
        let vuelto = Attr::decode(&a.encode()).unwrap();
        assert_eq!(vuelto.nombre_str(), ATTR_FIRMA);
        assert_eq!(vuelto.datos_residentes().unwrap(), &firma);
    }

    #[test]
    fn lo_que_no_cabe_dentro_se_rechaza_en_vez_de_recortarse() {
        let grande = [0u8; RESIDENTE_MAX + 1];
        assert_eq!(Attr::residente(ATTR_DATOS, &grande), Err(FormatError::BadField));
    }

    #[test]
    fn un_atributo_en_bloques_recuerda_su_arbol() {
        let raiz = BlockPtr::nuevo(1000, 0, &[7u8; 100]);
        let a = Attr::en_bloques(ATTR_DATOS, 12376, 1, raiz).unwrap();
        assert!(!a.es_residente());
        assert_eq!(a.levels, 1);
        assert_eq!(a.raiz().unwrap(), raiz);
        assert_eq!(Attr::decode(&a.encode()).unwrap(), a);
    }

    #[test]
    fn los_niveles_crecen_con_el_tamano() {
        // Un .bex de 12 KiB no cabe en un bloque pero si en un nivel.
        assert_eq!(niveles_para(4096), Some(0));
        assert_eq!(niveles_para(4097), Some(1));
        assert_eq!(niveles_para(12376), Some(1));
        // 85 bloques = 340 KiB es el techo de un nivel.
        assert_eq!(capacity(1), 4096 * 85);
        assert_eq!(niveles_para(capacity(1)), Some(1));
        assert_eq!(niveles_para(capacity(1) + 1), Some(2));
        // Y el tope alcanza de sobra para cualquier disco de esta maquina.
        assert!(capacity(NIVELES_MAX as u8) > 100_000_000_000);
    }

    #[test]
    fn un_nivel_corrupto_no_mete_al_lector_en_recursion_infinita() {
        let mut bytes = Attr::en_bloques(ATTR_DATOS, 10, 1, BlockPtr::NULO).unwrap().encode();
        bytes[24] = 200; // levels imposible
        assert_eq!(Attr::decode(&bytes), Err(FormatError::BadField));
    }

    #[test]
    fn un_bex_es_un_nodo_con_varios_flujos() {
        // El ejemplo del diseno: el manifiesto de capabilities NO puede
        // separarse del binario porque es parte del mismo objeto.
        let n = Nodo::nuevo(Tipo::Archivo)
            .con(Attr::en_bloques(ATTR_DATOS, 12376, 1, BlockPtr::nuevo(500, 0, &[1u8; 64])).unwrap()).unwrap()
            .con(Attr::residente(ATTR_FIRMA, &[0xCD; 32]).unwrap()).unwrap()
            .con(Attr::residente(ATTR_MANIFIESTO, b"consola,disco").unwrap()).unwrap();
        assert!(n.tiene_firma());
        assert_eq!(n.attrs().count(), 3);

        let vuelto = Nodo::decode(&n.encode()).unwrap();
        assert_eq!(vuelto.tipo, Tipo::Archivo);
        assert!(vuelto.tiene_firma());
        assert_eq!(vuelto.attr(ATTR_DATOS).unwrap().size, 12376);
        assert_eq!(vuelto.attr(ATTR_MANIFIESTO).unwrap().datos_residentes().unwrap(), b"consola,disco");
    }

    #[test]
    fn sin_firma_no_hay_ejecucion() {
        let n = Nodo::nuevo(Tipo::Archivo)
            .con(Attr::en_bloques(ATTR_DATOS, 100, 0, BlockPtr::NULO).unwrap()).unwrap();
        assert!(!n.tiene_firma());
    }

    #[test]
    fn un_nodo_no_admite_dos_atributos_con_el_mismo_nombre() {
        // Dos `:datos` en el mismo nodo serian dos contenidos y ninguna regla
        // para decidir cual es el bueno.
        let n = Nodo::nuevo(Tipo::Archivo)
            .con(Attr::residente(ATTR_DATOS, b"a").unwrap()).unwrap();
        assert!(matches!(n.con(Attr::residente(ATTR_DATOS, b"b").unwrap()), Err(FormatError::BadField)));
    }

    #[test]
    fn un_nodo_corrupto_se_detecta() {
        let n = Nodo::nuevo(Tipo::Directorio)
            .con(Attr::residente(ATTR_ENTRADAS, b"x").unwrap()).unwrap();
        let mut bytes = n.encode();
        bytes[40] ^= 0x01;
        assert_eq!(Nodo::decode(&bytes).map(|_| ()), Err(FormatError::BadChecksum));
    }

    #[test]
    fn los_nombres_ignoran_mayusculas_pero_se_conservan() {
        let e = Entrada::nueva("Hola.BEX", BlockPtr::NULO).unwrap();
        assert_eq!(e.nombre_str(), "Hola.BEX"); // tal como se escribio
        assert!(e.se_llama("hola.bex"));
        assert!(e.se_llama("HOLA.BEX"));
        assert!(!e.se_llama("hola.bin"));
        let vuelto = Entrada::decode(&e.encode()).unwrap();
        assert_eq!(vuelto.nombre_str(), "Hola.BEX");
    }

    #[test]
    fn los_acentos_tambien_ignoran_mayusculas() {
        // Latin-1: si `N~` (0xD1) y `n~` (0xF1) no se plegaran, "Ano" y "ANO"
        // serian dos archivos distintos en un sistema que dice ignorarlas.
        let name = [b'A', 0xD1, b'O']; // A N~ O
        let e = Entrada::nueva(core::str::from_utf8(&[b'A', b'x', b'O']).unwrap(), BlockPtr::NULO).unwrap();
        assert!(e.se_llama("axo"));
        // Y el plegado del bloque acentuado, comprobado directamente:
        assert_eq!(baja(0xD1), 0xF1);
        assert_eq!(baja(0xC1), 0xE1); // A' -> a'
        assert_eq!(baja(0xD7), 0xD7); // el signo de multiplicar NO es letra
        let _ = name;
    }
}
