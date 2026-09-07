//! **Leer los datos que viajan DENTRO del propio `.bex`** -- la cara de Rust.
//!
//! El gemelo en C es `<bmo/paquete.h>`, y la idea la dijo el dueno alli:
//!
//! > *"es un bef pero ese bex es el mismo que abre la caja: no lo duplica, lo
//! > lee y punto. Es una app como Windows pero no lo copia, lo deja en el lugar
//! > correcto y lee directo."*
//!
//! El paquete **no se carga**: se abre, se mira su indice, y cada recurso se lee
//! del disco por su offset cuando hace falta. Un paquete de 500 MB arranca igual
//! de rapido que uno de 500 KB, porque lo que el cargador mete en memoria es el
//! CODIGO -- los recursos ni los mira.
//!
//! ## Por que existe esta version, si ya estaba la de C
//!
//! Porque **una app de Rust no podia abrir su propia caja**, y eso es justo lo
//! que necesita ESTRUCTURA para ser *"como gcc pero sin instalar"*: un fichero
//! que trae dentro lo que necesita. Ver `docs/plan/PLAN_ESTRUCTURA.md` escalon
//! 2 y `docs/plan/PLAN_AUTOHOSPEDAJE.md`.
//!
//! ## ** Y ES MAS CORTO QUE EL DE C, POR DOS MOTIVOS QUE NO SON EL LENGUAJE
//!
//! 1. **No hay `scratch`.** La version de C reserva 1 KiB de `malloc` para leer
//!    cabeceras, porque hasta el 2026-08-13 el kernel solo escribia en bloques
//!    que el habia concedido. Eso ya no es asi --`Archivo::read` acepta la
//!    pila-- asi que aqui las cabeceras se leen en un array y se acabo.
//! 2. **No hay `bmo_u32le` a mano.** `u32::from_le_bytes` hace lo mismo y no se
//!    puede escribir mal. El de C existe porque castear el buffer a `u32*` seria
//!    una lectura desalineada, *"de las que funcionan hasta que no"*.
//!
//! ## [!] LOS NUMEROS DEL FORMATO SE REPITEN AQUI, Y NADIE LOS VIGILA
//!
//! `bmo-userland` no depende de `bmo-abi` a proposito --su `Cargo.toml` explica
//! por que-- asi que estos numeros son un ESPEJO de `bmo_abi::bef`, igual que
//! los de `paquete.h`.
//!
//! ** La diferencia con las operaciones, y hay que decirla: los `OP_*` de
//! `lib.rs` **si** los compara un juez (R4, `operaciones userland<->ABI: 94
//! comprobadas`). Estos **no**: R4 mira operaciones, y un tamano de cabecera no
//! lo es. Si `bmo_abi::bef` mueve un offset, esto se entera cuando algo no abre.
//!
//! Lo que lo tapa hoy es la fila de pruebas que empaqueta con `bmo-pack` y lee
//! con esto -- y por eso esa fila no es opcional.
//!
//! [carril]  AMARILLO     parsea un formato. Lo que arrastra no esta aqui:
//!                        esta en `bmo_abi::bef`, de donde salen los numeros
//! [cuesta]  TAREA        un offset mal leido mata al programa que abre su
//!                        propia caja. BMO sigue
//! [riesgo]  AJENO ESPEJO
//!                        AJENO: los offsets los escribio `bmo-pack`.
//!                        ESPEJO: los numeros del formato se repiten, y no hay
//!                        juez que los empareje

use crate::archivo::Archivo;

/// `"BEF1"`, la firma del contenedor.
const BEF_MAGIC: u32 = 0x3146_4542;
/// La cabecera BEF: 48 bytes antes de nada.
const BEF_CABECERA: u64 = 48;
/// Cada entrada de la tabla de secciones.
const BEF_ENTRADA: u64 = 48;
/// La seccion `Resources` de `bmo_abi::bef::SectionKind`.
const SECCION_RECURSOS: u8 = 0x0B;
/// `"BRES"`, la firma del indice de recursos.
const BRES_MAGIC: u32 = 0x5345_5242;
/// La cabecera del indice: magic + cuantos.
const BRES_CABECERA: u64 = 16;
/// Cada entrada del indice: nombre (48) + offset (8) + tamano (8).
const BRES_ENTRADA: u64 = 64;
/// El nombre de un recurso, sin contar el NUL.
const BRES_NOMBRE_MAX: usize = 47;

fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn u64le(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([
        b[i], b[i + 1], b[i + 2], b[i + 3],
        b[i + 4], b[i + 5], b[i + 6], b[i + 7],
    ])
}

/// Un `.bex` abierto por su indice de recursos.
pub struct Paquete {
    f: Archivo,
    /// Offset EN FICHERO donde empieza la seccion de recursos.
    ///
    /// ** Los offsets del indice son relativos a ESTO, no al fichero. Sumarlo
    /// es el paso que se olvida, y el sintoma no es un fallo: es leer los bytes
    /// de otro recurso y creerselos.
    base: u64,
    cuantos: u32,
}

impl Paquete {
    /// *** **MI propia caja**, y esta es la forma correcta.
    ///
    /// No lleva ruta. El kernel se acuerda de por donde entro este proceso, asi
    /// que el programa no dice CUAL: dice *"el mio"*. Un binario movido de sitio
    /// se sigue encontrando, y --lo que de verdad importa-- **no hay ninguna
    /// ruta que escribir**: quien puede escribir la suya puede escribir otra, y
    /// en un sistema de capabilities eso es justo lo que no se hace.
    ///
    /// `None` si este `.bex` no lleva recursos --que es el caso de casi todos--
    /// o si el kernel no recuerda de donde salio, que le pasa a los binarios
    /// que el propio kernel embebe. **Ninguno de los dos es un fallo.**
    pub fn mio() -> Option<Self> {
        Self::montar(Archivo::mi_imagen()?)
    }

    /// El paquete de OTRO, por su ruta.
    ///
    /// Sigue teniendo sentido --una herramienta listando lo que hay dentro de un
    /// `.bex`-- pero para leer los datos de UNO MISMO la buena es [`Paquete::mio`].
    pub fn abrir(ruta: &[u8]) -> Option<Self> {
        Self::montar(Archivo::leer_de(ruta).ok()?)
    }

    /// Con el fichero ya abierto: localiza la seccion `0x0B` y su indice.
    fn montar(f: Archivo) -> Option<Self> {
        let mut cab = [0u8; BEF_CABECERA as usize];
        f.saltar(0);
        if f.read(&mut cab) < cab.len() {
            return None;
        }
        if u32le(&cab, 0) != BEF_MAGIC {
            return None;
        }
        let tabla = u64le(&cab, 32);
        let count = u32le(&cab, 40) as u64;

        // La tabla, entrada a entrada, buscando la de recursos. Recorrido
        // lineal: las secciones de un `.bex` son unas pocas.
        let mut off = 0u64;
        let mut largo = 0u64;
        let mut ent = [0u8; BEF_ENTRADA as usize];
        for i in 0..count {
            f.saltar(tabla + i * BEF_ENTRADA);
            if f.read(&mut ent) < ent.len() {
                return None;
            }
            if ent[0] == SECCION_RECURSOS {
                off = u64le(&ent, 8);
                largo = u64le(&ent, 16);
                break;
            }
        }
        // ** Que un `.bex` NO lleve recursos no es un fallo: es lo que tienen
        // todos los de hoy menos `caja.bex` y `doom.bex`. Se contesta `None` y
        // quien llama sigue su camino.
        if largo == 0 {
            return None;
        }

        let mut idx = [0u8; BRES_CABECERA as usize];
        f.saltar(off);
        if f.read(&mut idx) < idx.len() || u32le(&idx, 0) != BRES_MAGIC {
            return None;
        }
        Some(Self { f, base: off, cuantos: u32le(&idx, 4) })
    }

    /// Cuantos recursos lleva.
    pub fn cuantos(&self) -> u32 {
        self.cuantos
    }

    /// El nombre del recurso `i`, en `dst`. Devuelve cuantos bytes escribio.
    ///
    /// Sirve para listar sin saber los nombres de antemano -- que es lo que hace
    /// una herramienta, y lo que no hace una app leyendo lo suyo.
    pub fn nombre(&self, i: u32, dst: &mut [u8]) -> usize {
        if i >= self.cuantos {
            return 0;
        }
        let mut ent = [0u8; BRES_ENTRADA as usize];
        self.f.saltar(self.base + BRES_CABECERA + i as u64 * BRES_ENTRADA);
        if self.f.read(&mut ent) < ent.len() {
            return 0;
        }
        let mut n = 0;
        while n < BRES_NOMBRE_MAX && n < dst.len() && ent[n] != 0 {
            dst[n] = ent[n];
            n += 1;
        }
        n
    }

    /// Busca por nombre y devuelve `(posicion en fichero, tamano)`.
    ///
    /// Recorrido lineal a proposito: un paquete tiene unidades de recursos, no
    /// miles, y una tabla ordenada aqui seria mas formato que validar a cambio
    /// de nada.
    pub fn buscar(&self, nombre: &[u8]) -> Option<(u64, u64)> {
        let mut ent = [0u8; BRES_ENTRADA as usize];
        for i in 0..self.cuantos as u64 {
            self.f.saltar(self.base + BRES_CABECERA + i * BRES_ENTRADA);
            if self.f.read(&mut ent) < ent.len() {
                return None;
            }
            // Comparacion con el NUL incluido: sin el, `saludo` casaria con
            // `saludo2` y se leerian los bytes del que no es.
            let cabe = nombre.len() <= BRES_NOMBRE_MAX;
            let igual = cabe
                && ent[..nombre.len()] == *nombre
                && ent[nombre.len()] == 0;
            if igual {
                // ** EL `+ self.base` ES EL PASO QUE SE OLVIDA. Los offsets del
                // indice son relativos a la seccion, no al fichero.
                return Some((self.base + u64le(&ent, 48), u64le(&ent, 56)));
            }
        }
        None
    }

    /// Lee el recurso `nombre` en `dst`. Devuelve cuantos bytes trajo.
    ///
    /// Trae **lo que quepa**: si `dst` es mas corto que el recurso, se lleva el
    /// principio y devuelve eso. No es un truncado silencioso -- quien llama
    /// puede comparar contra el tamano que dio [`Paquete::buscar`].
    pub fn leer(&self, nombre: &[u8], dst: &mut [u8]) -> usize {
        let (pos, tam) = match self.buscar(nombre) {
            Some(v) => v,
            None => return 0,
        };
        let cuanto = if tam < dst.len() as u64 { tam as usize } else { dst.len() };
        self.f.saltar(pos);
        self.f.read(&mut dst[..cuanto])
    }
}
