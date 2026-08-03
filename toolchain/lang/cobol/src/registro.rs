//! La DISPOSICIÓN de un registro: qué byte ocupa cada campo dentro de su `01`.
//!
//! # Por qué NO se reutiliza `bmo_abi::types::disposicion`
//!
//! Aquella coloca miembros **alineados**, que es lo que manda el ABI de SysV
//! para un agregado de C. Aquí eso sería **veneno**: la disposición de un
//! registro COBOL *es el formato del fichero*. Un byte de relleno entre dos
//! campos no es una optimización de acceso — es un byte que aparece en el disco
//! y que el sistema de al lado va a leer como si fuera un dígito.
//!
//! Así que un registro va **byte a byte, sin huecos**, y por eso vive aquí. Es
//! la excepción que confirma la regla de la casa: se comparte la REGLA cuando
//! es la misma, y ésta no lo es.
//!
//! # El modelo, y qué decide (camino B de `PLAN_BANCA.md` §1.0)
//!
//! ```text
//!   01 REG-CUENTA.                    offset  bytes
//!       05 CTA-NUMERO  PIC 9(10).          0     10
//!       05 CTA-SALDO   PIC S9(7)V99 COMP-3. 10      6
//!       05 CTA-ESTADO  PIC 9.              16      1
//!                                          ─────────
//!                              REG-CUENTA:  0     17
//! ```
//!
//! Cada `01` que es un GRUPO tiene un **área de registro** de ese tamaño, y
//! cada campo de dentro **conserva además su ranura de trabajo** de 64 bits. El
//! área es la representación EXTERNA —lo que va y viene del disco— y la ranura
//! es donde se calcula.
//!
//! Eso no es un rodeo para no tocar el almacenamiento: es lo que dice COBOL. El
//! área de registro sólo vale entre un `READ` y el siguiente, y por eso la
//! traducción entre las dos vive exactamente en esos dos puntos.
//!
//! # El tamaño de un campo lo decide su USAGE
//!
//! Y ahí ya no hay nada que inventar: `PicField::size()` lo sabe desde que
//! entró `COMP-3`. Un `PIC S9(7)V99 COMP-3` mide 6 bytes en el disco de un
//! mainframe y mide 6 aquí.

use std::collections::HashMap;

use crate::ast::{CobolError, DataItem};

/// Dónde vive un campo dentro de su registro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    /// El `01` al que pertenece. Un dato suelto es su propia raíz.
    pub raiz: String,
    /// Byte en el que empieza, contando desde el principio del `01`.
    pub offset: u32,
    /// Cuántos bytes ocupa. Para un grupo, la suma de los de dentro.
    pub bytes: u32,
    /// Un GRUPO no tiene PIC: es el nombre de lo que hay debajo.
    pub es_grupo: bool,
    /// Nivel con el que se declaró. Se guarda para poder decir el porqué en los
    /// errores sin volver a buscar el dato.
    pub nivel: u32,
}

/// La disposición de todos los datos de un programa.
#[derive(Debug, Default, Clone)]
pub struct Disposicion {
    campos: HashMap<String, Campo>,
    /// Las raíces en orden de declaración: el orden manda para reservar.
    raices: Vec<String>,
}

impl Disposicion {
    pub fn campo(&self, nombre: &str) -> Option<&Campo> {
        self.campos.get(&nombre.to_ascii_uppercase())
    }

    /// Los `01` en orden de declaración.
    pub fn raices(&self) -> &[String] {
        &self.raices
    }

    /// ¿Es un grupo, o sea algo que se mueve como un bloque de bytes?
    pub fn es_grupo(&self, nombre: &str) -> bool {
        self.campo(nombre).map(|c| c.es_grupo).unwrap_or(false)
    }

    /// Los campos ELEMENTALES de un grupo, en orden de byte.
    ///
    /// Es lo que recorren el empaquetado y el desempaquetado del área, y va en
    /// orden de offset y no de declaración a propósito: así el emisor escribe
    /// el área de izquierda a derecha y un `REDEFINES` futuro no puede colar un
    /// campo fuera de sitio sin que se note.
    pub fn hojas_de(&self, raiz: &str) -> Vec<(&String, &Campo)> {
        let raiz = raiz.to_ascii_uppercase();
        let mut v: Vec<_> = self
            .campos
            .iter()
            .filter(|(_, c)| !c.es_grupo && c.raiz == raiz)
            .collect();
        v.sort_by_key(|(_, c)| c.offset);
        v
    }
}

/// Calcula la disposición de todos los datos, en orden de declaración.
///
/// ## Cómo se cierra un grupo
///
/// Un grupo no dice cuánto mide: se sabe cuando aparece algo de nivel **igual o
/// menor**, que es lo que lo cierra. Por eso hay una pila — y por eso el
/// recorrido tiene que ser en orden de declaración, que es el único sitio donde
/// esa información existe.
pub fn calcular(items: &[DataItem]) -> Result<Disposicion, CobolError> {
    let mut d = Disposicion::default();
    // (nivel, nombre, offset donde empezó)
    let mut pila: Vec<(u32, String, u32)> = Vec::new();
    let mut cursor = 0u32;
    let mut raiz = String::new();

    for item in items {
        // Un 88 es un apodo de una comparación: no ocupa ni un byte y no entra
        // en la disposición de nadie.
        if item.level == 88 {
            continue;
        }
        let nombre = item.name.to_ascii_uppercase();

        // Cerrar los grupos que este nivel deja atrás.
        while pila.last().map(|(n, _, _)| *n >= item.level).unwrap_or(false) {
            let (_, cerrado, ini) = pila.pop().unwrap();
            if let Some(c) = d.campos.get_mut(&cerrado) {
                c.bytes = cursor - ini;
            }
        }

        // Sin nada abierto, esto empieza un registro nuevo.
        if pila.is_empty() {
            cursor = 0;
            raiz = nombre.clone();
            d.raices.push(nombre.clone());
        }

        if d.campos.contains_key(&nombre) {
            return Err(CobolError::new(
                0,
                format!(
                    "'{nombre}' esta declarado dos veces: con dos campos del mismo nombre \
                     no hay forma de saber a cual va un MOVE"
                ),
            ));
        }

        let offset = cursor;
        let es_grupo = item.pic.is_none();
        d.campos.insert(
            nombre.clone(),
            Campo { raiz: raiz.clone(), offset, bytes: 0, es_grupo, nivel: item.level },
        );

        if es_grupo {
            // Su tamaño se sabrá al cerrarlo.
            pila.push((item.level, nombre, offset));
        } else {
            // ★ Sin alineado: los bytes van pegados, porque esto es el formato
            // del fichero y un hueco aquí es un byte de más en el disco.
            let bytes = item.storage_size() as u32 * item.elementos();
            d.campos.get_mut(&nombre).unwrap().bytes = bytes;
            cursor += bytes;
        }
    }

    // Y cerrar lo que quede abierto al final del programa.
    while let Some((_, cerrado, ini)) = pila.pop() {
        if let Some(c) = d.campos.get_mut(&cerrado) {
            c.bytes = cursor - ini;
        }
    }

    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pic::Usage;

    fn dato(nivel: u32, nombre: &str, pic: Option<&str>) -> DataItem {
        DataItem::new(nivel, nombre.into(), pic.map(|p| p.into()), None)
    }

    fn comp3(nivel: u32, nombre: &str, pic: &str) -> DataItem {
        DataItem::new_with_usage(nivel, nombre.into(), Some(pic.into()), None, Usage::Comp3)
    }

    /// ★ El registro de una cuenta, con los tres tipos mezclados. Es el caso
    /// que decide: los offsets tienen que caer **pegados**, sin relleno, porque
    /// esto es lo que va al disco.
    #[test]
    fn los_campos_van_pegados_sin_relleno() {
        let items = vec![
            dato(1, "REG-CUENTA", None),
            dato(5, "CTA-NUMERO", Some("9(10)")),
            comp3(5, "CTA-SALDO", "S9(7)V99"),
            dato(5, "CTA-ESTADO", Some("9")),
        ];
        let d = calcular(&items).unwrap();

        assert_eq!(d.campo("CTA-NUMERO").unwrap().offset, 0);
        assert_eq!(d.campo("CTA-NUMERO").unwrap().bytes, 10);

        // COMP-3 de 9 dígitos = 9/2+1 = 5 bytes. Empieza justo detrás.
        assert_eq!(d.campo("CTA-SALDO").unwrap().offset, 10);
        assert_eq!(d.campo("CTA-SALDO").unwrap().bytes, 5);

        assert_eq!(d.campo("CTA-ESTADO").unwrap().offset, 15);
        assert_eq!(d.campo("CTA-ESTADO").unwrap().bytes, 1);

        // Y el grupo mide la suma, ni un byte más.
        let reg = d.campo("REG-CUENTA").unwrap();
        assert!(reg.es_grupo);
        assert_eq!(reg.offset, 0);
        assert_eq!(reg.bytes, 16, "un registro con relleno no es el del fichero");
    }

    /// Grupos DENTRO de grupos: el nivel 10 cierra cuando llega otro 05.
    #[test]
    fn los_grupos_se_anidan_y_se_cierran_solos() {
        let items = vec![
            dato(1, "REG", None),
            dato(5, "NOMBRE-COMPLETO", None),
            dato(10, "NOMBRE", Some("9(4)")),
            dato(10, "APELLIDO", Some("9(6)")),
            dato(5, "EDAD", Some("9(3)")),
        ];
        let d = calcular(&items).unwrap();

        assert_eq!(d.campo("NOMBRE").unwrap().offset, 0);
        assert_eq!(d.campo("APELLIDO").unwrap().offset, 4);
        // El grupo de en medio mide lo suyo y se cerró al llegar el 05 EDAD.
        let nc = d.campo("NOMBRE-COMPLETO").unwrap();
        assert!(nc.es_grupo);
        assert_eq!((nc.offset, nc.bytes), (0, 10));
        // Y EDAD sigue detrás, no encima.
        assert_eq!(d.campo("EDAD").unwrap().offset, 10);
        assert_eq!(d.campo("REG").unwrap().bytes, 13);
    }

    /// Cada `01` empieza en su propio cero: son registros distintos, no un
    /// bloque continuo.
    #[test]
    fn cada_registro_empieza_en_su_propio_cero() {
        let items = vec![
            dato(1, "UNO", None),
            dato(5, "A", Some("9(4)")),
            dato(1, "DOS", None),
            dato(5, "B", Some("9(4)")),
        ];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("A").unwrap().offset, 0);
        assert_eq!(d.campo("B").unwrap().offset, 0, "el segundo 01 arranco donde acabo el primero");
        assert_eq!(d.campo("A").unwrap().raiz, "UNO");
        assert_eq!(d.campo("B").unwrap().raiz, "DOS");
        assert_eq!(d.raices(), &["UNO".to_string(), "DOS".to_string()]);
    }

    /// Un `01` con PIC es un dato suelto: es su propia raíz y mide lo suyo.
    #[test]
    fn un_01_con_pic_es_un_dato_suelto() {
        let items = vec![dato(1, "SALDO", Some("S9(7)V99"))];
        let d = calcular(&items).unwrap();
        let c = d.campo("SALDO").unwrap();
        assert!(!c.es_grupo);
        assert_eq!((c.raiz.as_str(), c.offset), ("SALDO", 0));
    }

    /// Un `OCCURS` ocupa sus `n` veces dentro del registro, y lo que viene
    /// detrás empieza pasada la tabla entera.
    #[test]
    fn un_occurs_ocupa_todas_sus_veces() {
        let mut tabla = dato(5, "MES", Some("9(4)"));
        tabla.occurs = Some(12);
        let items = vec![dato(1, "REG", None), tabla, dato(5, "TOTAL", Some("9(6)"))];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("MES").unwrap().bytes, 4 * 12);
        assert_eq!(d.campo("TOTAL").unwrap().offset, 48);
    }

    /// Los 88 no ocupan nada, y por eso no pueden mover el campo de al lado.
    #[test]
    fn un_88_no_mueve_a_nadie() {
        let mut ochenta = dato(88, "ES-BUENO", None);
        ochenta.value = Some("1".into());
        let items = vec![
            dato(1, "REG", None),
            dato(5, "VALE", Some("9")),
            ochenta,
            dato(5, "OTRO", Some("9(4)")),
        ];
        let d = calcular(&items).unwrap();
        assert_eq!(d.campo("OTRO").unwrap().offset, 1);
        assert_eq!(d.campo("REG").unwrap().bytes, 5);
        assert!(d.campo("ES-BUENO").is_none());
    }

    /// Las hojas salen en orden de BYTE, que es el orden en que se escribe el
    /// área — no en el de declaración ni en el que quiera un `HashMap`.
    #[test]
    fn las_hojas_salen_en_orden_de_byte() {
        let items = vec![
            dato(1, "REG", None),
            dato(5, "A", Some("9")),
            dato(5, "B", Some("9(3)")),
            dato(5, "C", Some("9(2)")),
        ];
        let d = calcular(&items).unwrap();
        let nombres: Vec<&str> = d.hojas_de("REG").iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(nombres, vec!["A", "B", "C"]);
    }

    /// Dos campos con el mismo nombre no se pueden distinguir en un `MOVE`. En
    /// COBOL de verdad se resuelve con `A OF REG`, que todavía no existe — así
    /// que se dice en vez de quedarse con uno de los dos.
    #[test]
    fn dos_campos_con_el_mismo_nombre_se_rechazan() {
        let items = vec![
            dato(1, "UNO", None),
            dato(5, "IMPORTE", Some("9(4)")),
            dato(1, "DOS", None),
            dato(5, "IMPORTE", Some("9(4)")),
        ];
        let err = calcular(&items).unwrap_err();
        assert!(err.message.contains("dos veces"), "{}", err.message);
    }
}
