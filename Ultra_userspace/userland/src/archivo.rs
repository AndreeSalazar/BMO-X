//! El DISCO desde Ring 3: directorios y archivos.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// ── El disco ────────────────────────────────────────────────────────────

/// Una entrada de directorio, ya leída.
///
/// `EntradaDir` y no `Entrada` porque `Entrada` ya es el ratón+teclado. Dos
/// cosas distintas no pueden llamarse igual aunque en castellano suenen igual.
pub struct EntradaDir {
    /// Nombre 8.3 CRUDO, con sus espacios de relleno: `"COBOL   BEX"`.
    /// Convertirlo a `COBOL.BEX` es presentación, y eso es cosa de quien pinta.
    pub nombre: [u8; 11],
    pub es_dir: bool,
    pub bytes: u32,
}

impl EntradaDir {
    /// El nombre en forma legible, `cobol.bex`, escrito en `dst`. Devuelve
    /// cuántos bytes ocupó.
    ///
    /// ★ En MINÚSCULA. FAT32 los guarda en mayúscula porque el formato es de
    /// 1980 y no distinguía; eso es un detalle del disco, no del nombre. Un
    /// listado a gritos se lee peor, y el kernel acepta las dos formas al
    /// abrir — así que no se pierde nada bajándolos aquí, que es donde se
    /// decide cómo se ve.
    pub fn legible(&self, dst: &mut [u8; 12]) -> usize {
        let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
        let mut n = 0;
        for i in 0..8 {
            if self.nombre[i] == b' ' { break; }
            dst[n] = baja(self.nombre[i]);
            n += 1;
        }
        if self.nombre[8] != b' ' {
            dst[n] = b'.';
            n += 1;
            for i in 8..11 {
                if self.nombre[i] == b' ' { break; }
                dst[n] = baja(self.nombre[i]);
                n += 1;
            }
        }
        n
    }
}

/// Un directorio abierto.
///
/// ★ Esto NO es "una ruta que cualquiera puede escribir". Es un handle que te
/// concedieron: lo que no te hayan dado no existe para este proceso. Es la
/// misma disciplina que la pantalla y la entrada — un nombre es adivinable, un
/// permiso no.
pub struct Directorio {
    pub cap: u64,
}

impl Directorio {
    /// Abre un directorio del volumen de datos. Ruta vacía = la raíz.
    pub fn abrir(ruta: &[u8]) -> Result<Self, u32> {
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, OP_DIR_ABRIR, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value }) } else { Err(st.code) }
    }

    /// La siguiente entrada, o `None` cuando se acaba.
    pub fn siguiente(&self) -> Option<EntradaDir> {
        let v = invoke(self.cap, DIR_OP_SIGUIENTE, 0, 0, 0).value;
        if v >> 63 == 0 {
            return None;
        }
        let es_dir = (v >> 62) & 1 != 0;
        let bytes = v as u32;
        // El nombre viene aparte: son 11 bytes y no caben con lo demás.
        let mut nombre = [b' '; 11];
        let mut puesto = 0usize;
        for desde in [0u64, 7] {
            let w = invoke(self.cap, DIR_OP_NOMBRE, desde, 0, 0).value;
            let n = (w >> 56) as usize;
            let b = w.to_le_bytes();
            for k in 0..n.min(7) {
                if puesto < 11 {
                    nombre[puesto] = b[k];
                    puesto += 1;
                }
            }
        }
        Some(EntradaDir { nombre, es_dir, bytes })
    }
}

/// **Cerrar es del `Drop`, no de quien llama.**
///
/// ═══ El bug que esto arregla ═══
///
/// No había forma de cerrar un directorio, y la tabla del kernel son OCHO
/// ranuras que sólo se liberaban al **morir el proceso**. El cliente es el
/// compositor, que **no muere nunca** — es el escritorio.
///
/// Así que cada `ls` se quedaba una ranura para siempre y al noveno la tabla
/// estaba llena: `ls` empezaba a contestar *"no puedo abrir esa carpeta"* y ya
/// no se recuperaba hasta reiniciar. Un fallo que aparece **después de un rato
/// de uso normal** y no se puede reproducir recién arrancado, que es de los
/// peores de encontrar.
///
/// Y va en `Drop` y no en un método `cerrar()` a propósito: un cierre que hay
/// que acordarse de llamar es un cierre que un día no se llama. Aquí el
/// compositor no cambia ni una línea — el `Directorio` sale de ámbito al
/// terminar el `ls` y la ranura vuelve sola.
impl Drop for Directorio {
    fn drop(&mut self) {
        invoke(self.cap, DIR_OP_CERRAR, 0, 0, 0);
    }
}

// ── Un archivo ──────────────────────────────────────────────────────────

/// Un archivo abierto del volumen de datos.
///
/// Hermano de [`Directorio`]: aquel deja PREGUNTAR qué hay, éste deja mover
/// los bytes de dentro. Y la misma disciplina — no es una ruta que cualquiera
/// escriba, es un handle que te concedieron.
///
/// El MODO se fija al abrir: [`Archivo::leer_de`] da uno de lectura y
/// [`Archivo::crear`] uno de escritura. Pedirle bytes a uno de escritura no
/// devuelve un error de permisos: devuelve que esa pregunta no existe para ese
/// objeto.
///
/// **Límite de hoy**: 4 KiB por archivo. Los bytes cruzan de 7 en 7 (la
/// superficie congelada no acepta punteros) y hace falta un buffer en el
/// kernel donde juntarlos. Ver `ring0/archivo.rs`; lo que lo quitará es un
/// escritor por sectores en `bmo_fat32`, que es otra pieza.
pub struct Archivo {
    pub cap: u64,
    escribe: bool,
}

impl Archivo {
    fn con_ruta(ruta: &[u8], op: u32, escribe: bool) -> Result<Self, u32> {
        // El mismo renglón que usan `ejecutar` y `Directorio::abrir`. No hay
        // un segundo mecanismo para lo mismo.
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, op, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value, escribe }) } else { Err(st.code) }
    }

    /// Abre un archivo para LEER. Se trae entero al abrir, así que a partir de
    /// aquí una lectura no puede fallar a mitad por un error de disco.
    pub fn leer_de(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_ABRIR, false)
    }

    /// Abre un archivo para ESCRIBIR. Acepta subdirectorios
    /// (`datos/movim.dat`); el nombre tiene que ser un 8.3 válido.
    ///
    /// **Nada llega al disco hasta [`Archivo::cerrar`]**. Un proceso que muere
    /// a medias no deja un archivo a medias: no deja nada.
    pub fn crear(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_CREAR, true)
    }

    /// Llena `dst` con lo que quede. Devuelve cuántos bytes se leyeron; `0` =
    /// se acabó el archivo.
    pub fn leer(&self, dst: &mut [u8]) -> usize {
        if self.escribe {
            return 0;
        }
        let mut puestos = 0usize;
        while puestos < dst.len() {
            let v = invoke(self.cap, ARCH_OP_LEER, 0, 0, 0).value;
            let n = (v >> 56) as usize;
            if n == 0 {
                break;
            }
            let b = v.to_le_bytes();
            for k in 0..n.min(7) {
                if puestos < dst.len() {
                    dst[puestos] = b[k];
                    puestos += 1;
                }
            }
        }
        puestos
    }

    /// Añade bytes. Devuelve cuántos se aceptaron — menos de los pedidos
    /// significa que se llenó, y entonces `cerrar` devolverá `false`.
    ///
    /// Los bytes viajan de 7 en 7 con su cuenta en el byte alto, no cortando
    /// en el primer cero: un archivo no es texto y un `\0` en medio es un dato
    /// como cualquier otro.
    pub fn escribir(&self, datos: &[u8]) -> usize {
        if !self.escribe {
            return 0;
        }
        let mut puestos = 0usize;
        for trozo in datos.chunks(7) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            w[7] = trozo.len() as u8;
            let n = invoke(self.cap, ARCH_OP_ESCRIBIR, u64::from_le_bytes(w), 0, 0).value;
            puestos += n as usize;
            if (n as usize) < trozo.len() {
                break;
            }
        }
        puestos
    }

    /// Bytes que quedan por leer, o bytes acumulados si es de escritura.
    pub fn tamano(&self) -> u64 {
        invoke(self.cap, ARCH_OP_TAMANO, 0, 0, 0).value
    }

    /// Cierra. En uno de escritura es **donde el contenido llega al disco**:
    /// `false` significa que no se guardó nada, no que se guardara a medias.
    pub fn cerrar(self) -> bool {
        let ok = invoke(self.cap, ARCH_OP_CERRAR, 0, 0, 0).value != 0;
        // ★ Y NO se deja caer el `Drop` encima.
        //
        // `Drop` cierra lo que se olvidaron de cerrar; éste ya está cerrado, y
        // cerrarlo dos veces mandaría un `ARCH_OP_CERRAR` sobre un handle que
        // el kernel acaba de revocar. No rompe nada —contestaría "handle
        // inválido"— pero es una llamada que miente sobre lo que está pasando,
        // y las que mienten son las que confunden un log.
        //
        // `forget` es gratis aquí: esto son un `u64` y un `bool`, no hay nada
        // que liberar en Ring 3.
        core::mem::forget(self);
        ok
    }
}

/// **Cerrar es del `Drop` cuando nadie se acordó.**
///
/// `cerrar()` sigue existiendo y sigue siendo la forma correcta de cerrar un
/// archivo de ESCRITURA: es donde el contenido llega al disco, y **devuelve si
/// salió bien**. Un `Drop` no puede devolver nada, así que soltar la escritura
/// en el `Drop` sería tirar la única señal de que se guardó.
///
/// Lo que hace esto es tapar el otro caso: el archivo que se abrió, se leyó, y
/// alguien se fue por un `return` en medio. Hoy el compositor cierra bien en
/// los dos caminos — pero eso es **disciplina, no construcción**, y la
/// disciplina se rompe el día que se añade un comando nuevo con una rama de
/// error más. La tabla son 16 ranuras y los handles 64 por proceso.
impl Drop for Archivo {
    fn drop(&mut self) {
        invoke(self.cap, ARCH_OP_CERRAR, 0, 0, 0);
    }
}

