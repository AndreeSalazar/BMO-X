//! Registro de lenguajes activos en el sistema.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::descriptor::LangDescriptor;

pub struct LangRegistry<'a> {
    langs: &'a [LangDescriptor<'a>],
}

impl<'a> LangRegistry<'a> {
    pub const EMPTY: Self = Self { langs: &[] };

    pub const fn from_slice(langs: &'a [LangDescriptor<'a>]) -> Self {
        Self { langs }
    }

    pub fn lookup(&self, id: bx_u32) -> BxResult<&LangDescriptor<'a>> {
        for d in self.langs.iter() {
            if d.id == id { return Ok(d); }
        }
        Err(BxError::NotFound)
    }

    pub const fn count(&self) -> usize { self.langs.len() }
}
