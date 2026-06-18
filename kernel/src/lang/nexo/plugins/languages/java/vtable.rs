//! Java vtable — generates vtable structures and lookup helpers.
//!
//! Each Java class with virtual methods gets a `{ClassName}_vtable`
//! struct that holds function pointers. `new ClassName` allocates
//! space for the vtable pointer and fills it with the class vtable.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use super::ast::*;

/// One entry in a vtable: name of the virtual method + offset.
#[derive(Debug, Clone)]
pub struct VTableEntry {
    pub method_name: String,
    /// Byte offset within the vtable struct.
    pub offset: u32,
}

/// Build the vtable layout for a class: list of (name, offset) pairs
/// in declaration order (single inheritance).
pub fn build_vtable_layout(cls: &JClass, parent_layout: &[VTableEntry]) -> Vec<VTableEntry> {
    let mut out: Vec<VTableEntry> = parent_layout.to_vec();
    let mut next_offset: u32 = (parent_layout.len() as u32) * 8;
    for m in &cls.members {
        if let JMemberKind::Method { name, is_abstract, .. } = &m.kind {
            if !*is_abstract {
                out.push(VTableEntry { method_name: name.clone(), offset: next_offset });
                next_offset += 8;
            }
        }
    }
    out
}

