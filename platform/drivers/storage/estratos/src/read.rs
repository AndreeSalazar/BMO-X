//! Bajar por el árbol. **Un solo recorrido para todo el sistema.**
//!
//! El formateador del anfitrión y el kernel tienen que reconstruir un flujo
//! exactamente igual. La primera versión lo implementó dos veces —una en cada
//! lado— y eso es la misma trampa que casi cuesta el BLAKE3: dos copias del
//! mismo algoritmo son dos copias que pueden separarse, y el día que se
//! separen el síntoma es "un archivo que se lee mal", sin nada que apunte al
//! recorrido.
//!
//! Aquí no se sabe de dónde salen los bloques: quien llama trae su [`Fuente`]
//! (un archivo en el anfitrión, el contrato de bloques en el kernel) y su
//! memoria de trabajo. Por eso el mismo código sirve en los dos sitios sin
//! `alloc` en ninguno.
//!
//! ## Por qué un buffer POR NIVEL
//!
//! Al bajar, la lista de punteros del nivel actual tiene que seguir viva
//! mientras se recorren sus hijos. Con un buffer compartido, el primer hijo la
//! machacaría y habría que releer el bloque padre en cada iteración: una
//! lectura de disco por puntero. El `split_at_mut` de abajo le da a cada nivel
//! el suyo y hace imposible el error por construcción — un nivel no puede
//! tocar el buffer de su padre porque no lo tiene.

use crate::objects::{BlockPtr, BLOQUE, PTR_LEN};
use crate::FormatError;

/// De dónde salen los bloques.
pub trait Fuente {
    /// Lee el bloque `lba` entero. `false` si no se pudo.
    fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool;
}

/// Recorre el árbol de `raiz` y entrega los trozos de datos EN ORDEN.
///
/// `salida` devuelve `false` para parar (por ejemplo, cuando el buffer de
/// quien llama ya está lleno). Cada trozo entregado ya está **comprobado
/// contra la suma de su puntero**: quien recibe no tiene que verificar nada.
pub fn descender(
    src: &mut dyn Fuente,
    raiz: &BlockPtr,
    niveles: u8,
    scratch: &mut [[u8; BLOQUE]],
    salida: &mut dyn FnMut(&[u8]) -> bool,
) -> Result<(), FormatError> {
    let mut seguir = true;
    bajar(src, raiz, niveles, scratch, salida, &mut seguir)
}

fn bajar(
    src: &mut dyn Fuente,
    p: &BlockPtr,
    niveles: u8,
    scratch: &mut [[u8; BLOQUE]],
    salida: &mut dyn FnMut(&[u8]) -> bool,
    seguir: &mut bool,
) -> Result<(), FormatError> {
    if !*seguir { return Ok(()); }
    // Sin buffer para este nivel no se sigue. Es también el freno contra un
    // `levels` corrupto que pidiera bajar más de lo que el árbol admite.
    let (mio, resto) = match scratch.split_first_mut() {
        Some(v) => v,
        None => return Err(FormatError::SinScratch),
    };
    if !src.bloque(p.lba, mio) { return Err(FormatError::Io); }

    let ini = p.off as usize;
    let fin = ini + p.len as usize;
    if fin > BLOQUE { return Err(FormatError::BadField); }
    let datos = &mio[ini..fin];
    if !p.verifica(datos) { return Err(FormatError::BadChecksum); }

    if niveles == 0 {
        if !salida(datos) { *seguir = false; }
        return Ok(());
    }

    let total = datos.len() / PTR_LEN;
    for i in 0..total {
        let hijo = BlockPtr::decode(&datos[i * PTR_LEN..(i + 1) * PTR_LEN])?;
        if hijo.es_nulo() { break; }
        bajar(src, &hijo, niveles - 1, resto, salida, seguir)?;
        if !*seguir { break; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::PTRS_POR_BLOQUE;

    /// Un volumen en memoria: bloques consecutivos.
    struct Memoria { bloques: Vec<[u8; BLOQUE]> }

    impl Memoria {
        fn nuevo() -> Self { Self { bloques: Vec::new() } }
        /// Escribe un bloque y devuelve su puntero.
        fn poner(&mut self, datos: &[u8]) -> BlockPtr {
            let lba = self.bloques.len() as u64;
            let mut b = [0u8; BLOQUE];
            b[..datos.len()].copy_from_slice(datos);
            self.bloques.push(b);
            BlockPtr::nuevo(lba, 0, datos)
        }
        /// Escribe `datos` como árbol y devuelve `(raiz, niveles)`.
        fn arbol(&mut self, datos: &[u8]) -> (BlockPtr, u8) {
            if datos.len() <= BLOQUE { return (self.poner(datos), 0); }
            let mut nivel: Vec<BlockPtr> = datos.chunks(BLOQUE).map(|c| self.poner(c)).collect();
            let mut niveles = 0u8;
            while nivel.len() > 1 {
                let mut arriba = Vec::new();
                for grupo in nivel.chunks(PTRS_POR_BLOQUE) {
                    let mut b = Vec::new();
                    for p in grupo { b.extend_from_slice(&p.encode()); }
                    arriba.push(self.poner(&b));
                }
                nivel = arriba;
                niveles += 1;
            }
            (nivel[0], niveles)
        }
    }

    impl Fuente for Memoria {
        fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
            match self.bloques.get(lba as usize) {
                Some(b) => { dst.copy_from_slice(b); true }
                None => false,
            }
        }
    }

    fn scratch() -> Vec<[u8; BLOQUE]> { vec![[0u8; BLOQUE]; 6] }

    fn leer(m: &mut Memoria, raiz: BlockPtr, niveles: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let mut s = scratch();
        descender(m, &raiz, niveles, &mut s, &mut |t| { out.extend_from_slice(t); true }).unwrap();
        out
    }

    fn datos(n: usize) -> Vec<u8> { (0..n).map(|i| (i % 251) as u8).collect() }

    #[test]
    fn un_solo_bloque_vuelve_entero() {
        let mut m = Memoria::nuevo();
        let d = datos(4096);
        let (r, l) = m.arbol(&d);
        assert_eq!(l, 0);
        assert_eq!(leer(&mut m, r, l), d);
    }

    #[test]
    fn un_nivel_devuelve_los_trozos_en_orden() {
        // El fallo clasico de un recorrido de arbol no es perder datos: es
        // devolverlos desordenados, y entonces el archivo "se lee mal" sin que
        // ninguna suma falle, porque cada bloque por separado esta bien.
        let mut m = Memoria::nuevo();
        let d = datos(4096 * 3 + 17);
        let (r, l) = m.arbol(&d);
        assert_eq!(l, 1);
        assert_eq!(leer(&mut m, r, l), d);
    }

    #[test]
    fn dos_niveles_tambien() {
        let mut m = Memoria::nuevo();
        // 147 bloques > 85 por bloque de punteros: obliga a un segundo nivel.
        let d = datos(600_000);
        let (r, l) = m.arbol(&d);
        assert_eq!(l, 2);
        // Vuelve EXACTO, sin relleno: el puntero del ultimo trozo guarda su
        // longitud real (1984 B), no el bloque entero. Quien lee no tiene que
        // saber el tamano del archivo para no arrastrar basura al final.
        assert_eq!(leer(&mut m, r, l), d);
    }

    #[test]
    fn parar_a_media_lectura_no_es_un_error() {
        // Quien llama tiene un buffer finito. Decir "ya no quepo" tiene que ser
        // una salida limpia, no un fallo.
        let mut m = Memoria::nuevo();
        let d = datos(600_000);
        let (r, l) = m.arbol(&d);
        let mut leidos = 0usize;
        let mut s = scratch();
        descender(&mut m, &r, l, &mut s, &mut |t| { leidos += t.len(); leidos < 10_000 }).unwrap();
        assert!(leidos >= 10_000 && leidos < 20_000);
    }

    #[test]
    fn un_bloque_corrupto_para_el_recorrido() {
        let mut m = Memoria::nuevo();
        let d = datos(4096 * 3);
        let (r, l) = m.arbol(&d);
        m.bloques[1][0] ^= 0x01; // un bit en el segundo bloque de datos
        let mut s = scratch();
        let res = descender(&mut m, &r, l, &mut s, &mut |_| true);
        assert_eq!(res, Err(FormatError::BadChecksum));
    }

    #[test]
    fn sin_scratch_suficiente_falla_en_vez_de_desbordar() {
        // Un `levels` corrupto pidiendo bajar mas de lo que hay: se para.
        let mut m = Memoria::nuevo();
        let d = datos(600_000);
        let (r, _) = m.arbol(&d);
        let mut s = vec![[0u8; BLOQUE]; 1]; // solo un nivel de buffer
        assert_eq!(
            descender(&mut m, &r, 3, &mut s, &mut |_| true),
            Err(FormatError::SinScratch)
        );
    }
}
