extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use super::super::parser::ast::Type;

#[derive(Debug, Clone)]
pub struct ScopeEntry {
    pub name: String,
    pub ty: Type,
    /// Offset relativo al frame pointer (RBP) en bytes.
    pub frame_offset: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub entries: Vec<ScopeEntry>,
    /// Bytes ya reservados en el frame actual.
    pub frame_size: u32,
}

impl Scope {
    pub fn lookup(&self, name: &str) -> Option<&ScopeEntry> {
        self.entries.iter().rev().find(|e| e.name == name)
    }

    pub fn push(&mut self, entry: ScopeEntry) {
        self.entries.push(entry);
    }
}
