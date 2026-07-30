pub mod data;
pub mod error;
pub mod program;
pub mod statements;

pub use data::DataItem;
pub use error::CobolError;
pub use program::SyscallDef;
pub use statements::{CobolCondition, CobolStatement, DisplayArg};

use std::collections::HashMap;

/// Un fichero declarado en `FILE-CONTROL`.
///
/// `SELECT` le pone nombre y le asigna una RUTA; `FD` le da un registro. Los
/// dos hacen falta: sin ruta no hay qué abrir, y sin registro no hay dónde
/// dejar lo leído.
#[derive(Debug, Clone, PartialEq)]
pub struct CobolFile {
    /// El nombre con el que lo llaman `OPEN`, `READ` y `CLOSE`.
    pub name: String,
    /// La ruta en el volumen de datos, tal cual la escribió el `ASSIGN TO`.
    pub path: String,
    /// El `01` que va debajo del `FD`. Vacío si no se declaró.
    pub record: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CobolProgram {
    pub program_id: String,
    pub data_items: Vec<DataItem>,
    /// Los ficheros de `FILE-CONTROL`, en orden de declaración.
    pub files: Vec<CobolFile>,
    pub statements: Vec<CobolStatement>,
}

impl CobolProgram {
    pub fn new(program_id: String) -> Self {
        CobolProgram {
            program_id,
            data_items: Vec::new(),
            files: Vec::new(),
            statements: Vec::new(),
        }
    }

    /// El fichero declarado con ese nombre, si lo hay.
    pub fn file(&self, name: &str) -> Option<&CobolFile> {
        self.files.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    pub fn add_data_item(&mut self, item: DataItem) {
        self.data_items.push(item);
    }

    pub fn add_statement(&mut self, stmt: CobolStatement) {
        self.statements.push(stmt);
    }
}

pub type SyscallMap = HashMap<String, SyscallDef>;
