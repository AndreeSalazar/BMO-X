use crate::ast::error::CobolError;
use crate::pic::{parse_pic, PicField, Usage};

#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub level: u32,
    pub name: String,
    pub pic: Option<String>,
    pub pic_field: Option<PicField>,
    pub value: Option<String>,
    pub usage: Usage,
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
        let pic_field = pic.as_deref().and_then(|p| parse_pic(p, usage).ok());
        DataItem { level, name, pic, pic_field, value, usage }
    }

    /// Bytes de almacenamiento del item (mínimo 8, alineado por el codegen).
    pub fn storage_size(&self) -> usize {
        self.pic_field.as_ref().map(|p| p.size()).unwrap_or(8)
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
            match parse_pic(p, Usage::Display) {
                Ok(field) => item.pic_field = Some(field),
                Err(e) => return Err(CobolError::new(0, format!("invalid PIC '{p}': {e}"))),
            }
        }
        Ok(item)
    }
}
