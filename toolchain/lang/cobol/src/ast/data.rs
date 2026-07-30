use crate::ast::error::CobolError;
use crate::edicion::Plantilla;
use crate::pic::{parse_pic, PicField, Usage};

#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub level: u32,
    pub name: String,
    pub pic: Option<String>,
    pub pic_field: Option<PicField>,
    /// La plantilla de edición, si la PIC es de PRESENTACIÓN
    /// (`$$$,$$9.99`) en vez de de cálculo (`S9(7)V99`). Lo que la lleva no
    /// se guarda distinto —sigue siendo un entero escalado— pero al
    /// enseñarlo no se escribe el número: se escribe la máscara.
    pub edicion: Option<Plantilla>,
    pub value: Option<String>,
    pub usage: Usage,
    /// `OCCURS <n> TIMES` — cuántas veces se repite el dato.
    ///
    /// `None` = un dato suelto. `Some(n)` = una TABLA de `n` elementos, y
    /// entonces el nombre **exige subíndice**: `TOTAL(I)`. Un `MOVE 0 TO
    /// TOTAL` sobre una tabla no es "el primero", es una pregunta sin
    /// respuesta, y se rechaza.
    pub occurs: Option<u32>,
}

/// Analiza una PIC decidiendo primero de cuál de las dos familias es.
///
/// Son dos gramáticas distintas y por eso hay dos analizadores: `parse_pic`
/// sabe de `9`, `S` y `V` —cuántos dígitos y dónde cae la coma— y se atraganta
/// con un `$`. Mandarle una PIC editada devolvía error, y como el error se
/// tragaba con `.ok()`, el dato acababa con escala 0: `MOVE 19.99` guardaba
/// 19 y los centavos desaparecían sin que nadie dijera nada.
fn analizar_pic(pic: &str, usage: Usage) -> Result<(PicField, Option<Plantilla>), String> {
    if !Plantilla::es_editada(pic) {
        return Ok((parse_pic(pic, usage)?, None));
    }
    let plantilla = Plantilla::parse(pic)?;
    let escala = plantilla.escala;
    let campo = PicField {
        integer_digits: plantilla.digitos() as u32 - escala,
        scale: escala,
        // Una PIC editada puede enseñar signo (`-`, `CR`, `DB`), así que el
        // dato que la alimenta se guarda con signo. Al revés —guardarlo sin
        // signo— un saldo en rojo saldría en verde.
        signed: true,
        numeric: true,
        char_count: 0,
        usage,
    };
    Ok((campo, Some(plantilla)))
}

impl DataItem {
    pub fn new(level: u32, name: String, pic: Option<String>, value: Option<String>) -> Self {
        Self::new_with_usage(level, name, pic, value, Usage::Display)
    }

    pub fn new_with_usage(
        level: u32,
        name: String,
        pic: Option<String>,
        value: Option<String>,
        usage: Usage,
    ) -> Self {
        let (pic_field, edicion) = match pic.as_deref().map(|p| analizar_pic(p, usage)) {
            Some(Ok((campo, plantilla))) => (Some(campo), plantilla),
            _ => (None, None),
        };
        DataItem { level, name, pic, pic_field, edicion, value, usage, occurs: None }
    }

    /// Bytes de almacenamiento del item (mínimo 8, alineado por el codegen).
    pub fn storage_size(&self) -> usize {
        self.pic_field.as_ref().map(|p| p.size()).unwrap_or(8)
    }

    /// Cuántos elementos tiene. Un dato suelto es una tabla de uno.
    pub fn elementos(&self) -> u32 {
        self.occurs.unwrap_or(1)
    }

    /// Escala decimal (dígitos tras la V). 0 = entero. Es la llave del
    /// decimal exacto: el codegen escala los operandos a esta escala.
    pub fn scale(&self) -> u32 {
        self.pic_field.as_ref().map(|p| p.scale).unwrap_or(0)
    }
}

impl DataItem {
    pub fn from_parsed(
        level: u32,
        name: String,
        pic_str: Option<&str>,
        value: Option<&str>,
    ) -> Result<Self, CobolError> {
        let pic = pic_str.map(|s| s.to_string());
        let val = value.map(|s| s.to_string());
        let mut item = DataItem::new(level, name, pic, val);

        if let Some(p) = pic_str {
            match analizar_pic(p, Usage::Display) {
                Ok((field, plantilla)) => {
                    item.pic_field = Some(field);
                    item.edicion = plantilla;
                }
                Err(e) => return Err(CobolError::new(0, format!("invalid PIC '{p}': {e}"))),
            }
        }
        Ok(item)
    }
}
