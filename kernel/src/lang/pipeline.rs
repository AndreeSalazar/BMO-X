#![allow(dead_code)]

pub enum SourceLang { Bmo, C }

pub fn compile(_src: &str, _lang: SourceLang) -> Result<alloc::vec::Vec<u8>, &'static str> {
    Err("lang::compile: stub")
}
