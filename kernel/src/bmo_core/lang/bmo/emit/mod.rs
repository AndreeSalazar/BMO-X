extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

pub const NOP: u8 = 0x00;
pub const PUSH_IMM64: u8 = 0x01;
pub const POP: u8 = 0x02;
pub const ADD: u8 = 0x03;
pub const SUB: u8 = 0x04;
pub const MUL: u8 = 0x05;
pub const DIV: u8 = 0x06;
pub const MOD: u8 = 0x07;
pub const AND: u8 = 0x08;
pub const OR: u8 = 0x09;
pub const XOR: u8 = 0x0A;
pub const SHL: u8 = 0x0B;
pub const SHR: u8 = 0x0C;
pub const EQ: u8 = 0x0D;
pub const NE: u8 = 0x0E;
pub const LT: u8 = 0x0F;
pub const GT: u8 = 0x10;
pub const LE: u8 = 0x11;
pub const GE: u8 = 0x12;
pub const LAND: u8 = 0x13;
pub const LOR: u8 = 0x14;
pub const NOT: u8 = 0x15;
pub const NEG: u8 = 0x16;
pub const LOAD_LOCAL: u8 = 0x17;
pub const STORE_LOCAL: u8 = 0x18;
pub const CALL: u8 = 0x19;
pub const RET: u8 = 0x1A;
pub const JMP: u8 = 0x1B;
pub const JMP_IF_FALSE: u8 = 0x1C;
pub const SYS_CALL: u8 = 0x1D;
pub const DUP: u8 = 0x1E;
pub const SWAP: u8 = 0x1F;
pub const HALT: u8 = 0xFF;

pub struct Emitter {
    code: Vec<u8>,
    locals: BTreeMap<String, u8>,
    next_local: u8,
}

impl Emitter {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            locals: BTreeMap::new(),
            next_local: 0,
        }
    }

    pub fn emit_byte(&mut self, op: u8) {
        self.code.push(op);
    }

    pub fn emit_imm64(&mut self, val: u64) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }

    pub fn emit_imm16(&mut self, val: i16) {
        self.code.extend_from_slice(&val.to_le_bytes());
    }

    pub fn emit_local_index(&mut self, op: u8, name: &str) {
        let slot = self.get_or_create_local(name);
        self.code.push(op);
        self.code.push(slot);
    }

    pub fn get_or_create_local(&mut self, name: &str) -> u8 {
        if let Some(&slot) = self.locals.get(name) {
            slot
        } else {
            let slot = self.next_local;
            self.next_local += 1;
            self.locals.insert(alloc::string::String::from(name), slot);
            slot
        }
    }

    pub fn current_offset(&self) -> usize {
        self.code.len()
    }

    pub fn patch_jump(&mut self, offset: usize, target: i16) {
        let bytes = target.to_le_bytes();
        self.code[offset] = bytes[0];
        self.code[offset + 1] = bytes[1];
    }

    pub fn into_code(self) -> Vec<u8> {
        self.code
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self::new()
    }
}
