use gnucobol_rs::pic::{self, PicField, Usage};
use crate::ast::error::CobolError;

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
        let usage = Usage::Display;
        let pic_field = pic.as_deref().and_then(|p| {
            pic::build_field(p, usage, false, false).ok()
        });
        DataItem { level, name, pic, pic_field, value, usage }
    }

    pub fn new_with_usage(
        level: u32,
        name: String,
        pic: Option<String>,
        value: Option<String>,
        usage: Usage,
    ) -> Self {
        let pic_field = pic.as_deref().and_then(|p| {
            pic::build_field(p, usage, false, false).ok()
        });
        DataItem { level, name, pic, pic_field, value, usage }
    }

    pub fn storage_size(&self) -> usize {
        self.pic_field.map(|p| p.size).unwrap_or(8)
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

        if let Some(ref p) = pic_str {
            match pic::build_field(p, Usage::Display, false, false) {
                Ok(field) => item.pic_field = Some(field),
                Err(e) => {
                    return Err(CobolError::new(0, format!("invalid PIC '{}': {}", p, e)));
                }
            }
        }
        Ok(item)
    }
}
