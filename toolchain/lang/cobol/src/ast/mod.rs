pub mod data;
pub mod error;
pub mod program;
pub mod statements;

pub use data::DataItem;
pub use error::CobolError;
pub use program::SyscallDef;
pub use statements::{CobolCondition, CobolStatement};

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct CobolProgram {
    pub program_id: String,
    pub data_items: Vec<DataItem>,
    pub statements: Vec<CobolStatement>,
}

impl CobolProgram {
    pub fn new(program_id: String) -> Self {
        CobolProgram { program_id, data_items: Vec::new(), statements: Vec::new() }
    }

    pub fn add_data_item(&mut self, item: DataItem) {
        self.data_items.push(item);
    }

    pub fn add_statement(&mut self, stmt: CobolStatement) {
        self.statements.push(stmt);
    }
}

pub type SyscallMap = HashMap<String, SyscallDef>;
