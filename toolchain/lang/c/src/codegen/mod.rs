use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::bef::relocations::{Relocation, SEC_DATA, SEC_RODATA};
use crate::ast::*;
use crate::CError;

/// Structs y uniones POR VALOR, en su propio fichero. Ver su cabecera para
/// la ABI de agregados de BMO y para que hacen SysV y Win64 con esto mismo.
mod agregados;
/// La ENTRADA de C (`getchar`, `scanf`), tambien aparte. Escribir es empujar
/// bytes; leer es ESPERAR, guardar lo que sobra y decidir que significa lo que
/// alguien tecleo. Tres problemas que la salida no tiene.
mod entrada;
/// El CATALOGO de funciones sintetizadas: nombre -> los bytes que lo
/// implementan. Salio de aqui porque dentro no se sabe que es una expresion de
/// C -- solo hay nombres y codigo -- y esa frontera se ve en que ese fichero no
/// escribe `self` ni una vez. La PASADA sobre las relocs se queda en este.
mod sintetizadas;

type Result<T> = core::result::Result<T, CError>;

/// Target execution environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetProfile {
    /// Ring 0. **NO COMPILA, y se dice por que** -- ver
    /// [`Codegen::emit_call_to_syscall_stub`].
    ///
    /// Se conserva la variante en vez de borrarla porque la pregunta *"puedo
    /// compilar C para Ring 0?"* se la va a hacer alguien otra vez, y un
    /// rechazo con su motivo contesta mejor que un enum donde la opcion no
    /// aparece. Es la misma decision que tomo COBOL con el File I/O antes de
    /// tenerlo: rechazar diciendolo, en vez de compilar un `READ` que no lee.
    Ring0Kernel,
    /// Ring 3: emit `__bmo_syscall_stub` and call through it.
    Ring3App,
}

impl Default for TargetProfile {
    fn default() -> Self { TargetProfile::Ring3App }
}

pub fn compile_to_bef_bytes(program: &Program) -> Result<Vec<u8>> {
    compile_with_target(program, TargetProfile::default())
}

pub fn compile_to_bef_bytes_filtered(program: &Program, used: &[String]) -> Result<Vec<u8>> {
    let mut filtered = program.clone();
    filtered.functions.retain(|f| f.name == "main" || used.contains(&f.name));
    compile_with_target(&filtered, TargetProfile::default())
}

pub fn compile_with_target(program: &Program, target: TargetProfile) -> Result<Vec<u8>> {
    let mut cg = Codegen::new(target);
    cg.emit_program(program)?;
    Ok(cg.build_bef())
}

struct Fixup {
    lea_offset: usize,
    string_idx: usize,
}

struct PendingReloc {
    offset: usize,
    target_label: u32,
}

struct CallReloc {
    offset: usize,
    target: String,
}

struct Codegen {
    target: TargetProfile,
    code: Vec<u8>,
    strings: Vec<String>,
    fixups: Vec<Fixup>,
    labels: u32,
    /// Offset donde quedo fijada cada etiqueta (para saltos hacia atras).
    label_offsets: HashMap<u32, usize>,
    pending_relocs: Vec<PendingReloc>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    /// Nombres de TODAS las funciones del programa (para distinguir una
    /// llamada directa de una indirecta por puntero, y la decadencia
    /// funcion->direccion).
    known_functions: std::collections::HashSet<String>,
    /// Los TIPOS de los parametros y del retorno de cada funcion.
    ///
    /// Hacia falta desde que un argumento puede no caber en un registro: el
    /// llamante tiene que saber cuantas ranuras empuja, y eso lo dice el
    /// PARAMETRO, no la expresion que se le pasa. Antes solo se guardaban los
    /// nombres, asi que pasar un struct empujaba una palabra y la funcion
    /// recibia el primer campo y basura detras -- sin una palabra de aviso.
    firmas: std::collections::HashMap<String, (Vec<TypeSpec>, TypeSpec)>,
    /// Sitios donde hay que escribir la direccion (rip-relativa) de una
    /// funcion: `lea rax, [rip+func]`. Habilita punteros a funcion.
    func_addr_fixups: Vec<(usize, String)>,
    break_target: Vec<u32>,
    continue_target: Vec<u32>,
    var_offsets: HashMap<String, (i32, TypeSpec)>,
    // bytes de stack locales de la funcion actual (arrays/structs con tamano REAL)
    frame_size: i32,
    /// La funcion que se esta emitiendo declara `...`?
    es_variadica: bool,
    /// Ranuras que ocupan sus parametros CON NOMBRE. Justo detras empiezan los
    /// variadicos, porque los argumentos van seguidos en la pila.
    ranuras_con_nombre: i32,
    struct_layouts: HashMap<String, Vec<(String, u32, u32)>>,
    struct_sizes: HashMap<String, u32>,
    label_positions: HashMap<String, usize>,
    goto_relocs: Vec<(usize, String)>,
    entry_offset: usize,
    is_entry_function: bool,
    global_offsets: HashMap<String, (u32, TypeSpec)>,
    global_data: Vec<u8>,
    global_fixups: Vec<(usize, String)>,
    /// * Punteros DENTRO de `.data` que hay que rellenar con la direccion de una
    /// cadena de `.rodata`: `(offset en .data, indice en `strings`)`.
    ///
    /// Son relocations, no fixups del compilador, y la diferencia es el motivo
    /// de que exista esta lista: un `lea [rip+disp]` lo puede resolver el
    /// compilador porque la distancia entre dos secciones de la misma imagen es
    /// fija, pero **un puntero guardado en un dato necesita la direccion
    /// absoluta**, y esa depende de donde cargue el programa. Eso solo lo sabe
    /// el cargador.
    ///
    /// Se acumulan aqui y se convierten en `Relocation` en `patch_all_fixups`,
    /// que es donde ya se conocen los offsets de las cadenas.
    relocs_a_cadena: Vec<(u32, usize)>,
    /// * Este programa RECLAMA LA PANTALLA. Lo deduce el compilador; acaba en
    /// `BefFlags::WANTS_SCREEN` y lo lee el compositor antes de lanzarlo.
    quiere_pantalla: bool,
    /// Las relocations ya resueltas que van en la seccion `Relocs` del BEF.
    relocs: Vec<bmo_abi::bef::relocations::Relocation>,
    instruction_end: usize,
    string_data_end: usize,
    /// Functions from userland_ring3 that need imports.
    stdlib_imports: std::collections::HashSet<String>,
    /// Enum constants: name -> integer value.
    enum_values: HashMap<String, i64>,
    /// Tabla de instrucciones sem-asm (opcodes leidos de la TOML de forge).
    isa: bmo_sem_asm::Instructions,
    /// Tabla de intrinsecos (la fusion __nombre() <-> bytes exactos).
    intrinsics: bmo_sem_asm::Intrinsics,
    /// Errores acumulados durante la emision (p.ej. intrinseco desconocido) --
    /// el compilador FALLA con mensaje, jamas emite bytes adivinados.
    errors: Vec<String>,
}

impl Codegen {
    /// Emite bytes con el encoder sem-asm (opcode de la tabla + REX/ModRM).
    fn emit_asm(&mut self, build: impl FnOnce(&mut bmo_sem_asm::x86_64::Asm)) {
        let mut a = bmo_sem_asm::x86_64::Asm::new(&self.isa);
        build(&mut a);
        self.code.extend_from_slice(a.bytes());
    }

    fn new(target: TargetProfile) -> Self {
        Self {
            target,
            code: Vec::new(), strings: Vec::new(), fixups: Vec::new(),
            labels: 0, label_offsets: HashMap::new(), pending_relocs: Vec::new(), call_relocs: Vec::new(),
            function_offsets: HashMap::new(),
            known_functions: std::collections::HashSet::new(),
            firmas: std::collections::HashMap::new(),
            func_addr_fixups: Vec::new(),
            break_target: Vec::new(),
            continue_target: Vec::new(), var_offsets: HashMap::new(),
            frame_size: 0,
            es_variadica: false,
            ranuras_con_nombre: 0,
            struct_layouts: HashMap::new(), struct_sizes: HashMap::new(),
            label_positions: HashMap::new(), goto_relocs: Vec::new(),
            entry_offset: 0, is_entry_function: false,
            global_offsets: HashMap::new(), global_data: Vec::new(),
            global_fixups: Vec::new(),
            relocs_a_cadena: Vec::new(),
            quiere_pantalla: false,
            relocs: Vec::new(),
            instruction_end: 0, string_data_end: 0,
            stdlib_imports: std::collections::HashSet::new(),
            enum_values: HashMap::new(),
            isa: bmo_sem_asm::Instructions::load_x86_64()
                .expect("tablas sem-asm x86-64 (forge/sem-asm/tables)"),
            intrinsics: bmo_sem_asm::Intrinsics::load_x86_64()
                .expect("tabla de intrínsecos x86-64 (forge/sem-asm/tables)"),
            errors: Vec::new(),
        }
    }

    fn fresh_label(&mut self) -> u32 {
        let l = self.labels;
        self.labels += 1;
        l
    }

    // ---- Program ----
    fn emit_program(&mut self, program: &Program) -> Result<()> {
        // build struct/union layouts
        for decl in &program.globals {
            match decl {
                GlobalDecl::Struct(name, members) => {
                    self.build_struct_layout(name, members);
                }
                GlobalDecl::Union(name, members) => {
                    self.build_union_layout(name, members);
                }
                _ => {}
            }
        }
        // allocate space for global variables
        for decl in &program.globals {
            // * Un global con LISTA: `int t[4] = {1,2,3,4}`.
            //
            // Las escrituras llegan aplanadas del parser (offset absoluto,
            // tipo del subobjeto, valor), asi que aqui solo hay que evaluar
            // cada valor **en tiempo de compilacion** y ponerlo en su sitio.
            // El objeto entero se reserva a cero primero, que es lo que dice C
            // de lo que la lista no menciona -- y por eso `{[2] = 30}` deja los
            // huecos a cero sin que nadie los escriba.
            if let GlobalDecl::VarLista(typ, name, escrituras) = decl {
                // `self.type_stack_size` y NO `typ.stack_size()`: ver el motivo
                // en el `Var` de abajo. El metodo del `TypeSpec` devuelve CERO
                // para un `StructRef`, asi que una tabla de structs habria
                // reservado cero bytes.
                let size = self.type_stack_size(typ) as usize;
                let pad = (8 - self.global_data.len() % 8) % 8;
                for _ in 0..pad { self.global_data.push(0); }
                let off = self.global_data.len() as u32;
                for _ in 0..size { self.global_data.push(0); }
                for e in escrituras {
                    // * UNA CADENA en la tabla -> RELOCATION.
                    //
                    // `char *nombres[] = {"imp", "cyberdemon"}` y, en DOOM,
                    // `char *sprnames[]`. Cada elemento es un puntero y cada
                    // puntero es una reloc: el hueco queda a cero y el cargador
                    // escribe la direccion.
                    if let Expr::StringLit(s) = &e.valor {
                        let idx = match self.strings.iter().position(|t| t == s) {
                            Some(i) => i,
                            None => {
                                self.strings.push(s.clone());
                                self.strings.len() - 1
                            }
                        };
                        self.relocs_a_cadena.push((off + e.offset, idx));
                        continue;
                    }
                    let Some(valor) = Self::constante_de(&e.valor) else {
                        // Lo que sigue sin poderse poner. El caso vivo es una
                        // tabla de punteros a FUNCION (el campo `action` de cada
                        // `state_t` de DOOM): hace falta la misma relocation
                        // pero con destino en `.code`, y el codegen todavia no
                        // sabe el offset de una funcion en este punto -- se
                        // registran mas abajo, al emitirlas.
                        self.errors.push(format!(
                            "en la tabla global '{name}', el valor del offset {} no es una \
                             constante entera ni una cadena. Si es la direccion de una funcion, \
                             eso necesita una relocation a `.code` y todavia no esta: rellena esa \
                             posicion dentro de una funcion",
                            e.offset
                        ));
                        continue;
                    };
                    let ancho = e.tipo.stack_size() as usize;
                    let destino = off as usize + e.offset as usize;
                    let bytes = valor.to_le_bytes();
                    // Se recorta al ancho del subobjeto: un `char` de la tabla
                    // toma un byte, no ocho. Y se comprueba el limite en vez de
                    // confiar: un offset fuera del objeto seria escribir sobre
                    // el global de al lado.
                    for i in 0..ancho.min(8) {
                        if destino + i < self.global_data.len() {
                            self.global_data[destino + i] = bytes[i];
                        }
                    }
                }
                self.global_offsets.insert(name.clone(), (off, typ.clone()));
                continue;
            }
            if let GlobalDecl::Var(typ, name, init) = decl {
                // * `self.type_stack_size` y NO `typ.stack_size()`, y la
                // diferencia era un bug: el metodo del `TypeSpec` no conoce los
                // layouts de struct y devuelve **CERO** para un `StructRef`
                // (`ast/types.rs`), mientras el del `Codegen` los consulta en
                // `struct_sizes`.
                //
                // O sea que un `struct P g;` global reservaba **cero bytes**, y
                // el global declarado justo despues caia ENCIMA. La sonda:
                //
                //     struct P { int x; int y; };  struct P g;
                //     int centinela = 12345;
                //     g.x = 7;   ->  centinela pasaba a valer 7
                //
                // Compilaba, ejecutaba y daba un numero plausible. Los tres
                // tests que habia en `globales.rs` no lo veian porque solo
                // comprobaban que el programa COMPILARA.
                //
                // Aqui se puede usar el del `Codegen` porque los layouts se
                // calculan en el bucle de arriba, antes que este.
                let size = self.type_stack_size(typ) as u32;
                let pad = (8 - self.global_data.len() as u32 % 8) % 8;
                for _ in 0..pad { self.global_data.push(0); }
                let off = self.global_data.len() as u32;
                // * `None` y `Some(otra_cosa)` NO son lo mismo, y hasta hoy se
                // trataban igual: ceros.
                //
                // Un global sin inicializador vale cero, y eso es correcto en C.
                // Pero un global CON inicializador que este codegen no sabe
                // convertir tambien se rellenaba de ceros **sin decir nada**, y
                // eso hacia que
                //
                //     char *texto = "eltexto";
                //     printf("%s", texto);
                //
                // compilara e imprimiera los bytes de la seccion de codigo: el
                // puntero valia 0, y el byte 0 de la imagen es el `push rbp` de
                // la primera funcion. Se veia `UH\x89a...` y ningun test lo
                // miraba, porque `globales.rs` solo comprobaba que compilara.
                //
                // Ahora se dice. Un cero inventado es la peor respuesta a "no se
                // hacer esto": es un valor legitimo, asi que el error viaja
                // hasta donde ya no se puede rastrear.
                let literal = init.as_ref().and_then(Self::constante_de);
                match (init, literal) {
                    (_, Some(n)) => {
                        let bytes: Vec<u8> = match size {
                            1 => vec![n as u8],
                            2 => (n as u16).to_le_bytes().to_vec(),
                            4 => (n as u32).to_le_bytes().to_vec(),
                            _ => (n as u64).to_le_bytes().to_vec(),
                        };
                        self.global_data.extend_from_slice(&bytes);
                    }
                    // Sin inicializador: cero, que es lo que dice C.
                    (None, _) => {
                        for _ in 0..size { self.global_data.push(0); }
                    }
                    // * UNA CADENA: ya no es un error, es una RELOCATION.
                    //
                    // `char *mapa = "1111..."` tiene que guardar la DIRECCION de
                    // la cadena, y esa depende de donde cargue el programa. El
                    // compilador deja el hueco a cero y anota quien lo rellena;
                    // lo escribe el cargador, que es el unico que sabe la VA.
                    //
                    // Esto es lo que hacia falta para que el mapa del raycaster
                    // pudiera vivir donde estaba escrito.
                    (Some(Expr::StringLit(s)), _) => {
                        let idx = match self.strings.iter().position(|t| t == s) {
                            Some(i) => i,
                            None => {
                                // `collect_strings` solo recorre FUNCIONES, asi
                                // que una cadena que aparece unicamente en un
                                // global no entraria nunca en la tabla. Se
                                // interna aqui; el dedup es por valor, asi que
                                // si ademas sale en una funcion, es la misma.
                                self.strings.push(s.clone());
                                self.strings.len() - 1
                            }
                        };
                        self.relocs_a_cadena.push((off, idx));
                        for _ in 0..size { self.global_data.push(0); }
                    }
                    // Lo que sigue sin poderse poner: se DICE.
                    (Some(otro), _) => {
                        let que = match otro {
                            Expr::FloatLit(_) => "un literal de coma flotante, que aun no se convierte",
                            _ => "una expresion que este compilador no evalua en tiempo de \
                                  compilacion (aqui caben constantes enteras y cadenas)",
                        };
                        self.errors.push(format!(
                            "el global '{name}' se inicializa con {que}. Antes esto se rellenaba \
                             de CEROS en silencio y el programa arrancaba con un valor que nadie \
                             escribio"
                        ));
                        for _ in 0..size { self.global_data.push(0); }
                    }
                }
                self.global_offsets.insert(name.clone(), (off, typ.clone()));
            }
        }
        self.collect_strings(program);
        // registrar todos los nombres de funcion ANTES de emitir: una llamada
        // puede referir a una funcion definida mas abajo (forward reference).
        for func in &program.functions {
            self.known_functions.insert(func.name.clone());
            self.firmas.insert(
                func.name.clone(),
                (
                    func.params.iter().map(|p| p.typ.clone()).collect(),
                    func.ret_type.clone(),
                ),
            );
        }
        // emit all functions, tracking entry point
        for func in &program.functions {
            let off = self.code.len();
            self.function_offsets.insert(func.name.clone(), off);
            if func.name == "main" { self.entry_offset = off; }
            self.is_entry_function = func.name == "main";
            self.emit_function(func);
        }
        self.is_entry_function = false;
        // * Sin `main` no hay programa.
        //
        // Antes, un fichero vacio --o uno con funciones pero sin punto de
        // entrada-- producia un BEF de 8 240 bytes con `entry_offset = 0`, o
        // sea apuntando a lo primero que hubiera en la seccion de codigo. Se
        // escribia sin quejarse. Un binario con un punto de entrada inventado
        // es peor que no tener binario: falla en el metal y no en la
        // compilacion, que es donde se puede leer el motivo.
        //
        // Ring 0 se exceptua porque un modulo de kernel puede no tener `main`
        // -- hoy nadie construye ese perfil, pero la puerta se deja abierta
        // con su motivo en vez de cerrada por accidente.
        if self.target == TargetProfile::Ring3App
            && !program.functions.iter().any(|f| f.name == "main")
        {
            self.errors.push(
                "no hay funcion 'main': un programa de Ring 3 necesita punto de entrada"
                    .to_string(),
            );
        }
        // * Las funciones SINTETIZADAS. Ver `SINTETIZABLES` para el por que.
        //
        // Aqui estaba cableado el `syscall; ret` del stub, emitido SIEMPRE que
        // el perfil fuera Ring 3. Ya no hace falta el `if`: el stub se inyecta
        // porque alguien lo llama, no porque el perfil lo sugiera --Ring 0 hace
        // el `syscall` en linea y por eso nunca crea la reloc--, y de paso un
        // programa de Ring 3 que no hace ni una syscall deja de llevar tres
        // bytes muertos al final del codigo.
        self.sintetizar_referidas();
        // Saltos hacia atras (bucles): se resuelven aqui, cuando ya se
        // conocen todas las etiquetas.
        self.patch_backward_relocs();
        // patch all call relocs
        self.patch_call_relocs();
        self.patch_func_addr_fixups();
        self.patch_goto_relocs();
        self.patch_all_fixups();
        // Errores acumulados durante la emision: fallar con claridad, no
        // entregar un binario que hace algo distinto de lo escrito.
        if let Some(message) = self.errors.first() {
            return Err(CError::new(0, message.clone()));
        }
        Ok(())
    }

    /// Segunda de las tres copias que habia de la regla de disposicion. Ahora
    /// las tres llaman a `bmo_abi::types::disposicion`, que es donde esta
    /// escrita -- y con sus tests.
    ///
    /// Que el codegen la recalcule en vez de recibirla del parser **no es
    /// duplicacion**: es lo que hace que un frontend distinto (C++) que ya
    /// calculo offsets para sus nodos `Field` no pueda imponer una
    /// disposicion propia sin que se note.
    fn build_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz), sz));
        }
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
    }

    fn build_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::DisposicionUnion::nueva();
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz), sz));
        }
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
    }

    fn type_stack_size(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong | TypeSpec::UnsignedLongLong => 8,
            TypeSpec::Float => 4,
            TypeSpec::Double => 8,
            TypeSpec::Ptr(_) => 8,
            TypeSpec::Array(t, n) => self.type_stack_size(t) * n,
            TypeSpec::StructRef(name) | TypeSpec::UnionRef(name) => {
                self.struct_sizes.get(name).copied().unwrap_or(8)
            }
        }
    }

    fn collect_strings(&mut self, program: &Program) {
        for func in &program.functions {
            for stmt in &func.body { self.collect_stmt_strings(stmt); }
        }
    }

    fn collect_stmt_strings(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Printf(s) | Stmt::PrintfLn(s) => {
                if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
            }
            // * LAS CONDICIONES TAMBIEN. Estaban descartadas --el `_` de cada
            // una-- asi que un literal dentro de una condicion nunca entraba en
            // la tabla, y al emitirlo el `unwrap_or(0)` lo hacia apuntar **a la
            // primera cadena del programa**.
            //
            // Llevaba ahi desde siempre y no se veia porque hasta hoy no habia
            // forma de poner un literal en una condicion: hacia falta algo como
            // `if (strcmp(s, "salir") == 0)`, y `strcmp` no existia. El primer
            // test que lo piso decia `menor` y no imprimia nada -- comparando
            // "abc" contra el formato de un `printf` anterior.
            //
            // Un `unwrap_or(0)` sobre una tabla de direcciones es exactamente
            // la clase de fallo silencioso que este compilador no cuenta: no
            // falla, apunta a otro sitio.
            Stmt::If(c, t, e) => {
                self.collect_expr_strings(c);
                self.collect_stmt_strings(t);
                if let Some(el) = e { self.collect_stmt_strings(el); }
            }
            Stmt::While(c, b) => { self.collect_expr_strings(c); self.collect_stmt_strings(b); }
            Stmt::DoWhile(b, c) => { self.collect_stmt_strings(b); self.collect_expr_strings(c); }
            Stmt::For(ini, cond, paso, b) => {
                if let Some(e) = ini { self.collect_expr_strings(e); }
                if let Some(e) = cond { self.collect_expr_strings(e); }
                if let Some(e) = paso { self.collect_expr_strings(e); }
                self.collect_stmt_strings(b);
            }
            Stmt::Switch(c, cases) => {
                self.collect_expr_strings(c);
                for c in cases { for s in &c.stmts { self.collect_stmt_strings(s); } }
            }
            Stmt::Block(stmts) => { for s in stmts { self.collect_stmt_strings(s); } }
            Stmt::Expr(e) | Stmt::Return(Some(e)) => { self.collect_expr_strings(e); }
            Stmt::DeclAssign(_, _, Some(e)) => { self.collect_expr_strings(e); }
            // Sin esto, un `%s` dentro de una lista de inicializacion
            // apuntaria a una cadena que nunca se puso en .rodata.
            Stmt::DeclInit(_, _, es) => { for e in es { self.collect_expr_strings(&e.valor); } }
            _ => {}
        }
    }

    fn collect_expr_strings(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLit(s) => {
                if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => self.collect_expr_strings(a),
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => { self.collect_expr_strings(a); self.collect_expr_strings(b); }
            Expr::Conditional(c,t,f) => { self.collect_expr_strings(c); self.collect_expr_strings(t); self.collect_expr_strings(f); }
            Expr::Call(name, args) => {
                // * AQUI SE DEDUCE `WANTS_SCREEN`, y tiene que ser aqui.
                //
                // La tentacion es mirarlo en `Expr::Syscall`, donde esta el
                // INVOKE. No sirve: `bmo_valor` es una FUNCION C de verdad
                // --vive en `<bmo/bmo.h>`-- asi que dentro de ella la operacion es
                // un PARAMETRO, no un literal. En el sitio de la llamada si se
                // ve el numero.
                //
                // `0x09` es `BMO_OP_PANTALLA_RECLAMAR`. Se pide tambien que el
                // callee sea una de las dos puertas: un `0x09` suelto como
                // segundo argumento de cualquier funcion no significa nada.
                if (name == "bmo_valor" || name == "bmo_codigo") && args.len() >= 2 {
                    if let Expr::Int(0x09) = args[1] {
                        self.quiere_pantalla = true;
                    }
                }
                for a in args { self.collect_expr_strings(a); }
            }
            Expr::Syscall(_, args) => { for a in args { self.collect_expr_strings(a); } }
            Expr::Arrow(p,_,_,_) => self.collect_expr_strings(p),
            Expr::AssignArrow(p,_,_,_,v) => { self.collect_expr_strings(p); self.collect_expr_strings(v); }
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => self.collect_expr_strings(v),
            Expr::Cast(_, a) => self.collect_expr_strings(a),
            Expr::Intrinsic(_, args) => { for a in args { self.collect_expr_strings(a); } }
            Expr::IndexPtr(b, idx, _) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); }
            Expr::AssignIndexPtr(b, idx, _, v) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); self.collect_expr_strings(v); }
            Expr::CallPtr(c, args) => { self.collect_expr_strings(c); for a in args { self.collect_expr_strings(a); } }
            Expr::AssignDeref(a, v) => { self.collect_expr_strings(a); self.collect_expr_strings(v); }
            Expr::Field(b,_,_,_) => self.collect_expr_strings(b),
            Expr::Comma(v) => { for e in v { self.collect_expr_strings(e); } }
            _ => {}
        }
    }

    // Aqui vivia `pad_to_page`, que rellenaba cada tramo con `int3` hasta la
    // siguiente frontera de pagina. Se borra con el relleno: ya no hay a quien
    // rellenar. La intencion buena que tenia --que un CPU que se salga del
    // codigo pare en seco en vez de deslizarse por ceros-- no se pierde: el
    // cargador pone a cero los marcos y el resto de la pagina tras el codigo no
    // esta mapeado como ejecutable por nadie. Y el arnes de pruebas rellena los
    // huecos entre secciones con `0xCC` por esa misma razon.

    /// Coloca las cadenas y los globales, y parchea los `lea [rip+disp]`
    /// que los alcanzan.
    ///
    /// # Por que hay relleno a pagina
    ///
    /// Estos desplazamientos se calculan asumiendo que los datos van
    /// PEGADOS detras del codigo. Pero el cargador del kernel
    /// (`ring0/proc.rs`) coloca cada seccion en la PAGINA siguiente:
    /// `va_cursor = va_start + pages * PAGE`. Con el codigo a 500 bytes, el
    /// compilador apunta al byte 500 y el cargador pone la cadena en el
    /// 4096 -- un `%s` leeria basura EN HARDWARE.
    ///
    /// Rellenando cada tramo hasta una pagina, las dos cuentas coinciden.
    /// La solucion definitiva son relocations en el BEF; esto es el acuerdo
    /// correcto mientras no existan, y no depende de que el cargador cambie.
    ///
    /// NOTA: esto NO lo puede detectar el emulador de pruebas, porque alli
    /// las secciones se concatenan tal cual. Es un fallo que solo aparece en
    /// metal -- la razon por la que un banco de pruebas localiza bugs pero no
    /// sustituye a arrancar la maquina.
    /// El valor de una expresion **en tiempo de compilacion**, o `None`.
    ///
    /// Es lo unico que puede ir dentro de un dato inicializado: el `.bex` se
    /// escribe con los bytes ya puestos, asi que aqui no hay donde ejecutar
    /// nada. Un `None` no es un cero -- quien llama tiene que decirlo.
    ///
    /// # Que entra, y por que esto y no un evaluador entero
    ///
    /// Enteros, su negacion, y las operaciones que aparecen de verdad en una
    /// tabla escrita a mano: `{1, -2, 3*4, MAX-1}`. Las constantes de `enum`
    /// **no** se resuelven aqui porque `enum_values` es estado del `Codegen` y
    /// esto es una funcion asociada; es el siguiente paso obvio y esta dicho
    /// para que no parezca un olvido.
    ///
    /// No se plegaron divisiones por cero ni desbordamientos con `wrapping`:
    /// una tabla con `1/0` dentro es un error del programa, y contestar algo
    /// seria inventarlo. Con `checked_div` devolvemos `None` y el llamante
    /// dice que ese valor no es constante -- un mensaje impreciso, pero no una
    /// mentira.
    fn constante_de(e: &Expr) -> Option<i64> {
        match e {
            Expr::Int(n) => Some(*n),
            // `int x = -5` es `Neg(Int(5))` en el AST, no `Int(-5)`.
            Expr::Neg(interior) => Self::constante_de(interior).map(|v| -v),
            Expr::Add(a, b) => {
                Some(Self::constante_de(a)?.wrapping_add(Self::constante_de(b)?))
            }
            Expr::Sub(a, b) => {
                Some(Self::constante_de(a)?.wrapping_sub(Self::constante_de(b)?))
            }
            Expr::Mul(a, b) => {
                Some(Self::constante_de(a)?.wrapping_mul(Self::constante_de(b)?))
            }
            Expr::Div(a, b) => Self::constante_de(a)?.checked_div(Self::constante_de(b)?),
            _ => None,
        }
    }

    /// Redondea hacia arriba al multiplo de pagina. La cuenta del cargador.
    fn hasta_pagina(n: usize) -> usize {
        const PAGE: usize = 4096;
        (n + PAGE - 1) & !(PAGE - 1)
    }

    fn patch_all_fixups(&mut self) {
        // * EL BUFER VA APRETADO Y LOS DESPLAZAMIENTOS SE CALCULAN CON LA REGLA
        // DEL CARGADOR. Antes se rellenaba cada tramo hasta la pagina, y ese
        // relleno viajaba DENTRO DEL FICHERO.
        //
        // El problema que resolvia era real: estos `lea [rip+disp]` se contaban
        // asumiendo que los datos van PEGADOS detras del codigo, y el cargador
        // (`ring0/task/proc.rs`) hace `va_cursor = va_start + pages * PAGE`, o
        // sea que pone cada seccion en la pagina siguiente. Con el codigo a 500
        // bytes, el compilador apuntaba al byte 500 y el cargador dejaba la
        // cadena en el 4096: un `%s` leia basura EN HARDWARE.
        //
        // Rellenar hacia coincidir las dos cuentas. Pero **es la cuenta lo que
        // habia que arreglar, no el tamano del fichero**: ahora el compilador
        // modela la regla del cargador --tres sumas-- y no necesita empujar 2 642
        // bytes de `0xCC` por seccion para que el mundo cuadre.
        //
        // Lo que esto quita, MEDIDO:
        //
        //   - los seis ejemplos, de 107 184 a 84 952 bytes (-20,7%), y todo
        //     ahorro de codigo futuro deja de ser invisible bajo el relleno.
        //     `holac.bex`: 12 376 -> 8 432
        //   - el tercer `pad_to_page`, que rellenaba la seccion `data` -- la
        //     ultima, sin nada detras. Relleno por relleno.
        //
        // Lo que NO quita, y conviene tenerlo escrito con su numero porque es
        // el siguiente escalon: **el BEF sigue alineando los `file_offset` a
        // 4096**. En `holac.bex` eso son 3 952 bytes de hueco antes del codigo
        // y 2 642 antes de rodata -- o sea que **6 594 de sus 8 432 bytes son
        // agujeros**. El campo `alignment` de una seccion se usa para las dos
        // cosas a la vez, y solo la direccion VIRTUAL lo necesita: el cargador
        // copia desde `file_offset` con un `copy_nonoverlapping` al que le da
        // igual donde empiece.
        //
        // Lo que esto NO quita, y hay que decirlo: **el acoplamiento sigue
        // ahi**. El compilador conoce la regla de colocacion del cargador. La
        // solucion definitiva son relocations de verdad en el BEF, para que el
        // cargador parchee y el compilador no tenga que adivinar donde va a
        // caer nada. Esto es la mitad del camino: quita el coste, deja la deuda
        // -- y ahora el emulador SI distingue las dos cuentas, asi que la otra
        // mitad se puede escribir con red.
        let code_len = self.code.len();
        self.instruction_end = code_len;

        // Las direcciones virtuales de cada seccion, con la cuenta del
        // cargador: cada una arranca en la pagina siguiente a las que ocupa la
        // anterior. Relativas al inicio del codigo, que es lo que necesita un
        // `lea [rip+disp]`.
        let rodata_len: usize = self.strings.iter().map(|s| s.len() + 1).sum();
        let va_rodata = Self::hasta_pagina(code_len);
        let va_data = va_rodata + Self::hasta_pagina(rodata_len);

        // rodata: las cadenas. `off_en_seccion` es el offset DENTRO de rodata,
        // no dentro del bufer -- que es la distincion que este cambio introduce.
        let mut off_en_seccion = 0usize;
        for (idx, s) in self.strings.iter().enumerate() {
            for f in &self.fixups {
                if f.string_idx == idx {
                    let rip = f.lea_offset + 4;
                    let disp = (va_rodata + off_en_seccion) as i64 - rip as i64;
                    self.code[f.lea_offset..f.lea_offset + 4]
                        .copy_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            self.code.extend_from_slice(s.as_bytes());
            self.code.push(0);
            off_en_seccion += s.len() + 1;
        }
        self.string_data_end = self.code.len();

        // data: los globales.
        for &(lea_offset, ref name) in &self.global_fixups {
            if let Some(&(data_off, _)) = self.global_offsets.get(name) {
                let rip = lea_offset + 4;
                let disp = (va_data as i64 + data_off as i64) - rip as i64;
                self.code[lea_offset..lea_offset + 4]
                    .copy_from_slice(&(disp as i32).to_le_bytes());
            }
        }
        let globals = core::mem::take(&mut self.global_data);
        self.code.extend_from_slice(&globals);
        self.global_data = globals;

        // * LAS RELOCATIONS, que es lo unico que el compilador NO puede
        // resolver por su cuenta.
        //
        // Todo lo de arriba son desplazamientos: la distancia entre dos
        // secciones de la misma imagen es fija, asi que un `lea [rip+disp]` se
        // puede calcular aqui. Un PUNTERO GUARDADO EN UN DATO es otra cosa --
        // lleva la direccion absoluta, y esa depende de donde cargue el
        // programa. Se anota y la escribe el cargador.
        //
        // Los offsets van dentro de su seccion, no del bufer: el del puntero es
        // relativo a `.data` (ya lo es, sale de `global_data`) y el de la cadena
        // relativo a `.rodata`.
        let mut off_cadena: Vec<usize> = Vec::with_capacity(self.strings.len());
        let mut acc = 0usize;
        for s in &self.strings {
            off_cadena.push(acc);
            acc += s.len() + 1;
        }
        for &(off_en_data, idx) in &self.relocs_a_cadena {
            let Some(&destino) = off_cadena.get(idx) else {
                self.errors.push(format!(
                    "reloc a una cadena que no esta en la tabla (indice {idx}): esto es un bug \
                     del compilador, no del programa"
                ));
                continue;
            };
            self.relocs.push(Relocation::seccion_abs64(
                SEC_DATA,
                off_en_data as u64,
                SEC_RODATA,
                destino as i64,
            ));
        }
    }

    fn patch_goto_relocs(&mut self) {
        for (off, label) in &self.goto_relocs {
            if let Some(&target) = self.label_positions.get(label) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            }
        }
    }

    /// `call rel32` a una funcion del catalogo de [`sintetizadas`], con su
    /// reloc pendiente.
    ///
    /// El nombre no se comprueba contra el catalogo aqui a proposito: si
    /// alguien se equivoca escribiendolo, [`Self::patch_call_relocs`] falla
    /// diciendo *"no existe la funcion 'X'"* con el nombre delante, que es un
    /// mejor error que un `panic` del compilador -- y ese camino ya esta probado
    /// (`una_funcion_desconocida_sigue_fallando_con_su_nombre`).
    fn emit_call_sintetizada(&mut self, name: &str) {
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc {
            offset: self.code.len(),
            target: name.to_string(),
        });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// La PASADA sobre las relocs pendientes: pide a [`sintetizadas`] el cuerpo
    /// de lo que alguien llama y no esta definido, y registra su offset.
    ///
    /// El catalogo y los cuerpos NO estan aqui, y el corte es deliberado: este
    /// fichero sabe que es una reloc y que es el `Codegen`; aquel sabe que
    /// bytes implementan `strlen`. Anadir una funcion sintetizable no toca este
    /// metodo.
    ///
    /// Va ANTES de [`Self::patch_call_relocs`] y no puede ir despues: ese es
    /// quien escribe los desplazamientos, y necesita el offset ya registrado.
    fn sintetizar_referidas(&mut self) {
        // [!] ESTE GUARDIA VALE MAS QUE UN COMENTARIO, y el motivo esta escrito
        // en la cabecera de `patch_all_fixups`: la seccion de codigo es
        // `all[..instruction_end]`, y **`rodata` es lo que viene detras**. Si
        // esta pasada se moviera despues de `patch_all_fixups`, los cuerpos
        // sintetizados caerian en `rodata`, que se mapea SIN permiso de
        // ejecucion -- y el `.bex` saltaria EN METAL.
        //
        // El banco de pruebas NO puede cazarlo: el emulador reconcatena las
        // secciones tal cual, asi que ejecutaria el cuerpo igual y los 262
        // tests seguirian verdes. O sea, exactamente la clase de fallo que solo
        // aparece con la maquina delante. Por eso se comprueba aqui.
        debug_assert_eq!(
            self.instruction_end, 0,
            "sintetizar_referidas() tiene que ir ANTES de patch_all_fixups():              si no, el cuerpo sintetizado acaba en rodata y no se puede ejecutar"
        );
        sintetizadas::inyectar(&mut self.code, &self.call_relocs, &mut self.function_offsets);
    }

    /// Escribe el destino de cada `call rel32`.
    ///
    /// * Una llamada sin destino es un ERROR, no un hueco.
    ///
    /// Antes el `if let` no tenia `else`: el desplazamiento se quedaba en 0, y
    /// `E8 00000000` es "llama a la instruccion siguiente" -- o sea, un `call`
    /// que empuja una direccion de retorno, no hace nada y vuelve. Un nombre mal
    /// escrito, o una macro con parametros que este preprocesador todavia no
    /// expande, producia un programa que compilaba y **se saltaba la llamada en
    /// silencio**.
    ///
    /// Aqui no hay enlazado que pueda rellenarlo mas tarde: no existe tabla de
    /// importaciones en la salida de este codegen, asi que todo lo que se llama
    /// tiene que estar en esta misma unidad --o en el catalogo de
    /// [`sintetizadas`], que es lo que la pasada de arriba acaba de inyectar--.
    /// La prueba de que era un descuido y no una decision esta tres funciones
    /// mas abajo: `patch_func_addr_fixups` ya reportaba exactamente este caso
    /// para los punteros a funcion.
    fn patch_call_relocs(&mut self) {
        let mut faltan: Vec<String> = Vec::new();
        for reloc in &self.call_relocs {
            if let Some(&target_offset) = self.function_offsets.get(&reloc.target) {
                let off = reloc.offset;
                let disp = target_offset as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
            } else if !faltan.contains(&reloc.target) {
                faltan.push(reloc.target.clone());
            }
        }
        for name in faltan {
            self.errors.push(format!(
                "no existe la funcion '{name}' que se llama (aqui no hay enlazado: \
                 todo lo que se llama tiene que estar en esta unidad)"
            ));
        }
    }

    /// Escribe la direccion rip-relativa de cada funcion referida por un
    /// `lea rax, [rip+func]` (punteros a funcion). Mismo esquema que las
    /// call relocs: displacement dentro de la seccion de codigo.
    fn patch_func_addr_fixups(&mut self) {
        for (off, name) in &self.func_addr_fixups {
            if let Some(&target) = self.function_offsets.get(name) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            } else {
                self.errors.push(format!("no existe la funcion '{name}' cuya direccion se tomo"));
            }
        }
    }

    /// `lea rax, [rip+func]` -- deja en rax la direccion de una funcion.
    fn emit_func_addr(&mut self, name: &str) {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
        self.func_addr_fixups.push((self.code.len() - 4, name.to_string()));
    }

    /// Fija una etiqueta en la posicion actual y resuelve los saltos que ya
    /// la esperaban.
    ///
    /// El `label_offsets` es lo que faltaba: antes esta funcion SOLO
    /// parcheaba los saltos pendientes en ese instante, asi que un salto
    /// emitido DESPUES de fijar la etiqueta --es decir, todo salto hacia
    /// atras-- se quedaba con desplazamiento 0 para siempre. Eso significa
    /// "seguir a la instruccion siguiente": **ningun bucle de C daba mas de
    /// una vuelta**. `while`, `for`, `do-while`, y por tanto `break` y
    /// `continue`, ejecutaban el cuerpo exactamente una vez y salian. El
    /// binario compilaba y validaba igual.
    ///
    /// Es el mismo defecto que tenia el `IF` de COBOL, en otro lenguaje.
    fn resolve_label(&mut self, label: u32) {
        let here = self.code.len();
        self.label_offsets.insert(label, here);
        let mut i = 0;
        while i < self.pending_relocs.len() {
            if self.pending_relocs[i].target_label == label {
                let off = self.pending_relocs[i].offset;
                let disp = here as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                self.pending_relocs.swap_remove(i);
            } else { i += 1; }
        }
    }

    /// Resuelve los saltos que quedaron pendientes: los que apuntan a una
    /// etiqueta fijada ANTES de emitirlos (saltos hacia atras).
    ///
    /// Una etiqueta usada y jamas fijada es un bug del emisor: se aborta en
    /// vez de dejar un salto a ninguna parte.
    fn patch_backward_relocs(&mut self) {
        for reloc in std::mem::take(&mut self.pending_relocs) {
            let target = *self
                .label_offsets
                .get(&reloc.target_label)
                .unwrap_or_else(|| panic!("etiqueta {} usada pero nunca fijada", reloc.target_label));
            let disp = target as i32 - (reloc.offset as i32 + 4);
            self.code[reloc.offset..reloc.offset + 4].copy_from_slice(&disp.to_le_bytes());
        }
    }

    fn emit_jmp_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0xE9]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn emit_jz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x84]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn emit_jnz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x85]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    // ---- Stack frame helpers ----

    /// Recolecta TODAS las DeclAssign del cuerpo, a cualquier profundidad.
    /// Antes solo se miraba el nivel superior: una `int i` dentro de un
    /// for/if/bloque NO recibia slot -- stores descartados, loads = 0.
    fn collect_decls_stmt<'a>(s: &'a Stmt, out: &mut Vec<(&'a String, &'a TypeSpec)>) {
        match s {
            Stmt::DeclAssign(t, n, _) => out.push((n, t)),
            // El hueco en la pila. Sin esta linea la variable caia al
            // reparto de legado (8 bytes, tipo Long) y un struct de 16
            // habria escrito sobre la de al lado.
            Stmt::DeclInit(t, n, _) => out.push((n, t)),
            Stmt::Block(v) => for x in v { Self::collect_decls_stmt(x, out); },
            Stmt::If(_, a, b) => {
                Self::collect_decls_stmt(a, out);
                if let Some(b) = b { Self::collect_decls_stmt(b, out); }
            }
            Stmt::While(_, b) | Stmt::DoWhile(b, _) | Stmt::For(_, _, _, b) => Self::collect_decls_stmt(b, out),
            Stmt::Switch(_, cases) => for c in cases { for st in &c.stmts { Self::collect_decls_stmt(st, out); } },
            _ => {}
        }
    }

    fn build_var_map(&mut self, params: &[Param], var_names: &[String], func: &Function) {
        self.var_offsets.clear();
        // -- Los parametros, en la pila del llamante --
        //
        // Empiezan en `[rbp+16]` (detras de la direccion de retorno y del `rbp`
        // guardado) y avanzan por RANURAS, no de ocho en ocho: un agregado de
        // 12 bytes ocupa dos y corre el que viene detras.
        //
        // Era `16 + i*8` fijo. Mientras todo cupo en un registro daba lo mismo;
        // el dia que entro un struct por valor, el segundo parametro empezaba a
        // leerse desde la mitad del primero.
        let mut off = 16i32;
        for p in params.iter() {
            self.var_offsets.insert(p.name.clone(), (off, p.typ.clone()));
            let bytes = self.type_stack_size(&p.typ);
            off += agregados::ranuras(bytes) as i32 * 8;
        }
        // locales: tamano REAL del tipo (arrays y structs incluidos), alineado a 8
        let mut decls = Vec::new();
        for stmt in &func.body { Self::collect_decls_stmt(stmt, &mut decls); }
        let mut cur: i32 = 0;
        for (name, typ) in &decls {
            if self.var_offsets.contains_key(*name) { continue; } // sombra: un solo slot
            let sz = self.type_stack_size(typ).max(8);
            let sz = ((sz + 7) / 8 * 8) as i32;
            cur -= sz;
            self.var_offsets.insert((*name).clone(), (cur, (*typ).clone()));
        }
        // legado: nombres registrados por el parser sin DeclAssign visible
        for name in var_names.iter().skip(params.len()) {
            if !self.var_offsets.contains_key(name) {
                cur -= 8;
                self.var_offsets.insert(name.clone(), (cur, TypeSpec::Long));
            }
        }
        self.frame_size = -cur;
    }

    /// Guarda `rax` en `[rbp+disp]` con el tamano EXACTO de `tipo`.
    ///
    /// La pareja de `emit_store_var`, pero por offset en vez de por nombre: una
    /// lista de inicializacion escribe **dentro** de una variable, no sobre
    /// ella. Escribir siempre 8 bytes pisaria el campo siguiente -- es el mismo
    /// bug que ya se pago con `pt.x = 10` cuando `x` era `int`.
    fn emit_store_rbp(&mut self, disp: i32, tipo: &TypeSpec) {
        let corto = (-128..=127).contains(&disp);
        let modrm = if corto { 0x45 } else { 0x85 };
        let opcode: &[u8] = match tipo {
            TypeSpec::Char | TypeSpec::UnsignedChar => &[0x88],
            TypeSpec::Short | TypeSpec::UnsignedShort => &[0x66, 0x89],
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Float => &[0x89],
            _ => &[0x48, 0x89],
        };
        self.code.extend_from_slice(opcode);
        self.code.push(modrm);
        if corto {
            self.code.push(disp as u8);
        } else {
            self.code.extend_from_slice(&disp.to_le_bytes());
        }
    }

    /// Pone a cero `bytes` bytes a partir de `[rbp+base]`.
    ///
    /// De ocho en ocho mientras quepa, y el resto byte a byte. Sin memset:
    /// aqui no hay libc, y para los tamanos de un struct local un bucle
    /// desenrollado es mas corto que la llamada que no existe.
    fn emit_cero_local(&mut self, base: i32, bytes: u32) {
        if bytes == 0 {
            return;
        }
        self.emit_xor_eax();
        let mut hecho = 0u32;
        while bytes - hecho >= 8 {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Long);
            hecho += 8;
        }
        while hecho < bytes {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Char);
            hecho += 1;
        }
    }

    fn emit_store_var(&mut self, name: &str) {
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            let rex8 = if disp >= -128 && disp <= 127 { 0x45 } else { 0x85 };
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => {
                    self.code.extend_from_slice(&[0x88, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Short | TypeSpec::UnsignedShort => {
                    self.code.extend_from_slice(&[0x66, 0x89, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Int | TypeSpec::UnsignedInt => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
                _ => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
            }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // rax already has value; lea rdi, [rip+0]; mov [rdi], reg
            self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x88, 0x07]),
                TypeSpec::Short | TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x66, 0x89, 0x07]),
                TypeSpec::Int | TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0x07]),
                _ => self.code.extend_from_slice(&[0x48, 0x89, 0x07]),
            }
        }
    }

    fn emit_load_var(&mut self, name: &str) {
        // Enum constants: emit integer literal directly
        if let Some(&val) = self.enum_values.get(name) {
            self.code.extend_from_slice(&[0xB8]); // mov eax, imm32
            self.code.extend_from_slice(&(val as i32).to_le_bytes());
            return;
        }
        // Funcion usada como VALOR (fp = myfunc): decae a su direccion.
        if self.known_functions.contains(name)
            && !self.var_offsets.contains_key(name)
            && !self.global_offsets.contains_key(name)
        {
            self.emit_func_addr(name);
            return;
        }
        // Arrays: decaen a puntero -- "cargar" arr es su DIRECCION, no su contenido
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
            return;
        }
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            match typ {
            TypeSpec::Char => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedChar => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::Short => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedShort => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            // Un `int` con signo debe EXTENDER EL SIGNO al leerse: el resto
            // del codegen trabaja en 64 bits. Antes usaba `mov eax, [..]`,
            // que rellena de ceros, asi que un `int y = -7;` se releia como
            // 4294967289. Los tipos mas chicos ya lo hacian bien (movsx);
            // solo `int` se habia quedado sin su version con signo.
            TypeSpec::Int => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x45, disp as u8]); // movsxd
                } else {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedInt => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            _ => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
        }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // lea rax, [rip+0]; then mov with size to load value
            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]),
                TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]),
                TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
                TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
                // * `int` con SIGNO se extiende con signo, y compartia arm con
                // `unsigned int`.
                //
                // Era `mov eax,[rax]` para los dos, que rellena de CEROS los 32
                // bits altos. `char` y `short` si usaban `movsx` --asi que la
                // intencion estaba clara y el `int` se quedo fuera--, y no se
                // notaba porque **ningun global podia valer negativo**: el
                // inicializador solo entendia `Expr::Int` positivo y todo lo
                // demas se rellenaba de ceros en silencio. Al arreglar aquello,
                // `int frio = -40;` empezo a imprimir **4294967256**.
                //
                // `movsxd rax, dword [rax]` = `48 63 00`.
                TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0x00]),
                TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x8B, 0x00]),
                _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]),
            }
        } else {
            // * Un nombre que no es variable, ni global, ni constante de enum,
            // ni funcion, NO VALE CERO: no existe.
            //
            // Esto era un `xor eax,eax` mudo, y es lo que escondio que
            // `#include` tiraba los `#define` de la cabecera: `BMO_TECLA_REPAG`
            // y `BMO_TECLA_AVPAG` llegaban sin expandir, el codegen los ponia a
            // cero **a los dos**, y `if (t == REPAG)` era cierto para AvPag.
            // Comparaba cero contra cero y el programa parecia correcto.
            //
            // Un cero inventado es la peor respuesta posible a "no se que es
            // esto": es un valor legitimo en cualquier expresion, asi que el
            // error viaja hasta donde ya no se puede rastrear.
            self.errors.push(format!(
                "'{name}' no esta declarado (ni variable, ni global, ni constante de enum, \
                 ni funcion). Si venia de un #define, la cabecera no llego a expandirse."
            ));
            self.emit_xor_eax();
        }
    }

    /// Deja el handle (`rdx` de la primera llamada, hoy perdido) y la base
    /// (`rax`) en las globales que `<bmo/archivo.h>` declara.
    ///
    /// Se llama con **`rax` = base** y con el handle todavia recuperable: no lo
    /// esta, asi que hay que haberlo guardado antes. Ver el uso en `malloc`.
    ///
    /// Si el programa no declara esas globales, esto no emite **nada**: un
    /// programa que no lee ficheros no debe pagar por la maquinaria de los que
    /// si. Por eso se pregunta por el nombre en vez de reservarlas siempre.
    fn publicar_bloque(&mut self) {
        for (name, reg) in [("__bmo_bloque_base", 0u8), ("__bmo_bloque_cap", 1u8)] {
            if !self.global_offsets.contains_key(name) {
                continue;
            }
            // lea rdi, [rip+0]  (el fixup pone la direccion de la global)
            self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            if reg == 0 {
                self.code.extend_from_slice(&[0x48, 0x89, 0x07]); // mov [rdi], rax
            } else {
                self.code.extend_from_slice(&[0x4C, 0x89, 0x07]); // mov [rdi], r8
            }
        }
    }

    fn emit_xor_eax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
    }

    fn emit_inc_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        self.emit_load_var(name);
        self.code.extend_from_slice(&[0x48, 0x83, 0xC0, 0x01]);
        self.emit_store_var(name);
    }

    fn emit_dec_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        self.emit_load_var(name);
        self.code.extend_from_slice(&[0x48, 0x83, 0xE8, 0x01]);
        self.emit_store_var(name);
    }

    // ---- Function emit ----
    fn emit_function(&mut self, func: &Function) {
        // * Un parametro de coma flotante NO se puede pasar todavia, y hasta
        // hoy se aceptaba EN SILENCIO.
        //
        // BMO C evalua floats por la ruta paralela de xmm, pero **los
        // argumentos van por la pila como enteros**: `g(1.5)` empujaba los
        // bits del double en una ranura y el prologo los leia como si fueran
        // un `long`. Compilaba, escribia un `.bef`, y devolvia basura.
        //
        // Los floats GLOBALES ya se rechazaban con motivo desde el principio
        // (ver `load_float_var`); esta puerta se quedo abierta porque nadie
        // habia escrito una funcion que tomara un `double` -- lo destapo C++ al
        // probar una sobrecarga `f(int)` / `f(double)`.
        //
        // Un cero inventado o unos bits mal leidos son la peor respuesta a "no
        // se hacer esto": son valores legitimos y el error viaja hasta donde ya
        // no se puede rastrear. Mientras la ABI de xmm no exista, se DICE.
        for p in &func.params {
            if Self::is_float_ty(&p.typ) {
                self.errors.push(format!(
                    "el parametro '{}' de '{}' es de coma flotante, y BMO C todavia no PASA \
                     floats como argumento (los evalua en xmm, pero la ABI de argumentos xmm \
                     esta pendiente). Pasa el valor por puntero, o usa un entero escalado.",
                    p.name, func.name,
                ));
            }
        }
        // El RETORNO de coma flotante si funciona --el valor queda en xmm0, y
        // hay un test que lo fija (`double_return_value_in_xmm0`)--, asi que no
        // se toca. La asimetria es real y conviene tenerla escrita: **devolver
        // un double se puede, pasarlo no.**
        self.build_var_map(&func.params, &func.var_names, func);
        // Lo que `__va_arg` necesita saber, y solo se sabe aqui: si esta
        // funcion admite variadicos y donde acaban los que tienen nombre.
        self.es_variadica = func.variadica;
        self.ranuras_con_nombre = func
            .params
            .iter()
            .map(|p| agregados::ranuras(self.type_stack_size(&p.typ)) as i32)
            .sum();
        // prologue
        self.code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]); // push rbp; mov rbp, rsp
        // Copiar los parametros a su ranura local. Hoy es un no-op --
        // `build_var_map` los deja donde ya estan-- y se conserva por si algun
        // dia un parametro necesita hueco propio. El offset se recalcula igual
        // que alli: por ranuras, no `i*8`.
        let param_count = func.params.len();
        let mut src_off = 16i32;
        for p in func.params.iter() {
            let avance = agregados::ranuras(self.type_stack_size(&p.typ)) as i32 * 8;
            // Un agregado no se copia con un `mov` de 8 bytes: ya esta en su
            // sitio, y "copiarlo" asi se llevaria solo su primera palabra.
            if self.es_agregado(&p.typ) {
                src_off += avance;
                continue;
            }
            if src_off >= -128 && src_off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x45, src_off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&(src_off as i32).to_le_bytes());
            }
            // A su ranura local, si es otra.
            if let Some(&(local_off, _)) = self.var_offsets.get(&p.name) {
                if local_off != src_off {
                    if (-128..=127).contains(&local_off) {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x45, local_off as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                        self.code.extend_from_slice(&local_off.to_le_bytes());
                    }
                }
            }
            src_off += avance;
        }
        // allocate local var space -- tamano REAL calculado por build_var_map
        // (antes: var_count*8, y los arrays/structs pisaban a sus vecinos)
        let _ = param_count;
        let stack_size = self.frame_size;
        if stack_size > 0 {
            if stack_size <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xEC, stack_size as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
                self.code.extend_from_slice(&(stack_size as u32).to_le_bytes());
            }
        }
        for stmt in &func.body { self.emit_stmt(stmt); }
        self.emit_epilogue();
    }

    fn emit_epilogue(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D]); // mov rsp,rbp; pop rbp
        if self.is_entry_function {
            // Volver de `main` termina el proceso. Antes esto emitia
            // `mov eax,0x181; syscall`: otro numero plano que el kernel no
            // despacha -- el syscall retornaba error y la ejecucion seguia
            // de largo hacia lo que hubiera despues del codigo de main.
            //
            // NOTA: el valor de retorno de `main` se descarta. `TASK_OP_EXIT`
            // no acepta codigo de salida hoy (el kernel hace revoke + reap);
            // cuando lo acepte, se pasa `rax` como argumento aqui.
            bmo_lower::task::exit(&mut self.code);
        } else {
            self.code.push(0xC3); // ret
        }
    }

    // ---- Statement emit ----
    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Printf(s) => self.emit_printf(s, false),
            Stmt::PrintfLn(s) => self.emit_printf(s, true),
            Stmt::If(c, t, e) => {
                let else_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit_test_cond(c, else_lbl);
                self.emit_stmt(t);
                if e.is_some() { self.emit_jmp_reloc(end_lbl); }
                self.resolve_label(else_lbl);
                if let Some(el) = e { self.emit_stmt(el); }
                self.resolve_label(end_lbl);
            }
            Stmt::While(c, b) => {
                let start = self.fresh_label();
                let end = self.fresh_label();
                self.continue_target.push(start);
                self.break_target.push(end);
                self.resolve_label(start);
                self.emit_test_cond(c, end);
                self.emit_stmt(b);
                self.emit_jmp_reloc(start);
                self.resolve_label(end);
                self.continue_target.pop();
                self.break_target.pop();
            }
            Stmt::DoWhile(b, c) => {
                let start = self.fresh_label();
                let end = self.fresh_label();
                self.continue_target.push(end);
                self.break_target.push(end);
                self.resolve_label(start);
                self.emit_stmt(b);
                self.resolve_label(end);
                self.emit_test_cond_jnz(c, start);
                self.continue_target.pop();
                self.break_target.pop();
            }
            Stmt::For(init, cond, inc, b) => {
                if let Some(e) = init { self.emit_expr(e); self.emit_drop(); }
                let start = self.fresh_label();
                let end = self.fresh_label();
                let inc_lbl = self.fresh_label();
                self.continue_target.push(inc_lbl);
                self.break_target.push(end);
                self.resolve_label(start);
                if let Some(c) = cond { self.emit_test_cond(c, end); }
                self.emit_stmt(b);
                self.resolve_label(inc_lbl);
                if let Some(e) = inc { self.emit_expr(e); self.emit_drop(); }
                self.emit_jmp_reloc(start);
                self.resolve_label(end);
                self.continue_target.pop();
                self.break_target.pop();
            }
            // `switch`: el valor se guarda en un hueco de pila y cada
            // comparacion lo relee de ahi.
            //
            // El despacho anterior hacia DOS `pop` habiendo empujado una
            // sola vez, asi que comparaba contra un valor de la pila que no
            // era suyo: siempre entraba por el primer `case`. Y el
            // `default:` era inalcanzable -- su etiqueta se fijaba DESPUES
            // de todos los cuerpos, o sea al final, saltandose su propio
            // codigo.
            Stmt::Switch(expr, cases) => {
                self.emit_expr(expr);
                let end = self.fresh_label();
                self.break_target.push(end);

                self.code.push(0x50); // push rax -> el valor vive en [rsp]

                let case_labels: Vec<u32> = cases.iter().map(|_| self.fresh_label()).collect();
                for (i, c) in cases.iter().enumerate() {
                    if let Some(val) = c.value {
                        self.code.extend_from_slice(&[0x48, 0xBA]); // mov rdx, imm64
                        self.code.extend_from_slice(&val.to_le_bytes());
                        self.code.extend_from_slice(&[0x48, 0x8B, 0x04, 0x24]); // mov rax, [rsp]
                        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
                        self.emit_jz_reloc(case_labels[i]);
                    }
                }
                // Sin coincidencia: al `default:` si existe, si no al final.
                match cases.iter().position(|c| c.value.is_none()) {
                    Some(i) => self.emit_jmp_reloc(case_labels[i]),
                    None => self.emit_jmp_reloc(end),
                }

                for (i, c) in cases.iter().enumerate() {
                    self.resolve_label(case_labels[i]);
                    for s in &c.stmts { self.emit_stmt(s); }
                }

                // `end` va ANTES de liberar el hueco para que un `break`
                // dentro de un caso tambien lo libere.
                self.resolve_label(end);
                self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
                self.break_target.pop();
            }
            Stmt::Break => {
                if let Some(lbl) = self.break_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Continue => {
                if let Some(lbl) = self.continue_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Return(Some(e)) => {
                // return de un float: el valor vive en xmm0 (ABI de retorno SSE);
                // el epilogo preserva xmm0. En contexto entero, emit_expr trunca.
                if self.expr_is_float(e) && !self.is_entry_function {
                    self.emit_fexpr(e);
                } else {
                    self.emit_expr(e);
                }
                self.emit_epilogue();
            }
            Stmt::Return(None) => {
                self.emit_epilogue();
            }
            Stmt::DeclAssign(typ, name, init) => {
                // Variable float/double: valor por la ruta SSE, store movsd/movss.
                if Self::is_float_ty(typ) {
                    match init {
                        Some(e) => self.emit_fexpr_operand(e), // acepta double d = 5;
                        None => self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC0]), // xorpd xmm0,xmm0 = 0.0
                    }
                    self.store_float_var(name);
                } else {
                    if let Some(e) = init { self.emit_expr(e); } else { self.emit_expr(&Expr::Int(0)); }
                    self.emit_store_var(name);
                }
            }
            // `T x = { ... }` -- la lista ya viene APLANADA a escrituras por
            // `parser/inicializador.rs`. Aqui no se sabe que es un designador:
            // solo "en el byte N va este valor, de este tamano".
            Stmt::DeclInit(typ, name, escrituras) => {
                let Some(&(base, _)) = self.var_offsets.get(name) else {
                    self.errors.push(format!("'{name}' no tiene hueco en la pila"));
                    return;
                };
                // * C99 section 6.7.9/21: lo NO mencionado vale cero. Se borra el
                // objeto entero ANTES de escribir, y por eso `{.y = 2}` deja la
                // `x` en 0 en vez de en lo que hubiera en la pila -- que seria
                // basura distinta en cada llamada y un bug imposible de repetir.
                let bytes = self.type_stack_size(typ);
                self.emit_cero_local(base, bytes);
                for e in escrituras {
                    self.emit_expr(&e.valor);
                    self.emit_store_rbp(base + e.offset as i32, &e.tipo);
                }
            }
            Stmt::Expr(e) => {
                self.emit_expr(e);
                self.emit_drop();
            }
            Stmt::Goto(label) => {
                self.code.extend_from_slice(&[0xE9]);
                self.goto_relocs.push((self.code.len(), label.clone()));
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            Stmt::Label(label) => {
                self.label_positions.insert(label.clone(), self.code.len());
                // patch any pending gotos to this label
                let mut i = 0;
                while i < self.goto_relocs.len() {
                    if self.goto_relocs[i].1 == *label {
                        let (off, _) = self.goto_relocs.swap_remove(i);
                        let disp = self.code.len() as i32 - (off as i32 + 4);
                        self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                    } else { i += 1; }
                }
            }
            Stmt::Block(stmts) => {
                for s in stmts { self.emit_stmt(s); }
            }
        }
    }

    fn emit_test_cond(&mut self, expr: &Expr, false_label: u32) {
        self.emit_expr(expr);
        self.code.extend_from_slice(&[0x85, 0xC0]);
        self.emit_jz_reloc(false_label);
    }

    fn emit_test_cond_jnz(&mut self, expr: &Expr, label: u32) {
        self.emit_expr(expr);
        self.code.extend_from_slice(&[0x85, 0xC0]);
        self.emit_jnz_reloc(label);
    }

    fn emit_drop(&mut self) {}

    /// `printf(fmt, args...)` -- la L2 de C sobre la libreria de formateo.
    ///
    /// Antes esto empujaba los argumentos a la pila y llamaba a un
    /// `bmo_printf` **importado de `userland_ring3`**: un simbolo que en BMO
    /// nadie resuelve, porque no hay enlazado dinamico de una libc. El
    /// programa compilaba y luego saltaba a una direccion sin parchear.
    ///
    /// Ahora el formateo se emite EN LINEA: cada trozo literal baja por la
    /// puerta de consola y cada conversion evalua su argumento y llama al
    /// emisor correspondiente de `bmo_lower::fmt`. Sin runtime, sin
    /// importaciones, sin dependencias del cargador.
    ///
    /// Lo especifico de C --que significa `%d`, que `%x` va en minusculas,
    /// que `%%` es un porcentaje-- se decide aqui. La libreria solo sabe
    /// convertir un numero en digitos.
    /// **La superficie de biblioteca que se emite en linea.**
    ///
    /// Devuelve `Some(())` si `name` era una de ellas y ya se emitio.
    ///
    /// * Cada una carga sus argumentos en registros y llama al emisor de L1.
    /// El orden importa: se evalua el ultimo primero y se apila, porque
    /// evaluar el segundo argumento puede machacar el registro donde estaba el
    /// primero -- un `memcpy(a, f(x), n)` con `f` llamando a otra cosa es el
    /// caso que lo destapa, y no se destapa en las pruebas faciles.
    fn emitir_biblioteca(&mut self, name: &str, args: &[Expr]) -> Option<()> {
        use bmo_lower::memoria;
        use bmo_lower::x86;
        match (name, args.len()) {
            // * `memcpy` YA NO ESTA AQUI, y su ausencia es el cambio.
            //
            // Cae por el camino de llamada normal y su cuerpo lo pone
            // `SINTETIZABLES`: emitido UNA vez, alcanzado con `call rel32`. Se
            // eligio este y no otro porque es el que mas se repite --por
            // `memcpy` pasa el blit de cada fotograma-- y porque su cuerpo no
            // tiene estado, asi que compartirlo no cambia lo que hace.
            //
            // `memmove` se queda en linea, y no por simetria descuidada: se
            // llama poco, y tocar los dos a la vez habria mezclado en un solo
            // cambio la conversion y un riesgo que no hacia falta correr.
            //
            // [!] Y de paso queda anotado lo que se vio al mirar esto: este arm
            // le da a `memmove` el MISMO `copiar`, que avanza de principio a
            // fin. Para solapamiento con `dst > src` eso corrompe --es
            // exactamente lo que `memmove` promete y `memcpy` no--, asi que
            // `memmove` hoy es un `memcpy` con otro nombre. **No lo arregla
            // este cambio** y esta sin arreglar, dicho aqui para que no se
            // cuente como hecho.
            ("memmove", 3) => {
                self.cargar_tres(args, x86::RDI, x86::RSI, x86::RCX);
                memoria::copiar(&mut self.code);
                // Devuelve el destino, que sigue en la pila porque el bucle se
                // llevo rdi por delante.
                self.soltar_tres();
                Some(())
            }
            // `strncmp` y `memcmp` comparten emision y se distinguen en UN bool:
            // si el terminador corta o no. Ver `memoria::comparar_n`.
            ("abs", 1) => {
                self.emit_expr(&args[0]);
                memoria::absoluto(&mut self.code);
                Some(())
            }
            // -- malloc / free ----------------------------------------
            //
            // * Cada `malloc` es **una peticion al kernel**, no un trozo de un
            // monton. Y eso NO es un atajo: es lo que hay hoy, dicho como es.
            //
            // El kernel entrega bloques enteros y no sabe repartirlos -- a
            // proposito, porque el asignador es politica y la politica vive en
            // Ring 3. Un monton de verdad (bump + listas libres) se escribe
            // encima de `bmo::Memoria`, y ese es el siguiente paso.
            //
            // **Limite declarado**: el kernel acepta CUATRO peticiones por
            // proceso, porque no hay forma de devolver memoria y ese numero es
            // el de fugas posibles. Un quinto `malloc` devuelve **0**, que es
            // lo que un programa de C ya sabe comprobar. Falla pronto y con
            // un valor que significa algo, en vez de agotar la RAM callando.
            //
            // Para el caso que motivo todo esto --DOOM pide su bloque UNA vez y
            // se lo administra con `Z_Zone`-- esto es exactamente suficiente.
            // * Los dos saltos de aqui van por ETIQUETA, y no es cosmetica.
            //
            // La primera version los emitio con desplazamientos contados a
            // mano, y el primero se quedo **seis bytes corto**: `jnz +0x1D`
            // cuando el camino hasta el `xor rax,rax` mide 35. O sea que
            // cuando el kernel RECHAZABA la peticion --la quinta, o una
            // demasiado grande-- el salto caia **dentro** del `jnz` siguiente y
            // el CPU seguia por la mitad de una instruccion.
            //
            // Y el detalle que lo hace peor: la rama buena estaba bien. Un
            // `malloc` que funciona cuatro veces y descarrila a la quinta pasa
            // por "el tope se cumple" en cualquier prueba que no llegue a la
            // quinta. Lo cazo el emulador con `opcode 0x05 no emitido por BMO`
            // -- que es la firma de haber aterrizado a media instruccion.
            //
            // Contar bytes a mano es escribir un enlazador en la cabeza cada
            // vez que alguien anade una instruccion en medio. Las etiquetas ya
            // estaban aqui; solo habia que usarlas.
            ("malloc", 1) => {
                use bmo_sem_asm::x86_64::Reg;
                let sin_bloque = self.fresh_label();
                let fin = self.fresh_label();
                self.emit_expr(&args[0]);                          // rax = bytes
                self.emit_asm(|a| { a.mov_reg(Reg::Rdx, Reg::Rax).unwrap(); });
                // rdi = CURRENT_TASK, rsi = OP_MEMORIA_PEDIR
                self.emit_asm(|a| { a.mov_imm64(Reg::Rdi, 0xFFFF_FFFF_FFFF_FFFE).unwrap(); });
                self.emit_asm(|a| { a.mov_imm64(Reg::Rsi, 0x15).unwrap(); });
                self.code.extend_from_slice(&[0xB8, 0, 0, 0, 0]);  // mov eax, NR_INVOKE(0)
                self.emit_call_to_syscall_stub();
                // El handle vuelve en rdx (`value`); rax lleva el codigo.
                // Si el codigo no es 0, no hay bloque: se devuelve 0.
                self.code.extend_from_slice(&[0x85, 0xC0]);        // test eax, eax
                self.emit_jnz_reloc(sin_bloque);
                // * El handle a la pila ANTES de la segunda llamada, que pisa
                // `rdx`. Es el dato que `fread` necesita y que hasta ahora se
                // perdia justo aqui: se usaba para pedir la base y se tiraba.
                self.code.push(0x52);                              // push rdx
                // Segunda llamada: MEM_OP_BASE sobre el handle.
                self.emit_asm(|a| { a.mov_reg(Reg::Rdi, Reg::Rdx).unwrap(); });
                self.emit_asm(|a| { a.mov_imm64(Reg::Rsi, 0x01).unwrap(); });
                self.code.extend_from_slice(&[0x48, 0x31, 0xD2]);  // xor rdx, rdx
                self.code.extend_from_slice(&[0xB8, 0, 0, 0, 0]);  // mov eax, NR_INVOKE
                self.emit_call_to_syscall_stub();
                // El `pop` va ANTES del test, y eso no es estilo: por el camino
                // de fallo se salta a `sin_bloque`, y saltar con algo aun en la
                // pila la descuadra para el resto de la funcion.
                self.code.extend_from_slice(&[0x41, 0x58]);        // pop r8 (el handle)
                self.code.extend_from_slice(&[0x85, 0xC0]);        // test eax, eax
                self.emit_jnz_reloc(sin_bloque);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);  // mov rax, rdx (la base)
                // * PUBLICAR EL BLOQUE. Sin esto `fread` no puede existir.
                //
                // El kernel solo acepta escribir dentro de un bloque que el
                // concedio, y para pedirselo hay que darle SU handle y un
                // desplazamiento. `malloc` es el unico que tiene las dos cosas
                // --el handle vino en la primera llamada, la base en la segunda--
                // y hasta ahora tiraba el handle en cuanto sacaba la base.
                //
                // Se guardan en dos globales **solo si el programa las
                // declaro** (las trae `<bmo/archivo.h>`). Un programa que no
                // lee ficheros no paga ni un byte por esto, que es la razon de
                // preguntar por el nombre en vez de emitirlas siempre.
                self.publicar_bloque();
                self.emit_jmp_reloc(fin);
                self.resolve_label(sin_bloque);
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);  // xor rax, rax
                self.resolve_label(fin);
                Some(())
            }
            // `free` NO devuelve nada al kernel -- no hay forma, y decirlo aqui
            // vale mas que emitir una llamada que no haria nada. El bloque vive
            // hasta que el proceso muere, y entonces se destruye su espacio de
            // direcciones entero.
            //
            // Se acepta porque el codigo ajeno lo llama y quitarlo a mano de
            // 35.000 lineas no es una opcion. Evalua su argumento, por si tiene
            // efectos secundarios.
            ("free", 1) => {
                self.emit_expr(&args[0]);
                self.emit_xor_eax();
                Some(())
            }
            _ => None,
        }
    }

    /// Tres argumentos a tres registros, evaluando de derecha a izquierda.
    ///
    /// * Los tres se dejan EN LA PILA y los registros se cargan **leyendo**,
    /// no sacando. La primera version los sacaba con `pop` y apilaba el
    /// destino dos veces para poder devolverlo -- y eso desalineaba los tres
    /// `pop`: `memset` acababa con el valor de relleno en el registro del
    /// contador. Salio como `-16,-16,-16` donde tenia que salir `65,65,65`.
    ///
    /// Leyendo con desplazamiento no hay orden que cuadrar: cada argumento
    /// esta donde se puso. Y quien llama limpia con [`Self::soltar_tres`], que
    /// es lo que faltaba tambien -- la version de `pop` dejaba dos valores
    /// vivos en la pila por cada `memcpy`, y eso no se ve hasta que un bucle
    /// hace mil.
    fn cargar_tres(&mut self, args: &[Expr], r0: u8, r1: u8, r2: u8) {
        self.emit_expr(&args[2]);
        self.code.push(0x50); // push n        -> [rsp+16]
        self.emit_expr(&args[1]);
        self.code.push(0x50); // push src      -> [rsp+8]
        self.emit_expr(&args[0]);
        self.code.push(0x50); // push dst      -> [rsp]
        self.mov_desde_pila(r0, 0);
        self.mov_desde_pila(r1, 8);
        self.mov_desde_pila(r2, 16);
    }

    /// Recupera el destino en `rax` y tira los otros dos. Cierra a
    /// [`Self::cargar_tres`].
    fn soltar_tres(&mut self) {
        self.code.push(0x58);                               // pop rax (dst)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 16]); // add rsp, 16
    }

    /// `mov <r64>, [rsp+disp8]`.
    fn mov_desde_pila(&mut self, reg: u8, disp: u8) {
        self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 }); // REX.W (+R)
        self.code.push(0x8B);
        self.code.push(0x44 | ((reg & 7) << 3)); // modrm: [SIB + disp8]
        self.code.push(0x24);                    // SIB: base = rsp
        self.code.push(disp);
    }

    fn emit_printf_variadic(&mut self, args: &[Expr]) {
        let Expr::StringLit(format) = &args[0] else {
            self.errors.push(
                "printf con formato calculado en tiempo de ejecucion no se compila: \
                 el formato debe ser un literal para poder emitirlo en linea"
                    .to_string(),
            );
            return;
        };
        let format = format.clone();
        let va_args: Vec<Expr> = args[1..].to_vec();
        let mut next_arg = 0usize;
        let mut literal: Vec<u8> = Vec::new();

        // * **TODOS los argumentos se evaluan ANTES de escribir un solo byte.**
        //
        // Antes no: el emisor recorria la plantilla y evaluaba cada argumento
        // al llegar a su `%`, intercalado con la salida de los literales. Con
        // argumentos sin efectos daba igual, pero `printf("[%d]", f())` con `f`
        // imprimiendo sacaba `[` **antes** que lo de `f` -- y en C estandar
        // todos los argumentos se evaluan antes de entrar en la llamada.
        //
        // Lo destapo la matriz de C++ al probar RAII: un destructor que
        // imprime es justo un argumento con efectos. Es la clase de diferencia
        // que solo aparece al portar codigo de otro, y entonces ya no se sabe
        // de donde viene.
        //
        // Se guardan en la PILA y no en ranuras del marco a proposito: los
        // ayudantes de `bmo_lower::fmt` y `console` estan **equilibrados en
        // rsp** (cada `sub rsp` tiene su `add rsp`), asi que un offset
        // relativo a rsp sigue valiendo entre una conversion y la siguiente.
        // Y asi no hay que reservar sitio en el prologo para algo que solo
        // vive dentro de un `printf`.
        let n = va_args.len();
        for a in &va_args {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }

        let chars: Vec<char> = format.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != '%' {
                let mut buf = [0u8; 4];
                literal.extend_from_slice(chars[i].encode_utf8(&mut buf).as_bytes());
                i += 1;
                continue;
            }

            // Saltar los modificadores de longitud: en BMO todo entero viaja
            // en 64 bits, asi que `%ld` y `%d` producen lo mismo.
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j], 'l' | 'h' | 'z' | 'j' | 't') {
                j += 1;
            }
            let Some(&conversion) = chars.get(j) else {
                self.errors
                    .push("'%' al final del formato de printf".to_string());
                return;
            };

            if conversion == '%' {
                literal.push(b'%');
                i = j + 1;
                continue;
            }

            // Todo lo literal acumulado sale ANTES de la conversion.
            if !literal.is_empty() {
                bmo_lower::console::write_const(&mut self.code, &literal);
                literal.clear();
            }

            if next_arg >= n {
                self.errors.push(format!(
                    "printf: '%{conversion}' no tiene argumento correspondiente"
                ));
                return;
            }
            // El valor ya esta calculado en la pila: el primero empujado es el
            // que queda mas arriba, asi que el i-esimo esta en `n-1-i`.
            self.emit_cargar_de_pila(n - 1 - next_arg);
            next_arg += 1;

            // * Aqui estaba el formateador ENTERO, en linea, en cada `%`.
            //
            // Un `printf("%d %d %d")` se llevaba tres copias del mismo
            // conversor de entero a decimal, y no habia programa que no
            // pagara eso: `printf` es la funcion que todos usan. Ahora es un
            // `call` de cinco bytes al cuerpo que puso `SINTETIZABLES`.
            match conversion {
                'd' | 'i' => self.emit_call_sintetizada("__bmo_fmt_i64"),
                'u' => self.emit_call_sintetizada("__bmo_fmt_u64_dec"),
                'x' => self.emit_call_sintetizada("__bmo_fmt_u64_hex"),
                'c' => self.emit_call_sintetizada("__bmo_fmt_char"),
                's' => self.emit_call_sintetizada("__bmo_fmt_cstr"),
                other => {
                    self.errors.push(format!(
                        "printf: '%{other}' aun no se compila (se compilan \
                         %d %i %u %x %c %s %%; los flotantes necesitan la ruta SSE)"
                    ));
                    return;
                }
            }
            i = j + 1;
        }

        if !literal.is_empty() {
            bmo_lower::console::write_const(&mut self.code, &literal);
        }

        // Devolver la pila. Va DESPUES del ultimo literal, no antes: entre
        // medias todavia se leen ranuras relativas a rsp.
        self.emit_soltar_pila(n);

        if next_arg < n {
            self.errors.push(format!(
                "printf: sobran {} argumento(s) para el formato dado",
                n - next_arg
            ));
        }
    }

    /// `mov rax, [rsp + slot*8]` -- lee un argumento ya calculado.
    fn emit_cargar_de_pila(&mut self, slot: usize) {
        let disp = (slot * 8) as i64;
        if disp <= 127 {
            // 48 8B 44 24 disp8
            self.code.extend_from_slice(&[0x48, 0x8B, 0x44, 0x24, disp as u8]);
        } else {
            // 48 8B 84 24 disp32
            self.code.extend_from_slice(&[0x48, 0x8B, 0x84, 0x24]);
            self.code.extend_from_slice(&(disp as u32).to_le_bytes());
        }
    }

    /// `add rsp, ranuras*8` -- suelta los argumentos guardados.
    fn emit_soltar_pila(&mut self, ranuras: usize) {
        if ranuras == 0 { return; }
        let bytes = (ranuras * 8) as i64;
        if bytes <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, 0xC4, bytes as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
            self.code.extend_from_slice(&(bytes as u32).to_le_bytes());
        }
    }
    /// `printf("literal")` -- la L2 de C sobre la puerta generica (L1).
    ///
    /// Lo especifico de C que se resuelve AQUI y en ningun otro sitio: que la
    /// cadena es un literal ya escapado por el lexer y que `\n` va pegado al
    /// final. Los bytes resultantes se los entrega a `bmo_lower::console`,
    /// que no sabe que existe C.
    ///
    /// Antes esto emitia `lea rdi,[str]; mov esi,len; syscall 0x1F0`: un
    /// numero plano que el kernel no despacha, pasando ademas un PUNTERO,
    /// que la superficie congelada rechaza por diseno. No imprimia nada en
    /// hardware. La cadena ya no necesita vivir en `.rodata`: viaja como
    /// inmediatos dentro de las propias instrucciones.
    fn emit_printf(&mut self, s: &str, newline: bool) {
        let text = if newline { let mut t = s.to_string(); t.push('\n'); t } else { s.to_string() };
        bmo_lower::console::write_const(&mut self.code, text.as_bytes());
    }

    // ---- Expression emit ----
    fn emit_expr(&mut self, expr: &Expr) {
        // Guard SSE: una expresion FLOTANTE que llega a la ruta entera esta en
        // contexto entero (int x = 1.5; return d;) -> calcular en xmm y truncar
        // a rax (cvttsd2si). Las comparaciones dan int 0/1 (no son float) y se
        // manejan abajo. emit_fexpr_operand solo llama aqui para NO-floats, asi
        // que no hay recursion infinita.
        if self.expr_is_float(expr) {
            self.emit_fexpr(expr);
            self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            return;
        }
        match expr {
            Expr::Int(n) => {
                let v = *n as u64;
                self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, v).unwrap(); });
            }
            // El guard SSE de arriba ya captura los floats; este brazo solo
            // existe por exhaustividad (defensivo: trunca a entero).
            Expr::FloatLit(_) => {
                self.emit_fexpr(expr);
                self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            }
            Expr::CharLit(c) => {
                let v = *c as u64;
                self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, v).unwrap(); });
            }
            Expr::StringLit(s) => {
                // lea rax, [rip + disp] -- fixup patched in patch_string_fixups
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                let idx = self.strings.iter().position(|t| t == s).unwrap_or(0);
                self.fixups.push(Fixup { lea_offset: self.code.len() - 4, string_idx: idx });
            }
            Expr::Var(name) => {
                self.emit_load_var(name);
            }
            Expr::Call(name, args) => {
                // * Las funciones de biblioteca que se emiten EN LINEA.
                //
                // No hay libreria que enlazar, y no es una carencia: es el
                // modelo. Un `.bex` es una imagen entera y BEF no resuelve
                // relocaciones contra un `.so`. Emitir el bucle cuesta treinta
                // bytes y ahorra un enlazador, un formato de libreria y un
                // cargador dinamico.
                //
                // Lo que se emite vive en `bmo_lower::memoria` (L1) porque
                // "mueve estos bytes" no tiene semantica de lenguaje: COBOL
                // mueve grupos y Ada asigna arrays con la misma emision. Aqui
                // solo se pone el nombre que usa C.
                if let Some(n) = self.emitir_biblioteca(name, args) {
                    let _ = n;
                    return;
                }
                // Special case: printf -> emit bmo_printf from userland_ring3
                if name == "printf" && !args.is_empty() {
                    self.emit_printf_variadic(args);
                    return;
                }
                // La pareja de `printf`: se emiten EN LINEA por lo mismo -- aqui
                // no hay libc que enlazar ni simbolo que nadie resuelva.
                if name == "getchar" && args.is_empty() {
                    self.emit_getchar();
                    return;
                }
                if name == "scanf" && !args.is_empty() {
                    self.emit_scanf(args);
                    return;
                }
                // Llamada INDIRECTA? El nombre no es una funcion pero SI una
                // variable -> contiene una direccion (puntero a funcion).
                let is_indirect = !self.known_functions.contains(name)
                    && (self.var_offsets.contains_key(name) || self.global_offsets.contains_key(name));

                // -- Los argumentos, de derecha a izquierda --
                //
                // Cuantas ranuras ocupa cada uno lo dice el PARAMETRO, no la
                // expresion: un `struct` de 12 bytes ocupa dos aunque quien lo
                // pase sea una variable. Si no hay firma --llamada indirecta por
                // puntero-- se supone una ranura, que es lo que era antes.
                let tipos_param: Vec<TypeSpec> = self
                    .firmas
                    .get(name)
                    .map(|(p, _)| p.clone())
                    .unwrap_or_default();
                let mut ranuras_total = 0u32;
                for (i, arg) in args.iter().enumerate().rev() {
                    match tipos_param.get(i) {
                        Some(t) if self.es_agregado(t) => {
                            let bytes = self.type_stack_size(t);
                            ranuras_total += agregados::ranuras(bytes);
                            self.emit_empuja_agregado(arg, bytes);
                        }
                        _ => {
                            ranuras_total += 1;
                            self.emit_expr(arg);
                            self.code.push(0x50); // push rax
                        }
                    }
                }
                // Devolver un agregado es un tercer mecanismo (puntero oculto)
                // y todavia no esta. Se dice: devolver ocho bytes de un struct
                // de doce seria la clase de mentira que este compilador no
                // cuenta.
                if let Some((_, ret)) = self.firmas.get(name) {
                    if self.es_agregado(&ret.clone()) {
                        self.errors.push(format!(
                            "'{name}' devuelve un struct por valor, y eso aun no se compila \
                             (pasa un puntero al destino como parametro)"
                        ));
                    }
                }
                if is_indirect {
                    self.emit_load_var(name);                 // rax = direccion
                    self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                } else {
                    // call rel32 placeholder (directa)
                    self.code.extend_from_slice(&[0xE8]);
                    self.call_relocs.push(CallReloc { offset: self.code.len(), target: name.clone() });
                    self.code.extend_from_slice(&[0, 0, 0, 0]);
                    // Track stdlib imports for Ring 3 apps
                    if self.target == TargetProfile::Ring3App && !self.function_offsets.contains_key(name) {
                        self.stdlib_imports.insert(name.clone());
                    }
                }
                // Se quita de la pila lo que se PUSO, que ya no es una ranura
                // por argumento.
                let n = ranuras_total * 8;
                if n > 0 {
                    if n <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
                        self.code.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
            Expr::Syscall(def, args) => {
                // x86-64 SysV ABI syscall convention:
                // args: rdi, rsi, rdx, r10, r8, r9  ->  result in rax.
                // El `mov <reg>, rax` lo emite el encoder sem-asm (antes era
                // la tabla reg_mov de bytes a mano -- misma dup que COBOL).
                use bmo_sem_asm::x86_64::Reg;
                const ARG_REGS: [Reg; 6] =
                    [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::R10, Reg::R8, Reg::R9];
                for (i, arg) in args.iter().enumerate() {
                    if i < 6 {
                        self.emit_expr(arg);          // rax = expr value
                        let dst = ARG_REGS[i];
                        self.emit_asm(|a| { a.mov_reg(dst, Reg::Rax).unwrap(); });
                    }
                }
                self.code.extend_from_slice(&[0xB8]);        // mov eax, imm32
                self.code.extend_from_slice(&def.nr.to_le_bytes());
                self.emit_call_to_syscall_stub();
            }
            Expr::Assign(name, val) => {
                // `p = q` con `p` agregado: se copian sus BYTES, todos.
                //
                // Antes caia al camino normal --`mov rax,[q]` + `mov [p],rax`--
                // que se lleva ocho y deja el resto con lo que hubiera. Un
                // struct de 12 se copiaba a medias, en silencio.
                if let Some(t) = self.var_type_of(name) {
                    if self.es_agregado(&t) {
                        let bytes = self.type_stack_size(&t);
                        let destino = Expr::Var(name.clone());
                        self.emit_asigna_agregado(&destino, val, bytes);
                        return;
                    }
                }
                // Asignacion a variable float/double -> ruta SSE.
                if self.var_type_of(name).map_or(false, |t| Self::is_float_ty(&t)) {
                    self.emit_fexpr_operand(val);
                    self.store_float_var(name);
                } else {
                    self.emit_expr(val);
                    self.emit_store_var(name);
                }
            }
            Expr::Neg(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]); }
            Expr::Not(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x85, 0xC0, 0x0F, 0x94, 0xC0]); }
            Expr::BitNot(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD0]); }
            Expr::PreInc(name) => {
                self.emit_inc_var(name);
                // rax already has new value
            }
            Expr::PreDec(name) => {
                self.emit_dec_var(name);
            }
            Expr::PostInc(name) => {
                self.emit_load_var(name);
                self.code.push(0x50); // push old value
                self.emit_inc_var(name);
                self.code.push(0x58); // pop rax (old value)
            }
            Expr::PostDec(name) => {
                self.emit_load_var(name);
                self.code.push(0x50);
                self.emit_dec_var(name);
                self.code.push(0x58);
            }
            // `*p` debe leer el TAMANO DEL APUNTADO, no siempre 8 bytes.
            // Antes `*(p+1)` con `int *p` leia 8 bytes desde la posicion
            // correcta, o sea dos enteros pegados: devolvia 504403158366158848
            // en vez de 6.
            Expr::Deref(a) => {
                self.emit_expr(a); // rax = direccion
                match self.pointee_type(a) {
                    Some(TypeSpec::Char) => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]),
                    Some(TypeSpec::UnsignedChar) => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]),
                    Some(TypeSpec::Short) => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
                    Some(TypeSpec::UnsignedShort) => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
                    Some(TypeSpec::Int) => self.code.extend_from_slice(&[0x48, 0x63, 0x00]),
                    Some(TypeSpec::UnsignedInt) => self.code.extend_from_slice(&[0x8B, 0x00]),
                    _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]),
                }
            }
            Expr::AddrOf(inner) => {
                match inner.as_ref() {
                    Expr::Var(name) => {
                        if let Some(&(offset, _)) = self.var_offsets.get(name) {
                            if offset >= -128 && offset <= 127 {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                            } else {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                                self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                            }
                        } else if self.global_offsets.contains_key(name) {
                            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                            self.global_fixups.push((self.code.len() - 4, name.clone()));
                        } else if self.known_functions.contains(name) {
                            // &myfunc -- direccion de la funcion
                            self.emit_func_addr(name);
                        } else { self.emit_xor_eax(); }
                    }
                    Expr::Subscript(name, idx, scale) => {
                        self.emit_subscript_addr(name, idx, *scale);
                    }
                    Expr::Deref(ptr) => {
                        self.emit_expr(ptr); // rax = address of the pointed-to data
                    }
                    _ => self.emit_xor_eax(),
                }
            }
            Expr::Subscript(name, index, scale) => {
                // direccion exacta (array o puntero) + carga del TAMANO del elemento
                self.emit_subscript_addr(name, index, *scale);
                let elem = self.elem_type_of(name);
                self.emit_load_elem(&elem);
            }
            Expr::AssignSubscript(name, index, scale, val) => {
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_subscript_addr(name, index, *scale); // rax = direccion
                self.code.push(0x5A);         // pop rdx = valor
                let elem = self.elem_type_of(name);
                self.emit_store_elem(&elem);  // [rax] = rdx (tamano exacto)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = valor (resultado del assign)
            }
            Expr::IndexPtr(base, index, elem) => {
                // p->arr[i]: direccion = base(puntero) + i*sizeof(elem), luego load
                self.emit_index_ptr_addr(base, index, elem);
                self.emit_load_elem(&elem.clone());
            }
            Expr::AssignIndexPtr(base, index, elem, val) => {
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_index_ptr_addr(base, index, elem); // rax = direccion
                self.code.push(0x5A);         // pop rdx = valor
                self.emit_store_elem(&elem.clone());
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
            }
            Expr::CallPtr(callee, args) => {
                // (*fp)(args): args a la pila, callee da la direccion, call rax
                for arg in args.iter().rev() {
                    self.emit_expr(arg);
                    self.code.push(0x50);
                }
                self.emit_expr(callee);                     // rax = direccion de la funcion
                self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                let n = args.len() as u32 * 8;
                if n > 0 {
                    if n <= 127 { self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]); }
                    else { self.code.extend_from_slice(&[0x48, 0x81, 0xC4]); self.code.extend_from_slice(&n.to_le_bytes()); }
                }
            }
            // Tras `emit_binop`: rdx = operando IZQUIERDO, rax = DERECHO.
            // Los operadores conmutativos daban igual; los que no lo son
            // estaban invertidos y nadie lo vio hasta ejecutarlos.
            // `p + n` con `p` puntero avanza n ELEMENTOS, no n bytes. Antes
            // sumaba bytes: con `int *p`, `*(p+1)` leia desde el byte 1 en
            // vez del 4, o sea a caballo entre dos enteros.
            Expr::Add(a, b) => {
                if let Some(scale) = self.pointer_scale(a) {
                    let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(a, &scaled, &[0x48, 0x01, 0xD0]);
                } else if let Some(scale) = self.pointer_scale(b) {
                    let scaled = Expr::Mul(a.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(&scaled, b, &[0x48, 0x01, 0xD0]);
                } else {
                    self.emit_binop(a, b, &[0x48, 0x01, 0xD0]);
                }
            }
            // `a - b`. Antes: `sub rax, rdx` = b - a, o sea al reves.
            // `10 - 3` daba -7.
            Expr::Sub(a, b) => {
                const SUB: &[u8] = &[
                    0x48, 0x29, 0xC2, // sub rdx, rax   -> rdx = a - b
                    0x48, 0x89, 0xD0, // mov rax, rdx
                ];
                // `p - n` retrocede n ELEMENTOS (la resta puntero-puntero,
                // que daria un indice, no se deduce aqui).
                match self.pointer_scale(a) {
                    Some(scale) if self.pointer_scale(b).is_none() => {
                        let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                        self.emit_binop(a, &scaled, SUB);
                    }
                    _ => self.emit_binop(a, b, SUB),
                }
            }
            Expr::Mul(a, b) => self.emit_binop(a, b, &[0x48, 0x0F, 0xAF, 0xC2]),
            // `a / b` CON SIGNO. Antes hacia dos `pop` habiendo empujado una
            // sola vez --se llevaba un valor de la pila que no era suyo-- y
            // ademas dividia sin signo. `10 / 3` daba 0.
            Expr::Div(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax   -> divisor = b
                0x48, 0x89, 0xD0, // mov rax, rdx   -> dividendo = a
                0x48, 0x99,       // cqo            -> extiende el signo
                0x48, 0xF7, 0xF9, // idiv rcx
            ]),
            // `a % b`: el resto queda en rdx.
            Expr::Mod(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax
                0x48, 0x89, 0xD0, // mov rax, rdx
                0x48, 0x99,       // cqo
                0x48, 0xF7, 0xF9, // idiv rcx
                0x48, 0x89, 0xD0, // mov rax, rdx  -> el resto
            ]),
            // Comparaciones: si algun operando es float -> comisd (setcc unsigned);
            // si no, la comparacion entera de siempre.
            // Comparaciones enteras: todas comparan `a` contra `b` en ese
            // orden y usan el setcc que les toca. Antes `<`, `>` y `>=`
            // comparaban al reves --`1 < 2` daba 0-- porque la comparacion se
            // hacia sobre `b - a` con el setcc de la forma directa.
            Expr::Eq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x94) } else { self.emit_cmp(a, b, 0x94) },
            Expr::Neq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x95) } else { self.emit_cmp(a, b, 0x95) },
            Expr::Lt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x92) } else { self.emit_cmp(a, b, 0x9C) },
            Expr::Gt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x97) } else { self.emit_cmp(a, b, 0x9F) },
            Expr::Le(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x96) } else { self.emit_cmp(a, b, 0x9E) },
            Expr::Ge(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x93) } else { self.emit_cmp(a, b, 0x9D) },
            Expr::BitAnd(a, b) => self.emit_binop(a, b, &[0x48, 0x21, 0xD0]),
            Expr::BitXor(a, b) => self.emit_binop(a, b, &[0x48, 0x31, 0xD0]),
            Expr::BitOr(a, b) => self.emit_binop(a, b, &[0x48, 0x09, 0xD0]),
            // `a << b` / `a >> b`. Antes desplazaban el operando DERECHO por
            // el izquierdo: `1 << 3` intentaba `3 << 1`.
            //
            // El desplazamiento a la derecha es ARITMETICO (`sar`), que es
            // lo correcto para `int`. Un tipo sin signo querria `shr`; hoy
            // el codegen no arrastra esa distincion hasta aqui.
            Expr::Shl(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax   -> cuenta = b
                0x48, 0x89, 0xD0, // mov rax, rdx   -> valor  = a
                0x48, 0xD3, 0xE0, // shl rax, cl
            ]),
            Expr::Shr(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax
                0x48, 0x89, 0xD0, // mov rax, rdx
                0x48, 0xD3, 0xF8, // sar rax, cl
            ]),
            // `&&` y `||` valen 0 o 1, no "el operando que quedo". Antes
            // `0 || 3` daba 3: cortocircuitaba bien pero devolvia el valor
            // crudo, y el estandar dice que el resultado es `int` 0/1.
            Expr::LAnd(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
            }
            Expr::LOr(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jnz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
            }
            Expr::Conditional(c, t, f) => {
                let else_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit_test_cond(c, else_lbl);
                self.emit_expr(t);
                self.emit_jmp_reloc(end_lbl);
                self.resolve_label(else_lbl);
                self.emit_expr(f);
                self.resolve_label(end_lbl);
            }
            Expr::Field(base, _field, offset, ftyp) => {
                // direccion base + offset, carga del TAMANO/SIGNO del campo
                self.emit_expr_as_ptr(base);
                self.emit_add_offset(*offset);
                self.emit_load_elem(&ftyp.clone());
            }
            Expr::Arrow(ptr, _field, offset, ftyp) => {
                self.emit_expr(ptr);
                self.emit_add_offset(*offset);
                self.emit_load_elem(&ftyp.clone());
            }
            Expr::AssignField(base, _field, offset, ftyp, val) => {
                self.emit_expr(val);
                self.code.push(0x50);
                self.emit_expr_as_ptr(base);
                self.emit_add_offset(*offset);
                self.code.push(0x5A);
                // store del TAMANO exacto: pt.x=10 con x:int ya no pisa a pt.y
                self.emit_store_elem(&ftyp.clone());
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
            }
            Expr::AssignDeref(addr, val) => {
                self.emit_expr(val); // rax = value
                self.code.push(0x50); // push value
                self.emit_expr(addr); // rax = address
                self.code.push(0x5A); // pop rdx (value)
                self.code.extend_from_slice(&[0x48, 0x89, 0x10]); // mov [rax], rdx
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx (return value)
            }
            Expr::AssignArrow(ptr, _field, offset, ftyp, val) => {
                self.emit_expr(val); // rax = value
                self.code.push(0x50); // push value
                self.emit_expr(ptr); // rax = pointer
                self.emit_add_offset(*offset);
                self.code.push(0x5A); // pop rdx (value)
                self.emit_store_elem(&ftyp.clone()); // tamano exacto del campo
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
            }
            Expr::Intrinsic(name, args) => self.emit_intrinsic(name, args),
            Expr::Cast(t, inner) => {
                // cast REAL: trunca/extiende rax al tamano del tipo destino.
                // Antes era no-op: (char)300 quedaba como 300.
                self.emit_expr(inner);
                match t {
                    TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0xC0]), // movsx rax, al
                    TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx
                    TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0xC0]),
                    TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]),
                    TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0xC0]), // movsxd rax, eax
                    TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0xC0]), // mov eax, eax (zero-ext)
                    _ => {} // 64-bit y punteros: sin cambio de representacion
                }
            }
            Expr::Comma(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.emit_expr(e);
                    if i < exprs.len() - 1 { self.emit_drop(); }
                }
            }
        }
    }

    // ---- Subscript helpers (array en memoria vs puntero-valor) ----

    /// `name` es un array (su memoria vive en el slot) o un puntero (el slot
    /// guarda una direccion)? La distincion que antes no existia y corrompia.
    fn var_is_array(&self, name: &str) -> bool {
        if let Some(&(_, ref t)) = self.var_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        if let Some(&(_, ref t)) = self.global_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        false
    }

    /// Tipo del elemento de un array/puntero (para cargas/stores del tamano exacto).
    fn elem_type_of(&self, name: &str) -> TypeSpec {
        let t = self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()));
        match t {
            Some(TypeSpec::Array(e, _)) | Some(TypeSpec::Ptr(e)) => *e,
            _ => TypeSpec::Long,
        }
    }

    /// rax = rax * scale (shl si es potencia de 2; imul si no -- structs)
    /// * Escalar el indice por el tamano de UN paso.
    ///
    /// El paso ya no cabe siempre en un byte: en `int grid[2][3]` un paso del
    /// indice de fuera es una FILA entera --doce bytes--, y en
    /// `gammatable[5][256]` son 256. Por eso hay tres formas y no dos, y la
    /// tercera es la que faltaba: `imul` con inmediato de 32 bits.
    fn emit_scale_index(&mut self, scale: u32) {
        if scale <= 1 {
            return;
        }
        if scale.is_power_of_two() {
            // shl rax, log2(scale)
            self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, scale.trailing_zeros() as u8]);
        } else if scale <= i8::MAX as u32 {
            // imul rax, rax, imm8
            self.code.extend_from_slice(&[0x48, 0x6B, 0xC0, scale as u8]);
        } else {
            // imul rax, rax, imm32
            self.code.extend_from_slice(&[0x48, 0x69, 0xC0]);
            self.code.extend_from_slice(&scale.to_le_bytes());
        }
    }

    /// rax = direccion de name[idx]. Array -> base = lea del slot;
    /// puntero -> base = VALOR del slot. Local o global.
    fn emit_subscript_addr(&mut self, name: &str, index: &Expr, scale: u32) {
        self.emit_expr(index);
        self.emit_scale_index(scale);
        self.code.push(0x50); // push indice escalado
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]); // lea rax,[rip+global]
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
        } else {
            self.emit_load_var(name); // rax = valor del puntero
        }
        self.code.push(0x5A); // pop rdx = indice escalado
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
    }

    /// rax += offset (encoding corto si cabe en imm8)
    fn emit_add_offset(&mut self, offset: u32) {
        if offset == 0 { return; }
        let off = offset as i32;
        if off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x05]);
            self.code.extend_from_slice(&(off as u32).to_le_bytes());
        }
    }

    /// rax = base_ptr + index * sizeof(elem), donde `base` es una EXPRESION
    /// que produce un puntero (p->arr, a+1...). Deja la direccion en rax.
    fn emit_index_ptr_addr(&mut self, base: &Expr, index: &Expr, elem: &TypeSpec) {
        let size = self.type_stack_size(elem).max(1) as u32;
        self.emit_expr(base);          // rax = puntero base
        self.code.push(0x50);          // push base
        self.emit_expr(index);         // rax = indice
        self.emit_scale_index(size);   // rax = indice * size
        self.code.push(0x5A);          // pop rdx = base
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
    }

    /// Carga [rax] -> rax con el tamano y signo EXACTOS del elemento.
    /// Antes siempre era `mov rax,[rax]` (8 bytes): leer int[i] traia basura vecina.
    fn emit_load_elem(&mut self, elem: &TypeSpec) {
        match elem {
            // agregados: la direccion ES el valor (a.b.c anidado, arrays en structs)
            TypeSpec::Array(_, _) | TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => {}
            TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]), // movsx rax, byte
            TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]), // movzx
            TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
            TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
            TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0x00]), // movsxd rax, dword
            TypeSpec::UnsignedInt | TypeSpec::Float => self.code.extend_from_slice(&[0x8B, 0x00]), // mov eax, dword
            _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]), // mov rax, qword
        }
    }

    /// Guarda rdx -> [rax] con el tamano EXACTO del elemento.
    /// Antes un store de 8 bytes a int[i] pisaba el elemento siguiente.
    fn emit_store_elem(&mut self, elem: &TypeSpec) {
        match self.type_stack_size(elem) {
            1 => self.code.extend_from_slice(&[0x88, 0x10]),        // mov [rax], dl
            2 => self.code.extend_from_slice(&[0x66, 0x89, 0x10]),  // mov [rax], dx
            4 => self.code.extend_from_slice(&[0x89, 0x10]),        // mov [rax], edx
            _ => self.code.extend_from_slice(&[0x48, 0x89, 0x10]),  // mov [rax], rdx
        }
    }

    /// LA FUSION sem-asm<->C: emite un intrinseco de la tabla.
    /// Evalua cada argumento, lo apila, y lo vuelca al registro que dicta
    /// la tabla justo antes de los bytes de la instruccion. Bytes EXACTOS,
    /// sin caja negra: si el nombre o la aridad no cuadran -> error, no adivina.
    fn emit_intrinsic(&mut self, name: &str, args: &[Expr]) {
        // * `__va_arg(i)` -- el argumento variadico numero `i`, contando desde 0
        // despues de los que tienen nombre.
        //
        // No sale de la tabla de sem-asm porque no es una instruccion del CPU:
        // es aritmetica sobre el marco de pila, y depende de CUANTOS parametros
        // con nombre tiene la funcion que lo pregunta.
        //
        // Y es aritmetica y no ABI porque BMO C pasa los argumentos **por la
        // pila**, de derecha a izquierda. En la convencion de registros de
        // SysV esto obligaria a volcar seis registros en el prologo y a llevar
        // dos cursores (registros y pila); aqui los argumentos ya estan
        // seguidos en memoria y el numero `i` es un desplazamiento. La
        // convencion mas vieja resulto ser la que hace los varargs triviales.
        //
        // El indice es de EJECUCION, no una constante: sin eso no se puede
        // recorrer los argumentos en un bucle, que es justo lo que hace un
        // `vsprintf` -- y un `vsprintf` es lo que pide `I_Error(fmt, ...)`.
        if name == "va_arg" {
            if args.len() != 1 {
                self.errors.push(
                    "__va_arg(i) espera UN argumento: el indice del variadico".into());
                return;
            }
            if !self.es_variadica {
                self.errors.push(
                    "__va_arg() en una funcion que no declara '...': no hay argumentos \
                     variadicos que leer".into());
                return;
            }
            self.emit_expr(&args[0]);                       // rax = i
            let base = 16 + self.ranuras_con_nombre * 8;    // primer variadico
            // lea rdx, [rbp + base]
            self.code.extend_from_slice(&[0x48, 0x8D, 0x95]);
            self.code.extend_from_slice(&base.to_le_bytes());
            // mov rax, [rdx + rax*8]
            self.code.extend_from_slice(&[0x48, 0x8B, 0x04, 0xC2]);
            return;
        }
        let Some(def) = self.intrinsics.get(name) else {
            self.errors.push(format!(
                "intrinsic __{name}() no existe en la tabla sem-asm (tables/arch/x86_64/intrinsics.toml)"));
            return;
        };
        if args.len() != def.args.len() {
            self.errors.push(format!(
                "intrinsic __{name}() espera {} argumento(s), recibio {}",
                def.args.len(), args.len()));
            return;
        }
        let bytes = def.bytes.clone();
        let arg_regs = def.args.clone();
        let returns = def.returns.clone();

        // 1) evaluar cada argumento a rax y apilarlo (orden de aparicion)
        for a in args {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }
        // 2) volcar a los registros destino, en REVERSA (el tope es el ultimo
        //    arg). Cada destino es un registro DISTINTO (rax/rcx/rdx) -> pop
        //    directo sin pisarse.
        for reg in arg_regs.iter().rev() {
            self.emit_pop_to_reg(reg);
        }
        // 3) los bytes exactos de la instruccion
        self.code.extend_from_slice(&bytes);
        // 4) normalizar el valor de retorno a rax
        self.emit_intrinsic_return(returns.as_deref());
    }

    /// Saca el tope de la pila al registro destino de un argumento.
    ///
    /// Los nombres de 64 bits (`rdi`, `rsi`, `r10`, `r8`) no estaban, y esa
    /// ausencia era justo la que dejaba `syscall` fuera del lenguaje: la
    /// convencion de la puerta congelada pasa los argumentos por ahi, asi que
    /// sin estos registros no habia forma de escribir la llamada en C. Solo
    /// existian los de los puertos de E/S (`dx`, `al`) y los de `rdmsr`.
    fn emit_pop_to_reg(&mut self, reg: &str) {
        match reg {
            "rax" | "eax" | "ax" | "al" => self.code.push(0x58),  // pop rax
            "rcx" | "ecx" | "cx" | "cl" => self.code.push(0x59),  // pop rcx
            "rdx" | "edx" | "dx"        => self.code.push(0x5A),  // pop rdx
            "rbx" => self.code.push(0x5B),
            "rsi" | "esi" | "si" => self.code.push(0x5E),
            "rdi" | "edi" | "di" => self.code.push(0x5F),
            // r8..r11 llevan REX.B: el `pop` corto solo alcanza los ocho
            // registros clasicos.
            "r8"  => self.code.extend_from_slice(&[0x41, 0x58]),
            "r9"  => self.code.extend_from_slice(&[0x41, 0x59]),
            "r10" => self.code.extend_from_slice(&[0x41, 0x5A]),
            "r11" => self.code.extend_from_slice(&[0x41, 0x5B]),
            "u64_edx_eax" => {
                // valor de 64 bits en rax -> edx:eax (para wrmsr)
                self.code.push(0x58);                              // pop rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                self.code.extend_from_slice(&[0x48, 0xC1, 0xEA, 0x20]); // shr rdx, 32
            }
            _ => self.errors.push(format!("registro de argumento desconocido: {reg}")),
        }
    }

    /// Deja el resultado del intrinseco limpio en rax segun de donde salga.
    fn emit_intrinsic_return(&mut self, returns: Option<&str>) {
        match returns {
            Some("u64_edx_eax") => {
                self.code.extend_from_slice(&[0x48, 0xC1, 0xE2, 0x20]); // shl rdx, 32
                self.code.extend_from_slice(&[0x48, 0x09, 0xD0]);       // or rax, rdx
            }
            Some("al") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx rax, al
            Some("ax") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]), // movzx rax, ax
            // La puerta devuelve DOS cosas: el codigo en rax y el valor en
            // rdx. Quien pide el valor se lleva rdx a rax, que es donde este
            // codegen espera todo resultado.
            Some("rdx") => self.code.extend_from_slice(&[0x48, 0x89, 0xD0]), // mov rax, rdx
            // "eax": escribir eax en modo 64-bit ya deja rax con el alto en cero
            _ => {}
        }
    }

    // =============== Ruta SSE: floats en xmm0 (doble precision) ===============
    // C tradicional oculta si un valor es float; aqui el codegen lo SABE y lo
    // rutea por xmm. Se computa todo en double; `float` (f32) se convierte en
    // los bordes (load/store). Registro de trabajo: xmm0; scratch: xmm1.

    fn var_type_of(&self, name: &str) -> Option<TypeSpec> {
        self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()))
    }

    fn is_float_ty(t: &TypeSpec) -> bool { matches!(t, TypeSpec::Float | TypeSpec::Double) }

    /// Esta expresion produce un valor de punto flotante?
    fn expr_is_float(&self, e: &Expr) -> bool {
        match e {
            Expr::FloatLit(_) => true,
            Expr::Var(n) => self.var_type_of(n).map_or(false, |t| Self::is_float_ty(&t)),
            Expr::Cast(t, _) => Self::is_float_ty(t),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) =>
                self.expr_is_float(a) || self.expr_is_float(b),
            Expr::Neg(a) => self.expr_is_float(a),
            Expr::Field(_, _, _, t) | Expr::Arrow(_, _, _, t) => Self::is_float_ty(t),
            Expr::IndexPtr(_, _, t) => Self::is_float_ty(t),
            Expr::Conditional(_, a, b) => self.expr_is_float(a) || self.expr_is_float(b),
            _ => false,
        }
    }

    /// cvtsi2sd xmm0, rax -- entero (rax) -> double (xmm0).
    fn emit_int_to_double(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
    }

    /// modrm+disp para `<sse> xmm0, [rbp+off]` / `[rbp+off], xmm0` (reg field = 0).
    fn emit_rbp_disp(&mut self, off: i32) {
        if off >= -128 && off <= 127 {
            self.code.push(0x45);           // mod=01, reg=0, rm=101 (rbp) + disp8
            self.code.push(off as u8);
        } else {
            self.code.push(0x85);           // mod=10 + disp32
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    /// Carga una variable float/double del stack a xmm0 (siempre como double).
    fn emit_load_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x10]); // movss xmm0,[rbp+off]
                self.emit_rbp_disp(off);
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd xmm0,xmm0
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x10]); // movsd xmm0,[rbp+off]
                self.emit_rbp_disp(off);
            }
        } else {
            // global float: pendiente (locales primero) -> xmm0 = 0
            self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC0]); // xorpd xmm0,xmm0
            self.errors.push(format!("variable float global '{name}' aun no soportada (usa locales)"));
        }
    }

    /// Guarda xmm0 (double) en una variable float/double del stack.
    fn store_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0,xmm0
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x11]);       // movss [rbp+off],xmm0
                self.emit_rbp_disp(off);
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x11]);       // movsd [rbp+off],xmm0
                self.emit_rbp_disp(off);
            }
        } else {
            self.errors.push(format!("variable float global '{name}' aun no soportada (usa locales)"));
        }
    }

    /// Evalua `e` a xmm0 como double, convirtiendo enteros si hace falta.
    fn emit_fexpr_operand(&mut self, e: &Expr) {
        if self.expr_is_float(e) {
            self.emit_fexpr(e);
        } else {
            self.emit_expr(e);          // rax = valor entero
            self.emit_int_to_double();  // xmm0 = (double) rax
        }
    }

    /// a OP b en double: resultado en xmm0. `op` = bytes de `<opsd> xmm0,xmm1`.
    fn emit_fbinop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0  (spill a)
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0  (xmm1=b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (xmm0=a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(op);                             // op xmm0,xmm1
    }

    /// Evalua una expresion FLOTANTE dejando el resultado (double) en xmm0.
    fn emit_fexpr(&mut self, e: &Expr) {
        match e {
            Expr::FloatLit(f) => {
                let bits = f.to_bits();
                self.code.extend_from_slice(&[0x48, 0xB8]);            // mov rax, imm64
                self.code.extend_from_slice(&bits.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
            }
            Expr::Var(n) => self.emit_load_float_var(n),
            Expr::Cast(t, inner) if Self::is_float_ty(t) => {
                // (double)algo -- si algo ya es float, no-op; si es entero, convierte
                self.emit_fexpr_operand(inner);
            }
            Expr::Neg(a) => {
                self.emit_fexpr(a);
                // xorpd xmm0, sign-bit -> negacion
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&0x8000_0000_0000_0000u64.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
                self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC1]);       // xorpd xmm0, xmm1
            }
            Expr::Add(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x58, 0xC1]), // addsd
            Expr::Sub(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5C, 0xC1]), // subsd
            Expr::Mul(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x59, 0xC1]), // mulsd
            Expr::Div(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5E, 0xC1]), // divsd
            // cualquier otra cosa: es entera -> convertir a double
            _ => self.emit_fexpr_operand(e),
        }
    }

    /// Comparacion de floats: a CMP b -> 0/1 en rax. `setcc` es el opcode
    /// SETcc estilo UNSIGNED (comisd fija CF/ZF como comparacion sin signo).
    fn emit_fcmp(&mut self, a: &Expr, b: &Expr, setcc: u8) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0 (b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(&[0x66, 0x0F, 0x2F, 0xC1]);       // comisd xmm0,xmm1
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]);            // setcc al
        self.code.extend_from_slice(&[0x0F, 0xB6, 0xC0]);            // movzx eax, al
    }

    /// Emit expression as an address (pointer), not as a value
    fn emit_expr_as_ptr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => {
                if let Some(&(offset, _)) = self.var_offsets.get(name) {
                    if offset >= -128 && offset <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                        self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                    }
                } else if self.global_offsets.contains_key(name) {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                    self.global_fixups.push((self.code.len() - 4, name.clone()));
                } else { self.emit_xor_eax(); }
            }
            Expr::Subscript(name, index, scale) => {
                self.emit_subscript_addr(name, index, *scale);
            }
            Expr::IndexPtr(base, index, elem) => {
                self.emit_index_ptr_addr(base, index, elem);
            }
            _ => self.emit_expr(expr),
        }
    }

    /// Tipo al que apunta una expresion de direccion, si se puede deducir.
    ///
    /// Cubre lo que aparece en la practica: una variable puntero o array,
    /// aritmetica de punteros (`p + 1`), y un cast explicito. Cuando no se
    /// puede deducir se devuelve `None` y el `deref` lee 8 bytes, que es el
    /// comportamiento anterior.
    fn pointee_type(&self, expr: &Expr) -> Option<TypeSpec> {
        match expr {
            Expr::Var(name) => match self.var_type_of(name) {
                Some(TypeSpec::Ptr(inner)) | Some(TypeSpec::Array(inner, _)) => Some(*inner),
                _ => None,
            },
            Expr::Cast(TypeSpec::Ptr(inner), _) => Some((**inner).clone()),
            Expr::Add(a, b) | Expr::Sub(a, b) => {
                self.pointee_type(a).or_else(|| self.pointee_type(b))
            }
            _ => None,
        }
    }

    /// Cuantos bytes avanza `+1` sobre esta expresion, si es un puntero.
    /// `None` cuando no lo es o cuando el elemento mide 1 byte (no hace
    /// falta escalar).
    fn pointer_scale(&self, expr: &Expr) -> Option<u32> {
        let size = self.pointee_type(expr)?.stack_size();
        if size > 1 { Some(size) } else { None }
    }

    /// Convierte `rax` en 0 o 1, que es lo que valen `&&`, `||` y `!`.
    fn emit_normalize_bool(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x85, 0xC0]);       // test rax, rax
        self.code.extend_from_slice(&[0x0F, 0x95, 0xC0]);       // setne al
        self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
    }

    fn emit_binop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_expr(a);
        self.code.push(0x50);
        self.emit_expr(b);
        self.code.push(0x5A);
        self.code.extend_from_slice(op);
    }

    /// Comparacion entera `a <op> b` -> 0 o 1 en `rax`.
    ///
    /// `setcc` es el segundo byte del opcode: `0x94`=sete, `0x95`=setne,
    /// `0x9C`=setl, `0x9D`=setge, `0x9E`=setle, `0x9F`=setg.
    ///
    /// El `movzx` del final NO es decorativo: `setcc` solo escribe `al`, asi
    /// que sin el los 56 bits altos de `rax` conservan el valor del operando
    /// derecho. Con operandos chicos el resultado parecia correcto de puro
    /// milagro; `printf("%d", x == y)` con una `x` grande imprimia basura.
    fn emit_cmp(&mut self, a: &Expr, b: &Expr, setcc: u8) {
        self.emit_expr(a);
        self.code.push(0x50); // push rax (izquierdo)
        self.emit_expr(b); // rax = derecho
        self.code.push(0x5A); // pop rdx (izquierdo)
        self.code.extend_from_slice(&[0x48, 0x39, 0xC2]); // cmp rdx, rax -> a - b
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]); // setcc al
        self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
    }

    fn emit_mov_eax_syscall(&mut self, nr: u32) {
        self.code.extend_from_slice(&[0xB8]);
        self.code.extend_from_slice(&nr.to_le_bytes());
        self.emit_call_to_syscall_stub();
    }

    /// La puerta del kernel desde el codigo emitido.
    ///
    /// === * Por que `Ring0Kernel` NO compila, y lo que emitia ===
    ///
    /// Emitia `0F 05 C3` -- `syscall; ret` -- con el comentario *"los mismos 3
    /// bytes, sin relocacion"*. Y era falso de una forma que no se ve leyendo:
    /// el stub de Ring 3 es un **llamable** (`syscall; ret` al que se llega con
    /// un `call`, y el `ret` devuelve al llamante). Poniendolo en linea se
    /// quita el `call` y **se queda el `ret`**: la funcion entera retorna en
    /// cuanto vuelve el syscall, y todo lo que hubiera detras no se ejecuta.
    ///
    /// Y hay una segunda razon, mas de fondo: **`syscall` desde Ring 0 no tiene
    /// sentido**. Carga CS y SS de `IA32_STAR` y salta a `LSTAR`; desde CPL0 eso
    /// es reentrar en el manejador del kernel con la pila del kernel. Codigo de
    /// Ring 0 no pide servicios -- los llama.
    ///
    /// No lo cazo nadie porque **nadie construye este perfil**: es alcanzable
    /// solo por `compile_with_target`, y en todo el arbol nada lo pasa. Un
    /// camino muerto que emite bytes incorrectos es peor que uno que no existe,
    /// porque el dia que alguien lo use el fallo no se parecera a su causa.
    fn emit_call_to_syscall_stub(&mut self) {
        if self.target == TargetProfile::Ring0Kernel {
            self.errors.push(
                "no se compila C para Ring 0: `syscall` desde CPL0 reentra en el \
                 manejador del kernel, y el codigo de Ring 0 no pide servicios, \
                 los llama. Si algun dia hace falta, lo que toca es emitir la \
                 LLAMADA DIRECTA a la funcion del kernel, no la puerta"
                    .to_string(),
            );
        } else {
            // Ring 3: call __bmo_syscall_stub via E8 rel32
            self.code.extend_from_slice(&[0xE8]);
            self.call_relocs.push(CallReloc { offset: self.code.len(), target: "__bmo_syscall_stub".to_string() });
            self.code.extend_from_slice(&[0, 0, 0, 0]);
        }
    }

    fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();

        let code_bytes = &all[..self.instruction_end];
        let rodata_bytes = &all[self.instruction_end..self.string_data_end];
        let data_bytes = &all[self.string_data_end..];

        let mut code_sec = BefSection::code(code_bytes.to_vec());
        code_sec.alignment = 4096;
        b.add_section(code_sec);

        if !rodata_bytes.is_empty() {
            let mut rodata_sec = BefSection::rodata(rodata_bytes.to_vec());
            rodata_sec.alignment = 4096;
            b.add_section(rodata_sec);
        }

        if !data_bytes.is_empty() {
            let mut data_sec = BefSection::data(data_bytes.to_vec());
            data_sec.alignment = 4096;
            b.add_section(data_sec);
        }

        // * LA SECCION `Relocs`, y va DESPUES de las tres cargables a proposito:
        // sus offsets son relativos a `.data` y `.rodata`, o sea que se refiere
        // a las que ya estan puestas. Y no es cargable --`is_loadable` la
        // excluye-- asi que no ocupa una pagina en el proceso: el cargador la
        // lee del fichero, aplica lo que dice y la olvida.
        //
        // Solo se emite si hay alguna. Un `.bex` sin punteros en datos no lleva
        // seccion de relocs, igual que uno sin syscalls dejo de llevar el stub.
        if !self.relocs.is_empty() {
            let relocs = core::mem::take(&mut self.relocs);
            b.add_section(BefSection::relocs(relocs));
        }

        // * La bandera de la pantalla, deducida al recorrer el programa. Ver
        // `BefFlags::WANTS_SCREEN`: la pone el compilador y no el autor para que
        // diga lo que el programa HACE y no lo que promete.
        if self.quiere_pantalla {
            b.header.flags |= bmo_abi::bef::header::BefFlags::WANTS_SCREEN.bits();
        }

        b.entry_offset = self.entry_offset as u64;
        b.build().unwrap_or_default()
    }
}
