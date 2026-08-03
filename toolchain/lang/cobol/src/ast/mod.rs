pub mod data;
pub mod error;
pub mod program;
pub mod statements;

pub use data::{DataItem, Valor88};
pub use error::CobolError;
pub use program::SyscallDef;
pub use statements::{CobolCondition, CobolStatement, Condicion, DisplayArg};

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

/// Un PÁRRAFO de la PROCEDURE DIVISION: un nombre y lo que hace.
///
/// Es la unidad en la que se escribe COBOL de verdad. Un batch bancario tiene
/// un cuerpo principal de cinco `PERFORM` legibles y el trabajo repartido en
/// `1000-INICIO`, `2000-PROCESO`, `3000-CIERRE`. Sin párrafos, un programa es
/// una lista plana de sentencias y no hay forma de escribir eso.
#[derive(Debug, Clone, PartialEq)]
pub struct Parrafo {
    pub nombre: String,
    pub statements: Vec<CobolStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CobolProgram {
    pub program_id: String,
    pub data_items: Vec<DataItem>,
    /// Los ficheros de `FILE-CONTROL`, en orden de declaración.
    pub files: Vec<CobolFile>,
    /// El CUERPO PRINCIPAL: lo que hay antes del primer nombre de párrafo.
    ///
    /// Si está vacío —porque el programa empieza directamente con un párrafo—
    /// el codegen arranca ejecutando el primero. Es la otra forma de escribir
    /// lo mismo y las dos son corrientes.
    pub statements: Vec<CobolStatement>,
    /// Los párrafos, **en el orden en que se escribieron**. El orden manda: un
    /// `PERFORM A THRU B` ejecuta todo lo que hay entre los dos.
    pub parrafos: Vec<Parrafo>,
}

impl CobolProgram {
    pub fn new(program_id: String) -> Self {
        CobolProgram {
            program_id,
            data_items: Vec::new(),
            files: Vec::new(),
            statements: Vec::new(),
            parrafos: Vec::new(),
        }
    }

    /// El índice del párrafo con ese nombre, si existe.
    pub fn parrafo(&self, nombre: &str) -> Option<usize> {
        self.parrafos.iter().position(|p| p.nombre.eq_ignore_ascii_case(nombre))
    }

    /// El fichero declarado con ese nombre, si lo hay.
    pub fn file(&self, name: &str) -> Option<&CobolFile> {
        self.files.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    pub fn add_data_item(&mut self, item: DataItem) {
        self.data_items.push(item);
    }

    /// Añade al sitio que toca: al párrafo abierto, o al cuerpo principal si
    /// todavía no hay ninguno.
    pub fn add_statement(&mut self, stmt: CobolStatement) {
        match self.parrafos.last_mut() {
            Some(p) => p.statements.push(stmt),
            None => self.statements.push(stmt),
        }
    }

    pub fn abrir_parrafo(&mut self, nombre: String) {
        self.parrafos.push(Parrafo { nombre, statements: Vec::new() });
    }
}

pub type SyscallMap = HashMap<String, SyscallDef>;
