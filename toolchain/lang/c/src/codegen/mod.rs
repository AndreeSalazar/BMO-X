use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::bef::relocations::{Relocation, SEC_CODE, SEC_DATA, SEC_RODATA};
use crate::ast::*;
use crate::CError;

/// Structs y uniones POR VALOR, en su propio fichero. Ver su cabecera para
/// la ABI de agregados de BMO y para que hacen SysV y Win64 con esto mismo.
mod agregados;
/// INTERNAL LINKING: jumps, calls and function addresses. Everything emitted
/// as a hole and filled in once the distance is known.
mod linking;
/// THE STACK FRAME: where each local falls and how it is read back by width.
mod frame;
/// `printf`, the only part that emits an INTERPRETER -- which is why it carries
/// the formatter written twice, in Rust and in machine code.
mod format;
/// INDEXING AND POINTERS: the five ways of reaching an element are one sum, and
/// the STRIDE is the number that has failed more often than any other here.
mod indexing;
use indexing::Por;
/// INTRINSICS AND THE DOOR: the system surface as seen from C.
mod intrinsics;
/// THE QUESTIONS YOU ASK A TYPE: floating point and signedness, written as
/// carbon copies on purpose. The third axis that shows up goes here.
mod types;
/// FLOATING POINT: the only value that does not travel in `rax`.
mod floats;
/// La ENTRADA de C (`getchar`, `scanf`), tambien aparte. Escribir es empujar
/// bytes; leer es ESPERAR, guardar lo que sobra y decidir que significa lo que
/// alguien tecleo. Tres problemas que la salida no tiene.
mod entrada;
/// LA DISPOSICION: donde cae cada campo de un agregado, cuanto mide el
/// conjunto, y **el cotejo** contra lo que dijo el frontend. Salio de aqui
/// porque colocar y comprobar la colocacion son el mismo concepto.
mod disposicion;
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

/// **EL MAPA: que funcion vive en cada offset del codigo.**
///
/// === Por que esto existe, y con la fecha ===
///
/// El 2026-08-13 DOOM murio en el Ryzen con
///
/// ```text
///   causa     #GP proteccion general  (vector 13)
///   rip       0x400815f2   la instruccion que fallo
/// ```
///
/// Un `rip` es un dato exacto y no servia para nada: el `.bex` no lleva
/// simbolos, asi que la unica forma de saber que funcion es esa era **adivinar
/// leyendo el fuente** -- que es justo lo que esta casa lleva todo el dia
/// evitando. La informacion existia (`function_offsets`), sencillamente no
/// salia del compilador.
///
/// Ahora sale. Con la base de carga (`0x40000000` para un `.bex` de Ring 3) y el
/// `rip` de la autopsia, la pregunta *"que revento"* se contesta restando.
///
/// [!] Devuelve los offsets DENTRO de la seccion de codigo, no direcciones
/// virtuales: el compilador no decide donde se carga -- eso lo hace el cargador,
/// y ponerlo aqui seria repetir una decision que no es suya.
pub fn function_map(program: &Program) -> Result<Vec<(usize, String)>> {
    let mut cg = Codegen::new(TargetProfile::default());
    cg.emit_program(program)?;
    let mut v: Vec<(usize, String)> = cg
        .function_offsets
        .iter()
        .map(|(n, off)| (*off, n.clone()))
        .collect();
    v.sort();
    Ok(v)
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
    /// Un aviso de un solo uso para [`Self::emit_expr`]: **no metas el guarda
    /// SSE en esta expresion**. Lo pone `emit_fexpr` cuando quiere emitir una
    /// LLAMADA que devuelve un double -- si no, el guarda la mandaria otra vez
    /// a `emit_fexpr` y las dos se llamarian para siempre.
    ///
    /// Se consume en la primera comprobacion (`mem::take`), asi que los
    /// argumentos que se emitan dentro de esa llamada vuelven a tener su guarda.
    sin_guarda_float: bool,
    /// Ranuras que ocupan sus parametros CON NOMBRE. Justo detras empiezan los
    /// variadicos, porque los argumentos van seguidos en la pila.
    ranuras_con_nombre: i32,
    struct_layouts: HashMap<String, Vec<(String, u32, u32)>>,
    struct_sizes: HashMap<String, u32>,
    /// El alineado de cada agregado, que **no** se puede recalcular desde su
    /// tamano: `char[8]` y `long` miden lo mismo y no se alinean igual. Se
    /// guarda al colocarlo y se consulta cuando ese agregado es a su vez el
    /// miembro de otro.
    struct_aligns: HashMap<String, u32>,
    /// `(agregado, campo)` -> tipo del campo.
    ///
    /// ** Entro el 2026-09-02, al vaciar `Expr::Field` y `Expr::Arrow`. Antes
    /// el tipo del campo llegaba DENTRO del nodo, o sea que el codegen se fiaba
    /// de lo que el frontend hubiera resuelto. Ahora lo calcula el, con los
    /// mismos `members` con los que coloca -- y `cotejar_disposicion` comprueba
    /// que las dos colocaciones digan lo mismo.
    field_types: HashMap<(String, String), TypeSpec>,
    label_positions: HashMap<String, usize>,
    goto_relocs: Vec<(usize, String)>,
    entry_offset: usize,
    is_entry_function: bool,
    /// Nombre -> `(offset en el espacio de globales, tipo)`.
    ///
    /// * El offset es de un espacio UNICO que abarca `.data` y `.bss`: por
    /// debajo de `global_data.len()` el global vive en `.data`, por encima en
    /// `.bss`. Ver [`Self::separar_bss`].
    global_offsets: HashMap<String, (u32, TypeSpec)>,
    global_data: Vec<u8>,
    /// Cuantos bytes de globales son TODO CEROS y por tanto no viajan en el
    /// fichero. Los reserva el cargador y los entrega a cero. Ver
    /// [`Self::separar_bss`].
    bss_len: usize,
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
    /// Igual, pero el destino es OTRO GLOBAL: `(offset, nombre, sumando)`.
    ///
    /// `&key_right` dentro de `doom_defaults`, `&finesine[FINEANGLES/4]` en
    /// `tables.c`. Tambien se difiere, y por un motivo que las cadenas no
    /// tienen: la tabla puede nombrar un global **declarado mas abajo**, asi
    /// que su offset todavia no existe cuando se lee la lista.
    relocs_a_global: Vec<(u32, String, i64)>,
    /// Igual, pero el destino es una FUNCION: `(offset en .data, nombre)`.
    ///
    /// Es la tabla de punteros a funcion -- el campo `action` de cada `state_t`
    /// de DOOM, y con el las mil y pico filas de `info.c`. No se puede resolver
    /// donde se lee, porque ahi todavia no se sabe en que offset va a caer cada
    /// funcion: se anota y se cierra en `patch_all_fixups`, igual que las
    /// cadenas y por el mismo motivo.
    relocs_a_funcion: Vec<(u32, String)>,
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
            sin_guarda_float: false,
            ranuras_con_nombre: 0,
            struct_layouts: HashMap::new(), struct_sizes: HashMap::new(),
            field_types: HashMap::new(),
            struct_aligns: HashMap::new(),
            label_positions: HashMap::new(), goto_relocs: Vec::new(),
            entry_offset: 0, is_entry_function: false,
            global_offsets: HashMap::new(), global_data: Vec::new(),
            bss_len: 0,
            global_fixups: Vec::new(),
            relocs_a_cadena: Vec::new(),
            relocs_a_funcion: Vec::new(),
            relocs_a_global: Vec::new(),
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
        self.cotejar_disposicion(program)?;
        // Las funciones que ESTA unidad define. Se necesita antes de leer los
        // globales: un nombre suelto en una tabla es la direccion de una
        // funcion solo si la funcion existe aqui -- si solo hay un prototipo,
        // su direccion vive en otra unidad y eso pide un enlazador.
        let funciones: std::collections::HashSet<&str> =
            program.functions.iter().map(|f| f.name.as_str()).collect();

        // Y los globales que son ARRAY. Un array nombrado a secas DECAE a su
        // direccion (`int *p = tabla;`), y sin esta lista no habria como
        // distinguirlo de leer el valor de un escalar. Se saca de todo el
        // programa antes de empezar porque la tabla puede nombrar un array
        // declarado mas abajo.
        let arrays: std::collections::HashSet<&str> = program
            .globals
            .iter()
            .filter_map(|g| match g {
                GlobalDecl::Var(TypeSpec::Array(_, _), n, _)
                | GlobalDecl::VarLista(TypeSpec::Array(_, _), n, _) => Some(n.as_str()),
                _ => None,
            })
            .collect();

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
                    // ** UNA FUNCION en la tabla -> RELOCATION A `.code`.
                    //
                    // Es el hueco que este mismo sitio dejaba anotado: el campo
                    // `action` de cada `state_t` de DOOM, y con el las mil y
                    // pico filas de `info.c` -- o sea el comportamiento entero
                    // de todos los monstruos del juego.
                    //
                    // El offset de la funcion NO se sabe aqui, porque las
                    // funciones se emiten despues. Asi que se anota el nombre y
                    // se cierra en `patch_all_fixups`, exactamente como las
                    // cadenas. El mecanismo ya existia; lo que faltaba era la
                    // otra seccion de destino.
                    if let Expr::Var(fname) = &e.valor {
                        if funciones.contains(fname.as_str()) {
                            self.relocs_a_funcion.push((off + e.offset, fname.clone()));
                            continue;
                        }
                    }
                    // ** LA DIRECCION DE OTRO GLOBAL en la tabla.
                    //
                    // `doom_defaults[]` de `m_config.c` es una tabla de
                    // `{ "nombre", &la_variable, tipo }`: la configuracion
                    // entera del juego es una lista de PUNTEROS A GLOBALES. Y
                    // `tables.c` escribe `finecosine = &finesine[FINEANGLES/4]`,
                    // que es la misma tabla mirada un cuarto de vuelta despues.
                    //
                    // Es la tercera cara de la misma relocation, ahora con
                    // destino `.data`. El sumando lleva el indice ya
                    // multiplicado por el tamano del elemento.
                    if let Some((gname, sumando)) = self.direccion_de_global(&e.valor) {
                        self.relocs_a_global.push((off + e.offset, gname, sumando));
                        continue;
                    }
                    // Un ARRAY nombrado a secas es su direccion: `int *p = t;`
                    // vale lo mismo que `&t[0]`. Es la regla de decaimiento de
                    // C, y es como se escribe la mitad de las tablas de tablas.
                    if let Expr::Var(gname) = &e.valor {
                        if arrays.contains(gname.as_str()) {
                            self.relocs_a_global.push((off + e.offset, gname.clone(), 0));
                            continue;
                        }
                    }
                    let Some(valor) = Self::constante_de(&e.valor) else {
                        self.errors.push(format!(
                            "en la tabla global '{name}', el valor del offset {} no es una \
                             constante entera, ni una cadena, ni una funcion de esta unidad",
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
                    // Las mismas tres direcciones que en una tabla, y por el
                    // mismo motivo: un puntero global inicializado es una
                    // relocation, no una constante. `finecosine` de `tables.c`
                    // es exactamente esto -- y sin ello el coseno del juego
                    // entero apuntaria a la nada.
                    (Some(otro), _)
                        if self.direccion_de_global(otro).is_some()
                            || matches!(otro, Expr::Var(n)
                                if arrays.contains(n.as_str()) || funciones.contains(n.as_str())) =>
                    {
                        match otro {
                            Expr::Var(n) if funciones.contains(n.as_str()) => {
                                self.relocs_a_funcion.push((off, n.clone()));
                            }
                            Expr::Var(n) => {
                                self.relocs_a_global.push((off, n.clone(), 0));
                            }
                            _ => {
                                let (gname, sumando) = self.direccion_de_global(otro).unwrap();
                                self.relocs_a_global.push((off, gname, sumando));
                            }
                        }
                        for _ in 0..size { self.global_data.push(0); }
                    }
                    // * UN GLOBAL DE COMA FLOTANTE: se guarda su patron IEEE.
                    //
                    // `float mouse_acceleration = 2.0;` (i_video.c). Antes esto
                    // era un error, y el motivo escrito era que no se sabia
                    // convertir -- pero convertir es exactamente lo que sabe
                    // hacer `to_bits`: un `float` son los cuatro bytes de su
                    // representacion y un `double` los ocho.
                    //
                    // La anchura la manda el TIPO DECLARADO, no el literal: un
                    // `2.0` en un `float` son cuatro bytes distintos de los que
                    // ocuparia en un `double`, y escribir los ocho ahi seria
                    // pisar el global de al lado.
                    (Some(Expr::FloatLit(f)), _) => {
                        let bytes: Vec<u8> = if size == 4 {
                            (*f as f32).to_bits().to_le_bytes().to_vec()
                        } else {
                            f.to_bits().to_le_bytes().to_vec()
                        };
                        for k in 0..size {
                            self.global_data.push(*bytes.get(k as usize).unwrap_or(&0));
                        }
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
        // * Los globales ya estan colocados: ahora se separan los que son todo
        // ceros. Tiene que ir aqui -- despues del bucle, porque hace falta ver
        // los bytes ya escritos, y antes de emitir funciones, porque a partir de
        // aqui `global_offsets` es lo que resuelve cada `lea`.
        self.separar_bss();
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
            Expr::Arrow(p,_) => self.collect_expr_strings(p),
            Expr::AssignArrow(p,_,v) => { self.collect_expr_strings(p); self.collect_expr_strings(v); }
            Expr::Assign(_, v) | Expr::AssignField(_,_,v) => self.collect_expr_strings(v),
            Expr::Cast(_, a) => self.collect_expr_strings(a),
            Expr::Intrinsic(_, args) => { for a in args { self.collect_expr_strings(a); } }
            Expr::IndexPtr(b, idx) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); }
            Expr::AssignIndexPtr(b, idx, v) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); self.collect_expr_strings(v); }
            Expr::CallPtr(c, args) => { self.collect_expr_strings(c); for a in args { self.collect_expr_strings(a); } }
            Expr::AssignDeref(a, v) => { self.collect_expr_strings(a); self.collect_expr_strings(v); }
            Expr::Field(b,_) => self.collect_expr_strings(b),
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
    /// `&global` o `&global[n]` -> `(nombre, sumando en bytes)`.
    ///
    /// Devuelve `None` para cualquier otra cosa, incluido `&local`: la
    /// direccion de una local no existe hasta que hay una pila, asi que en un
    /// inicializador global no puede aparecer.
    ///
    /// El indice tiene que ser constante, y si no lo es se contesta `None` para
    /// que el error salga arriba con el nombre de la tabla delante.
    fn direccion_de_global(&self, e: &Expr) -> Option<(String, i64)> {
        let Expr::AddrOf(interior) = e else { return None };
        match interior.as_ref() {
            Expr::Var(n) => Some((n.clone(), 0)),
            Expr::Subscript(n, idx) => {
                let i = Self::constante_de(idx)?;
                Some((n.clone(), i * (self.paso_de_elemento(n) as i64)))
            }
            _ => None,
        }
    }

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
            // * Lo que faltaba, y cada linea es una tabla de DOOM.
            //
            //   'M'        `midiheader[] = {'M','T','h','d', ...}`  (mus2mid.c)
            //   1 << 16    `FRACUNIT`, o sea la unidad de TODA la aritmetica
            //              del juego: `xspeed[] = {FRACUNIT, 47000, ...}`
            //   ~ | & ^    mascaras de banderas en las tablas de estados
            //
            // El evaluador se habia quedado en las cuatro operaciones de la
            // aritmetica, y una tabla que no se puede plegar no da un valor
            // malo: **da un error y el fichero entero no compila**.
            Expr::CharLit(c) => Some(*c as i64),
            Expr::Mod(a, b) => Self::constante_de(a)?.checked_rem(Self::constante_de(b)?),
            Expr::Shl(a, b) => {
                Some(Self::constante_de(a)?.wrapping_shl(Self::constante_de(b)? as u32))
            }
            Expr::Shr(a, b) => {
                Some(Self::constante_de(a)?.wrapping_shr(Self::constante_de(b)? as u32))
            }
            Expr::BitAnd(a, b) => Some(Self::constante_de(a)? & Self::constante_de(b)?),
            Expr::BitOr(a, b) => Some(Self::constante_de(a)? | Self::constante_de(b)?),
            Expr::BitXor(a, b) => Some(Self::constante_de(a)? ^ Self::constante_de(b)?),
            Expr::BitNot(a) => Some(!Self::constante_de(a)?),
            Expr::Not(a) => Some((Self::constante_de(a)? == 0) as i64),
            // Un cast no cambia el VALOR de una constante entera, solo su
            // anchura -- y la anchura la pone el subobjeto al escribirlo.
            //
            // * Y si dentro hay COMA FLOTANTE, se pliega y se trunca.
            //
            // `(fixed_t)(-.867*FRACUNIT)` -- asi escribe `am_map.c` las flechas
            // del mapa, y es la forma normal de meter un numero real en punto
            // fijo: se calcula en flotante **al compilar** y lo que se guarda
            // es un entero. El programa no lleva un solo `float` dentro.
            //
            // Truncar hacia cero es lo que dice C de una conversion de
            // flotante a entero, y por eso se hace con `as i64` y no
            // redondeando: redondear daria otro numero, y el numero es el dato.
            Expr::Cast(t, a) => Self::constante_de(a).or_else(|| {
                if matches!(t, TypeSpec::Float | TypeSpec::Double) {
                    return None;
                }
                Self::constante_flotante(a).map(|f| f as i64)
            }),
            _ => None,
        }
    }

    /// Pliega una expresion constante que tiene coma flotante dentro.
    ///
    /// Solo se usa cuando el resultado va a un ENTERO: mientras BMO C no tenga
    /// la ruta SSE en los datos, un global que se quede en flotante sigue
    /// diciendo que no puede. Aqui el flotante es una forma de ESCRIBIR el
    /// numero, no de guardarlo.
    fn constante_flotante(e: &Expr) -> Option<f64> {
        // Lo que ya se pliega como entero, se pliega como entero: asi el
        // desplazamiento, las mascaras y el resto de operaciones que solo
        // existen sobre enteros no hay que escribirlas dos veces. `FRACUNIT`
        // es `(1<<16)`, y sin esta linea el `-.867*FRACUNIT` de DOOM no
        // llegaba a plegarse por culpa del desplazamiento.
        if let Some(n) = Self::constante_de(e) {
            return Some(n as f64);
        }
        Some(match e {
            Expr::FloatLit(f) => *f,
            Expr::Int(n) => *n as f64,
            Expr::CharLit(c) => *c as f64,
            Expr::Neg(a) => -Self::constante_flotante(a)?,
            Expr::Add(a, b) => Self::constante_flotante(a)? + Self::constante_flotante(b)?,
            Expr::Sub(a, b) => Self::constante_flotante(a)? - Self::constante_flotante(b)?,
            Expr::Mul(a, b) => Self::constante_flotante(a)? * Self::constante_flotante(b)?,
            Expr::Div(a, b) => {
                let d = Self::constante_flotante(b)?;
                if d == 0.0 {
                    return None;
                }
                Self::constante_flotante(a)? / d
            }
            Expr::Cast(_, a) => Self::constante_flotante(a)?,
            _ => return None,
        })
    }

    /// Redondea hacia arriba al multiplo de pagina. La cuenta del cargador.
    fn hasta_pagina(n: usize) -> usize {
        const PAGE: usize = 4096;
        (n + PAGE - 1) & !(PAGE - 1)
    }

    /// Cual de las regiones contiene el offset `off`. Las regiones vienen
    /// ordenadas por offset y son contiguas, asi que la busqueda binaria cae en
    /// la que empieza en `off` o en la inmediatamente anterior.
    fn region_de(regiones: &[(u32, u32, String)], off: u32) -> Option<usize> {
        match regiones.binary_search_by(|r| r.0.cmp(&off)) {
            Ok(i) => Some(i),
            Err(0) => None,
            Err(i) => Some(i - 1),
        }
    }

    /// * LOS CEROS NO SE GUARDAN: SE DECLARAN. La seccion `Bss`.
    ///
    /// === El numero que obligo a escribir esto ===
    ///
    /// De los **645.008 bytes** de la seccion `data` de DOOM, **582.291 eran
    /// cero**: el 90,3% de la seccion y el **44,8% del `.bex` entero**. Casi la
    /// mitad del fichero eran ceros que se guardaban en el disco, se leian del
    /// disco, se copiaban al bufer de rebote del kernel y se copiaban otra vez
    /// al espacio del proceso. Cuatro veces pagado un byte cuyo valor ya se
    /// sabia al compilar.
    ///
    /// El motivo era de una linea: este codegen metia TODOS los globales en
    /// `.data`, con o sin inicializador. La maquinaria para no hacerlo ya
    /// estaba entera y sin estrenar -- `BefBuilder::bss()` existe, el escritor
    /// ya salta las `Bss` al colocar y al volcar, y `proc.rs` ya reserva las
    /// paginas y las pone a cero (`bex.rs` acepta `file_size == 0` **solo** si
    /// la seccion es `Bss`). Faltaba quien lo pidiera.
    ///
    /// Ver `docs/identidad/LA_RAM.md`: es el escalon 0 del modelo quirofano, y va primero
    /// porque encoge todo lo demas ANTES de optimizar como se transporta.
    ///
    /// === Como se decide, y son TRES motivos para quedarse ===
    ///
    /// Un global se va a `.bss` solo si no le aplica ninguno:
    ///
    /// 1. **Sus bytes no son todos cero.** El caso obvio.
    /// 2. **El cargador ESCRIBE dentro de el** -- una relocation lo tiene como
    ///    destino de escritura. `char *p = "x"` guarda ceros en el fichero y
    ///    parece un candidato perfecto, pero su valor de verdad lo pone el
    ///    cargador: mandarlo a `.bss` seria mandar la reloc a una seccion que su
    ///    codigo de `donde` no sabe nombrar.
    /// 3. ** **Alguien apunta a el.** Esta es la que no es obvia. El codigo de
    ///    seccion de una relocation solo distingue `code`/`data`/`rodata` -- no
    ///    hay valor para `bss`. Asi que un global a cero cuya DIRECCION se
    ///    guarda en otro global (`&contador` dentro de una tabla, que en DOOM es
    ///    `doom_defaults[]` entero) tiene que quedarse donde la reloc lo sepa
    ///    nombrar. Ampliar el codigo de seccion se puede, pero toca el formato
    ///    Y el cargador del kernel, y eso es otra tanda.
    ///
    /// === Por que el espacio de offsets sigue siendo UNO ===
    ///
    /// Los anclados se colocan primero y los demas detras, en el mismo espacio
    /// de offsets. Asi `global_offsets` no necesita decir en que seccion vive
    /// cada global: se deduce de si su offset pasa de `global_data.len()`. La
    /// unica consecuencia es que `patch_all_fixups` calcula la VA con dos
    /// bases, y todo lo demas del compilador sigue sin enterarse.
    ///
    /// Las regiones incluyen el relleno de alineacion del global siguiente
    /// --que son ceros y no cambia ningun veredicto-- y por eso miden todas un
    /// multiplo de 8: el reparto nuevo sale alineado sin recalcular nada.
    fn separar_bss(&mut self) {
        if self.global_data.is_empty() {
            return;
        }

        let mut regiones: Vec<(u32, u32, String)> = self
            .global_offsets
            .iter()
            .map(|(n, &(off, _))| (off, 0u32, n.clone()))
            .collect();
        regiones.sort_by_key(|r| r.0);
        for i in 0..regiones.len() {
            let fin = regiones
                .get(i + 1)
                .map(|r| r.0)
                .unwrap_or(self.global_data.len() as u32);
            regiones[i].1 = fin.saturating_sub(regiones[i].0);
        }
        // ** FUERA LAS REGIONES DE LONGITUD CERO, y esto no es limpieza: es el
        // bug que el gate del BEF caza si no se hace.
        //
        // `type_stack_size` devuelve 0 para un tipo cuyo tamano no conoce, asi
        // que dos globales pueden acabar EN EL MISMO OFFSET. Con eso, el mapa
        // de traduccion --que se indexa por offset-- tiene dos duenos para la
        // misma clave: si uno esta anclado y el otro no, el ultimo en escribir
        // gana y **una reloc acaba apuntando dentro de `.bss`**, que es una
        // seccion que su codigo de `donde` no sabe nombrar.
        //
        // Se cayo asi de verdad al compilar DOOM: `reloc[293]: offset 0x614d0
        // exceeds target section size`. Quitarlas es correcto ademas de
        // necesario -- una region vacia no tiene bytes, y cualquier offset que
        // la nombrara cae igual en la region que empieza donde ella acaba.
        regiones.retain(|r| r.1 > 0);
        if regiones.is_empty() {
            return;
        }

        let mut anclado = vec![false; regiones.len()];

        // Motivo 2: el cargador escribe dentro.
        let escrituras = self
            .relocs_a_cadena
            .iter()
            .map(|&(off, _)| off)
            .chain(self.relocs_a_global.iter().map(|&(off, _, _)| off))
            .chain(self.relocs_a_funcion.iter().map(|&(off, _)| off))
            .collect::<Vec<u32>>();
        for off in escrituras {
            if let Some(i) = Self::region_de(&regiones, off) {
                anclado[i] = true;
            }
        }

        // Motivo 3: alguien apunta a el, y una reloc no sabe nombrar `.bss`.
        let destinos = self
            .relocs_a_global
            .iter()
            .filter_map(|(_, gname, _)| self.global_offsets.get(gname).map(|&(off, _)| off))
            .collect::<Vec<u32>>();
        for off in destinos {
            if let Some(i) = Self::region_de(&regiones, off) {
                anclado[i] = true;
            }
        }

        // Motivo 1: tiene algo escrito.
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            let ini = off as usize;
            let fin = (ini + len as usize).min(self.global_data.len());
            if self.global_data[ini..fin].iter().any(|&b| b != 0) {
                anclado[i] = true;
            }
        }

        // El reparto nuevo: anclados primero, en su orden; el resto detras.
        let mut datos: Vec<u8> = Vec::with_capacity(self.global_data.len());
        let mut nuevo_de: HashMap<u32, u32> = HashMap::with_capacity(regiones.len());
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            if !anclado[i] {
                continue;
            }
            nuevo_de.insert(off, datos.len() as u32);
            let ini = off as usize;
            let fin = (ini + len as usize).min(self.global_data.len());
            datos.extend_from_slice(&self.global_data[ini..fin]);
            while datos.len() % 8 != 0 {
                datos.push(0);
            }
        }
        let data_len = datos.len() as u32;
        let mut cursor = data_len;
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            if anclado[i] {
                continue;
            }
            nuevo_de.insert(off, cursor);
            cursor += (len + 7) & !7;
        }

        // Y se traduce todo lo que hablaba de offsets viejos. Un offset puede
        // caer DENTRO de un global (una tabla se parchea por elementos), asi que
        // se traslada su region y se conserva la distancia al principio.
        let traducir = |viejo: u32| -> u32 {
            match Self::region_de(&regiones, viejo) {
                Some(i) => nuevo_de[&regiones[i].0] + (viejo - regiones[i].0),
                None => viejo,
            }
        };
        for v in self.global_offsets.values_mut() {
            v.0 = traducir(v.0);
        }
        for r in self.relocs_a_cadena.iter_mut() {
            r.0 = traducir(r.0);
        }
        for r in self.relocs_a_global.iter_mut() {
            r.0 = traducir(r.0);
        }
        for r in self.relocs_a_funcion.iter_mut() {
            r.0 = traducir(r.0);
        }

        self.global_data = datos;
        self.bss_len = (cursor - data_len) as usize;

        // ** EL GUARDIA, y se queda aunque hoy no salte.
        //
        // La regla entera de este paso cabe en una frase: **una relocation
        // nunca escribe en `.bss`**. Si algun dia un motivo de anclaje se
        // queda corto, el sintoma sin este guardia es un `.bex` que el gate del
        // BEF rechaza con un offset en hexadecimal, o --peor, si el gate no
        // estuviera-- un cargador escribiendo ocho bytes en la pagina de otra
        // cosa. Aqui se dice con el nombre del global delante.
        let fuera: Vec<u32> = self
            .relocs_a_cadena
            .iter()
            .map(|&(off, _)| off)
            .chain(self.relocs_a_global.iter().map(|&(off, _, _)| off))
            .chain(self.relocs_a_funcion.iter().map(|&(off, _)| off))
            .filter(|&off| off >= data_len)
            .collect();
        for off in fuera {
            let quien = self
                .global_offsets
                .iter()
                .find(|(_, &(g, _))| g <= off && off < g + 8)
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| format!("offset {off}"));
            self.errors.push(format!(
                "bug del compilador: una relocation escribe en '{quien}', que quedo en .bss. \
                 Un global al que el cargador escribe tiene que quedarse en .data -- ver \
                 los tres motivos de anclaje en `separar_bss`"
            ));
        }
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
        // * AQUI SE RECHAZABA UN PARAMETRO DE COMA FLOTANTE, y ya no.
        //
        // El motivo escrito era *"la ABI de argumentos xmm esta pendiente"*, y
        // resulto que **BMO no necesita ninguna ABI de xmm**: aqui los
        // argumentos van por la PILA, en ranuras de ocho bytes, y un `double`
        // cabe entero en una. Lo que fallaba no era la convencion, era el sitio
        // de llamada: `emit_expr` de una expresion flotante TRUNCA a entero
        // (`cvttsd2si`), asi que la ranura llevaba `2` donde iba `2.5`.
        //
        // La correccion es empujar el **patron de bits** cuando el parametro es
        // flotante (ver `emit_empuja_flotante`), y el lado del callee ya
        // funcionaba: un parametro vive en `var_offsets` igual que un local, y
        // `emit_load_float_var` lo lee con `movsd [rbp+off]` sin saber si vino
        // de arriba o se declaro dentro.
        //
        // [!] Y la conversion la decide el TIPO DEL PARAMETRO, no la expresion:
        // `fabs(3)` tiene que llegar como `3.0`, y `f(2.5)` a un `float` tiene
        // que estrecharse a cuatro bytes -- si se empujaran los ocho de un
        // double, el callee leeria con `movss` la mitad baja de la mantisa.
        //
        // La asimetria que quedaba escrita aqui --"devolver un double se puede,
        // pasarlo no"-- se acabo: ahora las dos.
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

    // ---- Expression emit ----
    fn emit_expr(&mut self, expr: &Expr) {
        // Guard SSE: una expresion FLOTANTE que llega a la ruta entera esta en
        // contexto entero (int x = 1.5; return d;) -> calcular en xmm y truncar
        // a rax (cvttsd2si). Las comparaciones dan int 0/1 (no son float) y se
        // manejan abajo. emit_fexpr_operand solo llama aqui para NO-floats, asi
        // que no hay recursion infinita.
        let saltar_guarda = core::mem::take(&mut self.sin_guarda_float);
        if !saltar_guarda && self.expr_is_float(expr) {
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
                        // Un parametro de coma flotante viaja como sus BITS.
                        // Sin esto, `emit_expr` truncaria a entero y `fabs(-2.5)`
                        // recibiria `-2`.
                        Some(t) if Self::is_float_ty(t) => {
                            ranuras_total += 1;
                            let estrecho = matches!(t, TypeSpec::Float);
                            self.emit_empuja_flotante(arg, estrecho);
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
            // ** `!x` -- Y AQUI FALTABA EL `movzx`, QUE NO ES DECORATIVO.
            //
            // Era `test eax,eax` + `sete al`, y nada mas. `setcc` **solo escribe
            // `al`**: los 56 bits altos de `rax` se quedan como estaban. Con un
            // operando negativo --`rax = 0xFFFF_FFFF_FFFF_FFFA` para un -6-- el
            // `sete` pone `al = 0` y deja `0xFFFF_FFFF_FFFF_FF00`, o sea
            // **`!(-6)` valia -256**. Que en un `if` es VERDADERO.
            //
            // Lo que eso significa en C de verdad: `if (!strcmp(a, b))` --el
            // idioma mas comun del lenguaje para comparar cadenas-- **acertaba
            // cuando `a` era MENOR que `b`**, porque `strcmp` contesta negativo.
            // En DOOM, `M_CheckParmWithArgs("-config", ...)` casaba con
            // `-iwad` ('c' < 'i'), y por eso el juego anunciaba
            // `saving config in apps/doom1.wad`: iba a escribir su
            // configuracion ENCIMA DEL WAD.
            //
            // [!] La leccion ya estaba aprendida **en este mismo fichero**:
            // `emit_cmp` lleva su `movzx` con un comentario que dice justo esto,
            // *"el movzx del final NO es decorativo"*. Se aprendio en un sitio y
            // no se aplico en el de al lado.
            //
            // Y el `test` pasa a 64 bits (`48 85 C0`) a proposito: `!p` sobre un
            // puntero tiene que mirar el puntero ENTERO. Con `test eax,eax`, una
            // direccion cuyos 32 bits bajos fueran cero se declaraba nula.
            Expr::Not(a) => {
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
                self.code.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
            }
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
            // ** Y la LECTURA usa la misma tabla que la escritura.
            //
            // Aqui habia una TERCERA copia a mano del "carga por ancho", con su
            // propio `match` de siete brazos. Preguntaba bien --el ancho salia
            // correcto-- pero ser una copia es como se llega a que una crezca y
            // la otra no: le faltaban los agregados y el `float`.
            Expr::Deref(a) => {
                let apuntado = self.pointee_type(a).unwrap_or(TypeSpec::Long);
                self.emit_expr(a); // rax = direccion
                self.emit_load_elem(&apuntado);
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
                    Expr::Subscript(name, idx) => {
                        self.emit_subscript_addr(name, idx);
                    }
                    Expr::Deref(ptr) => {
                        self.emit_expr(ptr); // rax = address of the pointed-to data
                    }
                    // ** LAS TRES QUE FALTABAN, y las tres MATARON A DOOM.
                    //
                    // `&c->defaults[i]` es `AddrOf(IndexPtr(..))`, y hasta el
                    // 2026-08-13 caia en el `_` de abajo: la direccion salia
                    // CERO, en silencio, y `SearchCollection` de `m_config.c`
                    // devolvia `NULL` habiendo ENCONTRADO su entrada. DOOM se
                    // mataba con `I_Error` a 56.465 lineas de aqui.
                    //
                    // Las tres son la version SIN CARGA de los brazos que ya
                    // existen mas abajo: `Field`, `Arrow` e `IndexPtr` calculan
                    // la direccion y luego llaman a `emit_load_elem`. Tomar la
                    // direccion es exactamente eso menos el ultimo paso.
                    Expr::IndexPtr(base, index) => {
                        let elem = &self.pointee_type(base).unwrap_or(TypeSpec::Long);
                        self.emit_index_ptr_addr(base, index, &elem.clone());
                    }
                    Expr::Field(base, _campo) => {
                        let offset = &self.offset_de_valor(base, _campo);
                        self.emit_expr_as_ptr(base);
                        self.emit_add_offset(*offset);
                    }
                    Expr::Arrow(ptr, _campo) => {
                        let offset = &self.offset_por_puntero(ptr, _campo);
                        self.emit_expr(ptr);
                        self.emit_add_offset(*offset);
                    }
                    // ** Y ESTE BRAZO YA NO RELLENA DE CEROS: GRITA.
                    //
                    // Era la tercera vez que el mismo `_ =>` mudo costaba un
                    // dia de fotos --el `char *mapa` del raycaster
                    // (`2bc13367`), las relocations que no existian
                    // (`46506e51`), y esta--. El patron es siempre el mismo:
                    // un brazo por defecto que produce un valor LEGITIMO (cero
                    // es una direccion valida de escribir en cualquier
                    // expresion) para el caso "no supe traducirlo".
                    //
                    // Un compilador que no sabe tomar una direccion tiene que
                    // decirlo AQUI, donde la frase esta entera, y no dejar que
                    // el programa lo descubra en metal.
                    otro => {
                        self.errors.push(format!(
                            "no se de que forma tomar la direccion de esta expresion: {otro:?}"
                        ));
                        self.emit_xor_eax();
                    }
                }
            }
            Expr::Subscript(name, index) => {
                // direccion exacta (array o puntero) + carga del TAMANO del elemento
                self.emit_subscript_addr(name, index);
                let elem = self.elem_type_of(name);
                self.emit_load_elem(&elem);
            }
            // ** `E1 op= E2` con la direccion de `E1` calculada UNA vez.
            Expr::AssignOp(lvalue, kind, rhs) => self.emit_assign_op(lvalue, *kind, rhs),
            Expr::AssignSubscript(name, index, val) => {
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_subscript_addr(name, index); // rax = direccion
                self.code.push(0x5A);         // pop rdx = valor
                let elem = self.elem_type_of(name);
                self.emit_store_elem(&elem);  // [rax] = rdx (tamano exacto)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = valor (resultado del assign)
            }
            Expr::IndexPtr(base, index) => {
                let elem = &self.pointee_type(base).unwrap_or(TypeSpec::Long);
                // p->arr[i]: direccion = base(puntero) + i*sizeof(elem), luego load
                self.emit_index_ptr_addr(base, index, elem);
                self.emit_load_elem(&elem.clone());
            }
            Expr::AssignIndexPtr(base, index, val) => {
                let elem = &self.pointee_type(base).unwrap_or(TypeSpec::Long);
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
                // `p - n` retrocede n ELEMENTOS.
                match (self.pointer_scale(a), self.pointer_scale(b)) {
                    (Some(scale), None) => {
                        let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                        self.emit_binop(a, &scaled, SUB);
                    }
                    // ** PUNTERO MENOS PUNTERO DA UN INDICE, NO UNOS BYTES.
                    //
                    // El comentario que habia aqui decia que este caso *"no se
                    // deduce aqui"* -- o sea, hueco reconocido y sin cerrar. Y
                    // el resultado era plausible y equivocado: con `int *`,
                    // `b - a` sobre cinco elementos contestaba **20**.
                    //
                    // Es la cuenta inversa de `p + n`: aquella multiplica por el
                    // tamano del elemento, esta divide. Y la division va CON
                    // SIGNO, porque `a - b` con `a` antes que `b` es negativo y
                    // eso es legal en C -- una division sin signo lo convertiria
                    // en un numero gigante.
                    //
                    // Lo destapo la sonda del lenguaje, no un arranque.
                    (Some(scale), Some(_)) if scale > 1 => {
                        self.emit_binop(a, b, SUB);
                        // mov rcx, scale ; cqo ; idiv rcx
                        self.code.extend_from_slice(&[0x48, 0xC7, 0xC1]);
                        self.code.extend_from_slice(&scale.to_le_bytes());
                        self.code.extend_from_slice(&[0x48, 0x99]);
                        self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]);
                    }
                    _ => self.emit_binop(a, b, SUB),
                }
            }
            Expr::Mul(a, b) => self.emit_binop(a, b, &[0x48, 0x0F, 0xAF, 0xC2]),
            // `a / b` CON SIGNO. Antes hacia dos `pop` habiendo empujado una
            // sola vez --se llevaba un valor de la pila que no era suyo-- y
            // ademas dividia sin signo. `10 / 3` daba 0.
            //
            // ** Y SIN SIGNO es `div` con `rdx` a CERO, no `cqo`+`idiv`.
            // `cqo` extiende el signo de `rax` a `rdx`, o sea que con el bit 63
            // puesto deja `rdx = -1` y la division de 128 bits se hace sobre un
            // dividendo negativo. Ver `expr_is_unsigned`.
            Expr::Div(a, b) => {
                if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax   -> divisor = b
                        0x48, 0x89, 0xD0, // mov rax, rdx   -> dividendo = a
                        0x48, 0x31, 0xD2, // xor rdx, rdx   -> la mitad alta, a cero
                        0x48, 0xF7, 0xF1, // div rcx
                    ])
                } else {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x99,       // cqo            -> extiende el signo
                        0x48, 0xF7, 0xF9, // idiv rcx
                    ])
                }
            }
            // `a % b`: el resto queda en rdx.
            Expr::Mod(a, b) => {
                if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x31, 0xD2, // xor rdx, rdx
                        0x48, 0xF7, 0xF1, // div rcx
                        0x48, 0x89, 0xD0, // mov rax, rdx  -> el resto
                    ])
                } else {
                    self.emit_binop(a, b, &[
                        0x48, 0x89, 0xC1, // mov rcx, rax
                        0x48, 0x89, 0xD0, // mov rax, rdx
                        0x48, 0x99,       // cqo
                        0x48, 0xF7, 0xF9, // idiv rcx
                        0x48, 0x89, 0xD0, // mov rax, rdx  -> el resto
                    ])
                }
            }
            // Comparaciones: si algun operando es float -> comisd (setcc unsigned);
            // si no, la comparacion entera de siempre.
            // Comparaciones enteras: todas comparan `a` contra `b` en ese
            // orden y usan el setcc que les toca. Antes `<`, `>` y `>=`
            // comparaban al reves --`1 < 2` daba 0-- porque la comparacion se
            // hacia sobre `b - a` con el setcc de la forma directa.
            Expr::Eq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x94) } else { self.emit_cmp(a, b, 0x94) },
            Expr::Neq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x95) } else { self.emit_cmp(a, b, 0x95) },
            // ** Las cuatro de ORDEN llevan DOS `setcc`: con signo y sin el.
            //
            // `setl`/`setb` no son la misma instruccion porque `<` no es la
            // misma pregunta: `0x8000000000000000 > 1` es cierto para un
            // `unsigned long` y falso para un `long`. Las de igualdad (`==`,
            // `!=`) no cambian -- dos patrones de bits son iguales o no lo son,
            // y eso no depende de como se lean.
            //
            // El `setcc` sin signo es el de flotante: `comisd` deja las
            // banderas en la forma no ordenada, y esos son justo los codigos
            // `setb`/`seta`/`setbe`/`setae`. Por eso el brazo de float ya los
            // usaba y el entero no.
            Expr::Lt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x92) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x92) }
                else { self.emit_cmp(a, b, 0x9C) },
            Expr::Gt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x97) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x97) }
                else { self.emit_cmp(a, b, 0x9F) },
            Expr::Le(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x96) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x96) }
                else { self.emit_cmp(a, b, 0x9E) },
            Expr::Ge(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x93) }
                else if self.expr_is_unsigned(a) || self.expr_is_unsigned(b) { self.emit_cmp(a, b, 0x93) }
                else { self.emit_cmp(a, b, 0x9D) },
            Expr::BitAnd(a, b) => self.emit_binop(a, b, &[0x48, 0x21, 0xD0]),
            Expr::BitXor(a, b) => self.emit_binop(a, b, &[0x48, 0x31, 0xD0]),
            Expr::BitOr(a, b) => self.emit_binop(a, b, &[0x48, 0x09, 0xD0]),
            // `a << b` / `a >> b`. Antes desplazaban el operando DERECHO por
            // el izquierdo: `1 << 3` intentaba `3 << 1`.
            //
            // A la izquierda no hay dos versiones: `shl` y `sal` son la misma
            // instruccion. A la derecha si -- `sar` copia el bit de signo y
            // `shr` mete ceros -- y **manda el operando IZQUIERDO**, no la
            // conversion usual: `1u >> x` es sin signo aunque `x` sea `int`.
            Expr::Shl(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax   -> cuenta = b
                0x48, 0x89, 0xD0, // mov rax, rdx   -> valor  = a
                0x48, 0xD3, 0xE0, // shl rax, cl
            ]),
            Expr::Shr(a, b) => {
                let logico = self.expr_is_unsigned(a);
                self.emit_binop(a, b, &[
                    0x48, 0x89, 0xC1, // mov rcx, rax
                    0x48, 0x89, 0xD0, // mov rax, rdx
                    // shr rax,cl (/5) sin signo, sar rax,cl (/7) con el
                    0x48, 0xD3, if logico { 0xE8 } else { 0xF8 },
                ])
            }
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
            Expr::Field(base, campo) => self.emit_leer_campo(base, campo, Por::Valor),
            Expr::Arrow(ptr, campo) => self.emit_leer_campo(ptr, campo, Por::Puntero),
            Expr::AssignField(base, campo, val) => {
                self.emit_guardar_campo(base, campo, Por::Valor, val)
            }
            // *** `*p = x` CON EL ANCHO DE LO APUNTADO, y no siempre ocho.
            //
            // Hasta el 2026-09-03 esto emitia `mov [rax], rdx` a secas: OCHO
            // bytes, sobre cualquier puntero. `*p = 1` en un `char *` se
            // llevaba siete vecinos por delante.
            //
            // ** Lo encontro DOOM, y por una suma: `I_VideoBuffer` mide 64.000
            // bytes y acaba justo donde empezaba el bloque pisado del monton.
            // `r_draw.c` pinta con `*dest = dc_colormap[...]`, o sea que el
            // ULTIMO pixel de la pantalla escribia siete bytes fuera.
            //
            // [!] Y por eso duro meses: DOOM dibuja las columnas de izquierda a
            // derecha, asi que los siete bytes que cada escritura se lleva los
            // vuelve a escribir la columna siguiente. Se veia BIEN. Solo
            // sobrevivia el desperdicio de la ultima, que es la que cae fuera.
            // **Un fallo que se repara solo el 99,7% de las veces es de los que
            // no se encuentran mirando la pantalla.**
            //
            // * El `unwrap_or(Long)` conserva el comportamiento viejo cuando el
            // tipo no se resuelve: ocho bytes. No se convierte en error aqui
            // porque eso es una decision aparte y mas ancha que este arreglo.
            Expr::AssignDeref(addr, val) => {
                let apuntado = self.pointee_type(addr).unwrap_or(TypeSpec::Long);
                self.emit_expr(val); // rax = valor
                self.code.push(0x50); // push valor
                self.emit_expr(addr); // rax = direccion
                self.code.push(0x5A); // pop rdx = valor
                self.emit_store_elem(&apuntado); // <- el ancho EXACTO
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = valor
            }
            Expr::AssignArrow(ptr, campo, val) => {
                self.emit_guardar_campo(ptr, campo, Por::Puntero, val)
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

    /// **La tabla de simbolos del `.bex`.** Ver la llamada en `build_bef`.
    ///
    /// ## El TAMANO se deduce, y por eso vale mas que `--map`
    ///
    /// `--map` da la direccion de inicio de cada funcion. Con eso, un `rip`
    /// entre dos funciones se atribuye a la de arriba **aunque caiga fuera de
    /// ella** -- en un hueco de relleno, o en una funcion que el mapa no vio.
    ///
    /// Aqui el tamano sale de la distancia a la siguiente funcion, y la ultima
    /// llega hasta el final del codigo. Con eso, quien lee puede decir *"esta
    /// direccion NO esta en ninguna funcion"*, que es una respuesta distinta y
    /// mucho mas util que un nombre equivocado.
    ///
    /// [!] `virt_addr` guarda el offset DENTRO de la seccion de codigo, no una
    /// direccion virtual, y `section_idx` dice cual es esa seccion. El
    /// compilador no decide donde se carga el programa -- eso es del cargador, y
    /// escribir aqui una direccion absoluta seria repetir una decision ajena.
    fn seccion_de_simbolos(&self) -> BefSection {
        use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility};

        let mut orden: Vec<(usize, &String)> =
            self.function_offsets.iter().map(|(n, off)| (*off, n)).collect();
        orden.sort();

        let mut entradas: Vec<Symbol> = Vec::with_capacity(orden.len());
        let mut cadenas: Vec<u8> = Vec::new();

        for (i, (offset, nombre)) in orden.iter().enumerate() {
            // Hasta donde llega: el principio de la siguiente, o el final del
            // codigo si es la ultima.
            let fin = orden
                .get(i + 1)
                .map(|(sig, _)| *sig)
                .unwrap_or(self.instruction_end);

            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(nombre.as_bytes());
            cadenas.push(0); // las cadenas acaban en cero: quien lee no trae longitudes

            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(nombre),
                virt_addr: *offset as u64,
                size: fin.saturating_sub(*offset) as u64,
                kind: SymbolKind::Function as u8,
                // Todos LOCAL por ahora, y es la verdad de hoy: sin enlazador no
                // hay nadie a quien exportar. Cuando exista, esto lo decide
                // `static` en el fuente y no una constante aqui.
                binding: SymbolBinding::Local as u8,
                visibility: SymbolVisibility::Default as u8,
                section_idx: 0, // la seccion de codigo se anade siempre la primera
                _reserved: 0,
            });
        }

        BefSection::symbols(entradas, cadenas)
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

        // * LA SECCION `Bss`: los globales que son todo ceros. No lleva ni un
        // byte en el fichero -- solo dice cuantos hacen falta, y el cargador
        // reserva las paginas y las entrega a cero, que es lo que ya hacia con
        // cualquier seccion (`phys::zero_frame` antes de copiar).
        //
        // Va DESPUES de `data` y antes de las relocs, porque el cargador coloca
        // en el orden de la tabla y `patch_all_fixups` calculo `va_bss`
        // contando con que `.data` va justo delante.
        if self.bss_len > 0 {
            let mut bss_sec = BefSection::bss(self.bss_len as u64);
            bss_sec.alignment = 4096;
            b.add_section(bss_sec);
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

        // ** LA SECCION `Symbols`: que funcion vive en cada offset.
        //
        // === Por que existe, y por que hasta hoy no ===
        //
        // El compilador SIEMPRE ha sabido esto: `function_offsets` lo lleva
        // dentro para resolver las llamadas. El 2026-08-13 salio por la consola
        // con `--map`, porque DOOM murio con `rip 0x400815f2` y ese numero no
        // servia para nada. Pero salir por la consola obliga a que alguien
        // recompile el programa y reste a mano cada vez.
        //
        // Escribirlo en el `.bex` cierra el circuito: el que tiene el binario
        // tiene los nombres. La autopsia del kernel puede decir
        // `SHA1_Update+0x18` sin que nadie pase nada por `--map`.
        //
        // ** Y es la primera fila de la COMPILACION SEPARADA. Un enlazador
        // necesita saber que ofrece cada objeto; esto es exactamente eso, aunque
        // hoy lo lea un depurador y no un enlazador.
        //
        // [!] `SectionKind::Symbols` y `BefSection::symbols()` llevaban escritos
        // desde que se diseno BEF y **no los usaba nadie** -- igual que
        // `Resources` antes del paquete. El sitio ya estaba; faltaba llenarlo.
        //
        // No es cargable (`is_loadable` solo mapea Code/RoData/Data/Bss), asi
        // que **no cuesta ni una pagina al proceso**: viaja en el fichero y el
        // cargador la salta.
        if !self.function_offsets.is_empty() {
            b.add_section(self.seccion_de_simbolos());
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
