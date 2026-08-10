//! El DISCO desde Ring 3: directorios y archivos.
//!
//! Salio de `lib.rs`, que llego a tener 1624 lineas con siete trabajos
//! distintos dentro. **Aqui no se cambio ni una linea de logica: solo se
//! movio**, y quien usa la crate lo escribe exactamente igual que ayer.

use crate::*;

// -- El disco ------------------------------------------------------------

/// Una entrada de directorio, ya leida.
///
/// `EntradaDir` y no `Entrada` porque `Entrada` ya es el raton+teclado. Dos
/// cosas distintas no pueden llamarse igual aunque en castellano suenen igual.
pub struct EntradaDir {
    /// Nombre 8.3 CRUDO, con sus espacios de relleno: `"COBOL   BEX"`.
    /// Convertirlo a `COBOL.BEX` es presentacion, y eso es cosa de quien pinta.
    pub name: [u8; 11],
    pub es_dir: bool,
    pub bytes: u32,
}

impl EntradaDir {
    /// El nombre en forma legible, `cobol.bex`, escrito en `dst`. Devuelve
    /// cuantos bytes ocupo.
    ///
    /// * En MINUSCULA. FAT32 los guarda en mayuscula porque el formato es de
    /// 1980 y no distinguia; eso es un detalle del disco, no del nombre. Un
    /// listado a gritos se lee peor, y el kernel acepta las dos formas al
    /// abrir -- asi que no se pierde nada bajandolos aqui, que es donde se
    /// decide como se ve.
    pub fn legible(&self, dst: &mut [u8; 12]) -> usize {
        let baja = |c: u8| if c.is_ascii_uppercase() { c + 32 } else { c };
        let mut n = 0;
        for i in 0..8 {
            if self.name[i] == b' ' { break; }
            dst[n] = baja(self.name[i]);
            n += 1;
        }
        if self.name[8] != b' ' {
            dst[n] = b'.';
            n += 1;
            for i in 8..11 {
                if self.name[i] == b' ' { break; }
                dst[n] = baja(self.name[i]);
                n += 1;
            }
        }
        n
    }
}

/// Un directorio abierto.
///
/// * Esto NO es "una ruta que cualquiera puede escribir". Es un handle que te
/// concedieron: lo que no te hayan dado no existe para este proceso. Es la
/// misma disciplina que la pantalla y la entrada -- un nombre es adivinable, un
/// permiso no.
pub struct Directorio {
    pub cap: u64,
}

impl Directorio {
    /// Abre un directorio del volumen de datos. Ruta vacia = la raiz.
    pub fn open(ruta: &[u8]) -> Result<Self, u32> {
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, OP_DIR_ABRIR, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value }) } else { Err(st.code) }
    }

    /// La siguiente entrada, o `None` cuando se acaba.
    pub fn next(&self) -> Option<EntradaDir> {
        let v = invoke(self.cap, DIR_OP_SIGUIENTE, 0, 0, 0).value;
        if v >> 63 == 0 {
            return None;
        }
        let es_dir = (v >> 62) & 1 != 0;
        let bytes = v as u32;
        // El nombre viene aparte: son 11 bytes y no caben con lo demas.
        let mut name = [b' '; 11];
        let mut puesto = 0usize;
        for desde in [0u64, 7] {
            let w = invoke(self.cap, DIR_OP_NOMBRE, desde, 0, 0).value;
            let n = (w >> 56) as usize;
            let b = w.to_le_bytes();
            for k in 0..n.min(7) {
                if puesto < 11 {
                    name[puesto] = b[k];
                    puesto += 1;
                }
            }
        }
        Some(EntradaDir { name, es_dir, bytes })
    }
}

/// **Cerrar es del `Drop`, no de quien llama.**
///
/// === El bug que esto arregla ===
///
/// No habia forma de cerrar un directorio, y la tabla del kernel son OCHO
/// ranuras que solo se liberaban al **morir el proceso**. El cliente es el
/// compositor, que **no muere nunca** -- es el escritorio.
///
/// Asi que cada `ls` se quedaba una ranura para siempre y al noveno la tabla
/// estaba llena: `ls` empezaba a contestar *"no puedo abrir esa carpeta"* y ya
/// no se recuperaba hasta reiniciar. Un fallo que aparece **despues de un rato
/// de uso normal** y no se puede reproducir recien arrancado, que es de los
/// peores de encontrar.
///
/// Y va en `Drop` y no en un metodo `close()` a proposito: un cierre que hay
/// que acordarse de llamar es un cierre que un dia no se llama. Aqui el
/// compositor no cambia ni una linea -- el `Directorio` sale de ambito al
/// terminar el `ls` y la ranura vuelve sola.
impl Drop for Directorio {
    fn drop(&mut self) {
        invoke(self.cap, DIR_OP_CERRAR, 0, 0, 0);
    }
}

// -- Un archivo ----------------------------------------------------------

/// Un archivo abierto del volumen de datos.
///
/// Hermano de [`Directorio`]: aquel deja PREGUNTAR que hay, este deja mover
/// los bytes de dentro. Y la misma disciplina -- no es una ruta que cualquiera
/// escriba, es un handle que te concedieron.
///
/// El MODO se fija al abrir: [`Archivo::leer_de`] da uno de lectura y
/// [`Archivo::create`] uno de escritura. Pedirle bytes a uno de escritura no
/// devuelve un error de permisos: devuelve que esa pregunta no existe para ese
/// objeto.
///
/// **Limite de hoy**: 4 KiB por archivo. Los bytes cruzan de 7 en 7 (la
/// superficie congelada no acepta punteros) y hace falta un buffer en el
/// kernel donde juntarlos. Ver `ring0/archivo.rs`; lo que lo quitara es un
/// escritor por sectores en `bmo_fat32`, que es otra pieza.
pub struct Archivo {
    pub cap: u64,
    escribe: bool,
}

impl Archivo {
    fn con_ruta(ruta: &[u8], op: u32, escribe: bool) -> Result<Self, u32> {
        // El mismo renglon que usan `ejecutar` y `Directorio::open`. No hay
        // un segundo mecanismo para lo mismo.
        for trozo in ruta.chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            invoke(CURRENT_TASK, OP_RUTA, u64::from_le_bytes(w), 0, 0);
        }
        let st = invoke(CURRENT_TASK, op, 0, 0, 0);
        if st.ok() { Ok(Self { cap: st.value, escribe }) } else { Err(st.code) }
    }

    /// Abre un archivo para LEER. Se trae entero al abrir, asi que a partir de
    /// aqui una lectura no puede fallar a mitad por un error de disco.
    pub fn leer_de(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_ABRIR, false)
    }

    /// Abre un archivo para ESCRIBIR. Acepta subdirectorios
    /// (`datos/movim.dat`); el nombre tiene que ser un 8.3 valido.
    ///
    /// **Nada llega al disco hasta [`Archivo::close`]**. Un proceso que muere
    /// a medias no deja un archivo a medias: no deja nada.
    pub fn create(ruta: &[u8]) -> Result<Self, u32> {
        Self::con_ruta(ruta, OP_ARCHIVO_CREAR, true)
    }

    /// Llena `dst` con lo que quede. Devuelve cuantos bytes se leyeron; `0` =
    /// se acabo el archivo.
    pub fn read(&self, dst: &mut [u8]) -> usize {
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

    /// Anade bytes. Devuelve cuantos se aceptaron -- menos de los pedidos
    /// significa que se lleno, y entonces `close` devolvera `false`.
    ///
    /// Los bytes viajan de 7 en 7 con su cuenta en el byte alto, no cortando
    /// en el primer cero: un archivo no es texto y un `\0` en medio es un dato
    /// como cualquier otro.
    pub fn write(&self, datos: &[u8]) -> usize {
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

    /// Mueve el cursor a una posicion ABSOLUTA. Devuelve donde quedo.
    ///
    /// Hacia falta para leer un PAQUETE: la seccion de recursos vive al final
    /// del `.bex` y su indice dice offsets, asi que sin saltar habria que leer
    /// --de siete en siete bytes-- todo lo que hay delante. Para el icono de
    /// una app eso es leerse el programa entero para llegar a su cara.
    ///
    /// El kernel recorta al tamano en vez de fallar: saltar mas alla del final
    /// deja el cursor al final, y lo dice devolviendo donde quedo.
    pub fn saltar(&self, pos: u64) -> u64 {
        if self.escribe {
            return 0;
        }
        invoke(self.cap, ARCH_OP_SALTAR, pos, 0, 0).value
    }

    /// Bytes que quedan por leer, o bytes acumulados si es de escritura.
    pub fn tamano(&self) -> u64 {
        invoke(self.cap, ARCH_OP_TAMANO, 0, 0, 0).value
    }

    /// Cierra. En uno de escritura es **donde el contenido llega al disco**:
    /// `false` significa que no se guardo nada, no que se guardara a medias.
    pub fn close(self) -> bool {
        let ok = invoke(self.cap, ARCH_OP_CERRAR, 0, 0, 0).value != 0;
        // * Y NO se deja caer el `Drop` encima.
        //
        // `Drop` cierra lo que se olvidaron de cerrar; este ya esta cerrado, y
        // cerrarlo dos veces mandaria un `ARCH_OP_CERRAR` sobre un handle que
        // el kernel acaba de revocar. No rompe nada --contestaria "handle
        // invalido"-- pero es una llamada que miente sobre lo que esta pasando,
        // y las que mienten son las que confunden un log.
        //
        // `forget` es gratis aqui: esto son un `u64` y un `bool`, no hay nada
        // que liberar en Ring 3.
        core::mem::forget(self);
        ok
    }
}

/// **Cerrar es del `Drop` cuando nadie se acordo.**
///
/// `close()` sigue existiendo y sigue siendo la forma correcta de cerrar un
/// archivo de ESCRITURA: es donde el contenido llega al disco, y **devuelve si
/// salio bien**. Un `Drop` no puede devolver nada, asi que soltar la escritura
/// en el `Drop` seria tirar la unica senal de que se guardo.
///
/// Lo que hace esto es tapar el otro caso: el archivo que se abrio, se leyo, y
/// alguien se fue por un `return` en medio. Hoy el compositor cierra bien en
/// los dos caminos -- pero eso es **disciplina, no construccion**, y la
/// disciplina se rompe el dia que se anade un comando nuevo con una rama de
/// error mas. La tabla son 16 ranuras y los handles 64 por proceso.
impl Drop for Archivo {
    fn drop(&mut self) {
        invoke(self.cap, ARCH_OP_CERRAR, 0, 0, 0);
    }
}

