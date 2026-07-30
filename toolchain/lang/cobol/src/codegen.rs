use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::syscalls::surface;
use bmo_sem_asm::Instructions;
use bmo_sem_asm::x86_64::{Asm, Reg};
use bmo_lower::x86;
use crate::ast::{CobolProgram, CobolStatement, CobolCondition, DisplayArg};
use crate::ast::error::CobolError;
use crate::edicion::Plantilla;

type Result<T> = core::result::Result<T, CobolError>;

// BMO x86-64 SYSCALL argument registers: RDI, RSI, RDX, R10, R8, R9.
// RCX lo pisa el CPU con la dirección de retorno del usuario. Antes esto era
// la tabla REG_MOV de bytes a mano; ahora el mov reg,rax lo emite el encoder
// sem-asm (mismos bytes, leídos de la tabla TOML).
const ARG_REGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::R10, Reg::R8, Reg::R9];

pub fn compile_to_bef_bytes(program: &CobolProgram) -> Result<Vec<u8>> {
    let mut cg = Codegen::new();
    cg.emit_program(program)?;
    Ok(cg.build_bef())
}

struct StrFixup { lea_offset: usize, string_idx: usize }
struct CallReloc { offset: usize, target: String }

struct Codegen {
    code: Vec<u8>,
    strings: Vec<String>,
    str_fixups: Vec<StrFixup>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    var_offsets: HashMap<String, i32>,
    /// Escala decimal por variable (dígitos tras la V del PIC). El alma
    /// bancaria: un `PIC 9(3)V99` tiene escala 2 → guarda centavos.
    var_scales: HashMap<String, u32>,
    /// Plantilla de edición por variable, para las PIC de PRESENTACIÓN.
    /// Sólo cambia cómo se ESCRIBE: el dato sigue siendo el mismo entero
    /// escalado y la aritmética no se entera de que existe.
    var_edicion: HashMap<String, Plantilla>,
    /// Los ficheros de `FILE-CONTROL`, por nombre.
    files: HashMap<String, crate::ast::CobolFile>,
    /// Dónde vive el handle de cada fichero: una ranura de pila sin nombre en
    /// COBOL. Entre el `OPEN` y el `CLOSE` pasa el programa entero, y
    /// cualquier `DISPLAY` hace un `syscall` que destruye medio banco de
    /// registros — así que un registro no vale.
    file_handles: HashMap<String, i32>,
    /// Y su ESTADO: la ranura donde el último `READ` dejó "hubo registro".
    ///
    /// Va en la pila por la misma razón que el handle, y con una razón de más:
    /// entre leer y decidir la rama pasa la conversión del registro, y
    /// `fmt::parse_decimal_scaled` usa `r10` y `r11` para su propio recuento.
    /// Guardar la bandera en un registro caller-saved daba un `AT END` que
    /// saltaba con el fichero LLENO — y sólo se notaba si la PIC no tenía
    /// decimales, porque con `V99` el `r11` acababa valiendo 1 de casualidad.
    file_estado: HashMap<String, i32>,
    /// De qué fichero es cada registro. Es lo que hace que `WRITE SALDO` sepa
    /// a dónde va sin que nadie se lo diga.
    record_owner: HashMap<String, i32>,
    next_label: u32,
    /// Offset donde quedó fijada cada etiqueta.
    label_offsets: HashMap<u32, usize>,
    /// Saltos pendientes: (offset del campo rel32, etiqueta destino).
    ///
    /// Esto es lo que faltaba y hacía que el flujo de control fuera una
    /// mentira: antes se emitían `jcc` con desplazamiento 0 —o sea, "saltar
    /// a la instrucción siguiente"— y nadie los parcheaba nunca. El `IF`
    /// ejecutaba las dos ramas y el `PERFORM` no repetía nada, pero el BEF
    /// compilaba y validaba igual.
    jump_fixups: Vec<(usize, u32)>,
    /// Errores detectados durante la emision (expresiones malformadas).
    /// Se acumulan y se reportan al final en vez de emitir codigo que
    /// calcula cualquier cosa.
    errors: Vec<CobolError>,
    stack_size: i32,
    /// Tabla de instrucciones sem-asm (opcodes leídos de la TOML).
    isa: Instructions,
}

impl Codegen {
    fn new() -> Self {
        Self {
            code: vec![],
            strings: vec![],
            str_fixups: vec![],
            call_relocs: vec![],
            function_offsets: HashMap::new(),
            var_offsets: HashMap::new(),
            var_scales: HashMap::new(),
            var_edicion: HashMap::new(),
            files: HashMap::new(),
            file_handles: HashMap::new(),
            file_estado: HashMap::new(),
            record_owner: HashMap::new(),
            next_label: 0,
            label_offsets: HashMap::new(),
            jump_fixups: Vec::new(),
            errors: Vec::new(),
            stack_size: 0,
            isa: Instructions::load_x86_64().expect("tablas sem-asm x86-64 (forge/sem-asm/tables)"),
        }
    }

    /// Escala decimal de una variable (0 = entero / no declarada).
    fn var_scale(&self, name: &str) -> u32 {
        *self.var_scales.get(name).unwrap_or(&0)
    }

    /// Convierte un literal COBOL a su ENTERO escalado por `scale`. Es el
    /// corazón del decimal exacto: `"10.05"` con escala 2 → `1005` centavos;
    /// `"3.2"` → `320`; `"7"` → `700`. Trunca los decimales sobrantes (el
    /// default de COBOL sin `ROUNDED`). Un entero con escala 0 queda igual.
    ///
    /// El signo se aplica al final. Antes se quitaba con `trim_start_matches`
    /// y no se volvía a poner nunca: `MOVE -120.00 TO SALDO` guardaba +12000
    /// y un descubierto aparecía en verde. La aritmética de abajo ya es con
    /// signo (`idiv`, `jl`/`jg`), así que el complemento a dos vale tal cual.
    fn scaled_imm(lit: &str, scale: u32) -> u64 {
        let t = lit.trim();
        let negativo = t.starts_with('-');
        let s = t.trim_start_matches(['+', '-']);
        let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
        let int_val: u64 = int_part.parse().unwrap_or(0);
        let mut frac = frac_part.to_string();
        while (frac.len() as u32) < scale {
            frac.push('0');
        }
        let frac_val: u64 = if scale == 0 {
            0
        } else {
            frac[..scale as usize].parse().unwrap_or(0)
        };
        let magnitud = int_val * 10u64.pow(scale) + frac_val;
        if negativo {
            (magnitud as i64).wrapping_neg() as u64
        } else {
            magnitud
        }
    }

    /// `mov rax, <literal escalado a `scale`>`.
    fn load_scaled_imm(&mut self, lit: &str, scale: u32) {
        let v = Self::scaled_imm(lit, scale);
        self.emit_asm(|a| { a.mov_imm64(Reg::Rax, v).unwrap(); });
    }

    /// Emite bytes con el encoder sem-asm (opcode de la tabla + REX/ModRM).
    fn emit_asm(&mut self, build: impl FnOnce(&mut Asm)) {
        let mut a = Asm::new(&self.isa);
        build(&mut a);
        self.code.extend_from_slice(a.bytes());
    }

    fn fresh_label(&mut self) -> u32 { let l = self.next_label; self.next_label += 1; l }

    /// Fija una etiqueta en la posición actual del código.
    fn bind_label(&mut self, label: u32) {
        let here = self.code.len();
        self.label_offsets.insert(label, here);
    }

    /// `jmp <etiqueta>` (rel32, se parchea al final).
    fn emit_jmp(&mut self, label: u32) {
        self.code.push(0xE9);
        self.jump_fixups.push((self.code.len(), label));
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// `jcc <etiqueta>` con el segundo byte del opcode (`0x84`=je,
    /// `0x85`=jne, `0x8C`=jl, `0x8D`=jge, `0x8E`=jle, `0x8F`=jg).
    ///
    /// Siempre rel32: el cuerpo de un `PERFORM` o de un `IF` puede crecer
    /// más allá de los 127 bytes de un rel8, y un salto que se desborda en
    /// silencio es peor que uno largo de más.
    fn emit_jcc(&mut self, cc: u8, label: u32) {
        self.code.extend_from_slice(&[0x0F, cc]);
        self.jump_fixups.push((self.code.len(), label));
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// Resuelve todos los saltos. Una etiqueta sin fijar es un bug del
    /// emisor, no del programa COBOL: se aborta en vez de emitir un salto
    /// a ninguna parte.
    fn patch_jumps(&mut self) {
        for (field, label) in std::mem::take(&mut self.jump_fixups) {
            let target = *self
                .label_offsets
                .get(&label)
                .unwrap_or_else(|| panic!("etiqueta {label} usada pero nunca fijada"));
            let rel = (target as i64 - (field as i64 + 4)) as i32;
            self.code[field..field + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    /// ¿Este nombre es un dato declarado, o un literal?
    ///
    /// Es la pregunta que el descenso nunca hacía: todo operando se trataba
    /// como literal, así que `ADD PRECIO TO TOTAL` sumaba cero (el parseo
    /// numérico de "PRECIO" fallaba y caía a `unwrap_or(0)`).
    fn is_variable(&self, name: &str) -> bool {
        self.var_offsets.contains_key(name)
    }

    /// Carga un operando en `rax`, reescalado a `scale`.
    ///
    /// El reescalado es lo que hace que el decimal siga siendo exacto al
    /// mezclar datos de distinta PIC: sumar un `PIC 9(3)` (escala 0) a un
    /// `PIC 9(3)V99` (escala 2) exige multiplicar por 100 primero, si no se
    /// sumarían centavos con pesos.
    fn load_operand(&mut self, name: &str, scale: u32) {
        if self.is_variable(name) {
            let from = self.var_scale(name);
            self.load_var(name);
            self.rescale(from, scale);
        } else {
            self.load_scaled_imm(name, scale);
        }
    }

    /// Lleva `rax` de la escala `from` a la escala `to`.
    fn rescale(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        if to > from {
            let factor = 10u64.pow(to - from);
            self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, factor).unwrap(); });
            self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]); // imul rax, rcx
        } else {
            let factor = 10u64.pow(from - to);
            self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, factor).unwrap(); });
            self.code.extend_from_slice(&[0x48, 0x99]);       // cqo
            self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
        }
    }

    /// Escala común en la que comparar dos operandos: la mayor de las dos,
    /// para no perder decimales al comparar.
    fn comparison_scale(&self, a: &str, b: &str) -> u32 {
        let sa = if self.is_variable(a) { self.var_scale(a) } else { 0 };
        let sb = if self.is_variable(b) { self.var_scale(b) } else { 0 };
        sa.max(sb)
    }

    fn emit_program(&mut self, program: &CobolProgram) -> Result<()> {
        self.stack_size = 0;
        for item in &program.data_items {
            let size = item.storage_size();
            let aligned = (size as i32 + 7) & !7;
            self.stack_size += aligned;
            self.var_offsets.insert(item.name.clone(), -(self.stack_size));
            // Recuerda la escala decimal del PIC para la aritmética exacta.
            self.var_scales.insert(item.name.clone(), item.scale());
            if let Some(p) = &item.edicion {
                self.var_edicion.insert(item.name.clone(), p.clone());
            }
        }
        // Dos ranuras de pila por fichero: su handle y su estado. Van DESPUÉS
        // de los datos y con el mismo mecanismo: son variables que COBOL no
        // nombra.
        for f in &program.files {
            self.stack_size += 8;
            let off = -(self.stack_size);
            self.stack_size += 8;
            self.file_estado.insert(f.name.clone(), -(self.stack_size));
            self.file_handles.insert(f.name.clone(), off);
            self.files.insert(f.name.clone(), f.clone());
            if !f.record.is_empty() {
                self.record_owner.insert(f.record.to_ascii_uppercase(), off);
            }
        }
        self.collect_strings(program);

        // Function prologue
        self.code.extend_from_slice(&[0x55]);              // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        // A BEF process entry may be reached by a jump rather than CALL. Do not
        // assume an incoming RSP residue: reserve all locals plus alignment
        // slack, then establish BMO's required 64-byte pre-call alignment.
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
        self.code.extend_from_slice(&((self.stack_size as u32) + 63).to_le_bytes());
        self.code.extend_from_slice(&[0x48, 0x83, 0xE4, 0xC0]); // and rsp, -64

        // Emit COBOL statements
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }

        // STOP RUN implícito al final del programa.
        //
        // Antes el cierre era `INVOKE(EXIT)` + `hlt`. Pero `hlt` es una
        // instrucción PRIVILEGIADA: si EXIT alguna vez retornara, en Ring 3
        // eso es un #GP inmediato — una "red de seguridad" que provoca
        // justo el fallo del que protege. La red correcta es girar en
        // `pause`, que es lo que emite la puerta.
        bmo_lower::task::exit(&mut self.code);

        // Syscall stub
        let stub_off = self.code.len();
        self.code.extend_from_slice(&[0x0F, 0x05, 0xC3]); // syscall; ret
        self.function_offsets.insert("__bmo_syscall_stub".to_string(), stub_off);
        self.patch_jumps();
        self.patch_call_relocs();
        self.patch_string_fixups();
        if let Some(err) = self.errors.first() {
            return Err(err.clone());
        }
        Ok(())
    }

    fn collect_strings(&mut self, p: &CobolProgram) {
        for stmt in &p.statements {
            // Solo los literales van a la tabla de cadenas: una variable no
            // tiene texto que guardar, su texto se fabrica al ejecutar.
            if let CobolStatement::Display(DisplayArg::Literal(s)) = stmt {
                if !self.strings.iter().any(|t| *t == *s) {
                    self.strings.push(s.clone());
                }
            }
        }
    }

    fn load_var(&mut self, name: &str) {
        if let Some(&off) = self.var_offsets.get(name) {
            if off >= -128 && off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x45, off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
        }
    }

    fn store_var(&mut self, name: &str) {
        if let Some(&off) = self.var_offsets.get(name) {
            if off >= -128 && off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x89, 0x45, off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
        }
    }

    fn load_imm64(&mut self, val: &str) {
        let num: u64 = val.parse().unwrap_or(0);
        // mov rax, imm64 — antes [0x48,0xB8]+imm a mano; ahora el encoder.
        self.emit_asm(|a| { a.mov_imm64(Reg::Rax, num).unwrap(); });
    }

    /// `DISPLAY "literal"` — la L2 de COBOL sobre la puerta genérica (L1).
    ///
    /// Lo específico de COBOL que se decide AQUÍ: que `DISPLAY` termina
    /// siempre en salto de línea (el kernel hace flush de la línea con
    /// `\n`, así que cada DISPLAY ocupa su propia fila, como manda el
    /// lenguaje). Cuando llegue `DISPLAY <variable>`, la edición PIC se
    /// aplicará también aquí y el resultado saldrá por
    /// `bmo_lower::console::write_buffer` — L1 seguirá sin saber qué es una
    /// PIC.
    ///
    /// Antes esto emitía `lea rdi,[str]; mov esi,len; syscall NR_DEBUG_PRINT`:
    /// número plano que el kernel ya no despacha, y encima pasando un
    /// puntero, que la superficie congelada rechaza. No imprimía nada.
    fn emit_display(&mut self, s: &str) {
        let mut text = s.as_bytes().to_vec();
        text.push(b'\n');
        bmo_lower::console::write_const(&mut self.code, &text);
    }

    /// `DISPLAY <variable>` — el valor, formateado EN EJECUCION.
    ///
    /// Tiene que ser en ejecucion: el compilador no sabe cuanto vale `SALDO`
    /// despues de tres `ADD`. Se carga el entero escalado y se llama al
    /// formateador de `bmo-lower`, que le pone el punto donde dice la PIC.
    ///
    /// `PIC 9(5)V99` con 5997 dentro imprime `59.97`. Los centavos nunca han
    /// dejado de ser un entero; lo unico que cambia es donde va la coma al
    /// escribirlo.
    fn emit_display_var(&mut self, nombre: &str) {
        let escala = self.var_scale(nombre);
        self.load_var(nombre);
        // ★ Si el dato lleva PIC de EDICION, lo que sale no es el numero: es
        // la mascara. `12345.67` deja de ser "12345.67" y pasa a ser
        // "$12,345.67" — que es la linea de un extracto, no un volcado.
        //
        // La plantilla se consume AQUI, al compilar: lo que va al `.bex` es
        // el recorrido convertido en instrucciones, no la plantilla ni un
        // interprete que la lea.
        if let Some(p) = self.var_edicion.get(nombre).cloned() {
            if let Err(e) = p.emitir(&mut self.code) {
                self.errors.push(CobolError::new(0, e));
            }
        } else {
            bmo_lower::fmt::write_decimal_scaled(&mut self.code, escala);
        }
        // El salto de linea, aparte: el formateador escribe el numero y nada
        // mas, que es lo correcto para poder encadenar campos algun dia.
        bmo_lower::console::write_const(&mut self.code, b"\n");
    }

    /// `ACCEPT <variable>` — lee una linea y la guarda con la escala de su PIC.
    ///
    /// El buffer vive en la pila del programa: 64 bytes, que es mas de lo que
    /// nadie teclea en un importe y menos de lo que estorba.
    fn emit_accept(&mut self, destino: &str) {
        const BUF: i8 = 64;
        let escala = self.var_scale(destino);
        // Hueco en la pila y r8 = principio del buffer.
        x86::sub_r64_imm8(&mut self.code, x86::RSP, BUF);
        x86::lea_r64_rsp_disp8(&mut self.code, x86::R8, 0);
        // El tope va como INMEDIATO. Antes viajaba en `rcx` y `read_line` lo
        // guardaba en `r11`, que el `syscall` pisa con RFLAGS: el guarda del
        // buffer estaba muerto y una linea larga se salia de estos 64 bytes.
        // La linea entra en el buffer; r9 queda con su largo. `r8` avanza al
        // final, asi que hay que devolverlo al principio para leerlo.
        bmo_lower::console::read_line(&mut self.code, BUF as u8);
        x86::sub_r64_r64(&mut self.code, x86::R8, x86::R9);
        bmo_lower::fmt::parse_decimal_scaled(&mut self.code, escala);
        x86::add_r64_imm8(&mut self.code, x86::RSP, BUF);
        self.store_var(destino);
    }

    // ── E/S de ficheros ─────────────────────────────────────────────────
    //
    // El HANDLE de cada fichero vive en una ranura de la pila, como una
    // variable mas — solo que sin nombre en COBOL. Va en la pila y no en un
    // registro porque entre el `OPEN` y el `CLOSE` pasa el programa entero:
    // cualquier `DISPLAY` hace un `syscall`, y eso destruye medio banco de
    // registros.

    /// La ranura del handle de este fichero, o un error si no se declaro.
    fn file_slot(&mut self, fichero: &str) -> Option<i32> {
        match self.file_handles.get(&fichero.to_ascii_uppercase()) {
            Some(&off) => Some(off),
            None => {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "'{fichero}' no esta declarado: falta su \
                         `SELECT {fichero} ASSIGN TO \"ruta\"` en FILE-CONTROL"
                    ),
                ));
                None
            }
        }
    }

    fn store_slot(&mut self, off: i32) {
        if off >= -128 && off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x89, 0x45, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    fn load_slot(&mut self, off: i32) {
        if off >= -128 && off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x8B, 0x45, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    /// `OPEN INPUT|OUTPUT <fichero>`.
    fn emit_open(&mut self, modo: &str, fichero: &str) {
        let Some(off) = self.file_slot(fichero) else { return };
        let Some(f) = self.files.get(&fichero.to_ascii_uppercase()).cloned() else { return };
        let m = modo.trim().to_ascii_uppercase();
        let escribe = match m.as_str() {
            "INPUT" => false,
            "OUTPUT" => true,
            // `EXTEND` es AÑADIR al final, y la puerta que hay abre con
            // `TASK_OP_ARCHIVO_CREAR`: crea de cero. Compilarlo como OUTPUT
            // BORRARIA el historico entero y el programa parecería funcionar
            // —el fichero existe, tiene lineas nuevas— hasta que alguien
            // buscara el mes pasado. Se rechaza.
            "EXTEND" => {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "OPEN EXTEND {fichero}: la puerta de archivos abre creando de cero, \
                         asi que esto BORRARIA lo que ya hay. Falta el modo anadir en \
                         `KIND_ARCHIVO`; usa OPEN OUTPUT si de verdad quieres reescribirlo"
                    ),
                ));
                return;
            }
            otro => {
                // I-O queda fuera a proposito: leer y escribir a la vez sobre
                // el mismo handle no lo soporta `KIND_ARCHIVO`, que fija el
                // modo AL ABRIR. Se dice en vez de abrir en uno de los dos y
                // que el otro falle en ejecucion.
                self.errors.push(CobolError::new(
                    0,
                    format!("OPEN {otro}: solo INPUT y OUTPUT. I-O necesita un handle que lea y escriba, y el modo se fija al abrir"),
                ));
                return;
            }
        };
        bmo_lower::archivo::abrir_const(&mut self.code, f.path.as_bytes(), escribe);
        self.store_slot(off);
    }

    /// `CLOSE <fichero>`. En uno de salida, **aqui es donde llega al disco**.
    fn emit_close(&mut self, fichero: &str) {
        let Some(off) = self.file_slot(fichero) else { return };
        self.load_slot(off);
        self.emit_asm(|a| { a.mov_reg(Reg::R10, Reg::Rax).unwrap(); });
        bmo_lower::archivo::cerrar(&mut self.code);
    }

    /// `READ <f> AT END … NOT AT END … END-READ`.
    ///
    /// Lee una linea a un buffer de pila, la convierte al entero escalado del
    /// REGISTRO del fichero y lo guarda ahi. La conversion usa la misma
    /// pareja que `ACCEPT` — `parse_decimal_scaled` — porque un registro de
    /// texto y una linea tecleada son el mismo problema.
    fn emit_read(
        &mut self,
        fichero: &str,
        al_final: &[CobolStatement],
        si_hay: &[CobolStatement],
    ) {
        const BUF: i8 = 64;
        let Some(off) = self.file_slot(fichero) else { return };
        let Some(f) = self.files.get(&fichero.to_ascii_uppercase()).cloned() else { return };
        if f.record.is_empty() {
            self.errors.push(CobolError::new(
                0,
                format!("{fichero} no tiene registro: falta el `FD {fichero}` con su 01 debajo"),
            ));
            return;
        }
        let escala = self.var_scale(&f.record);

        self.load_slot(off);
        self.emit_asm(|a| { a.mov_reg(Reg::R10, Reg::Rax).unwrap(); });
        x86::sub_r64_imm8(&mut self.code, x86::RSP, BUF);
        x86::lea_r64_rsp_disp8(&mut self.code, x86::R8, 0);
        bmo_lower::archivo::leer_linea(&mut self.code, BUF as u8);
        // `rax` = 1 si hubo registro. Se guarda en la RANURA del fichero antes
        // de tocar nada: el parseo de abajo se lleva por delante `r10` y `r11`.
        let estado = self.file_estado[&fichero.to_ascii_uppercase()];
        self.store_slot(estado);
        // `r8` acabo al final de lo leido; se devuelve al principio.
        x86::sub_r64_r64(&mut self.code, x86::R8, x86::R9);
        bmo_lower::fmt::parse_decimal_scaled(&mut self.code, escala);
        x86::add_r64_imm8(&mut self.code, x86::RSP, BUF);
        self.store_var(&f.record);

        // Si no hubo registro, la rama de AT END. El valor parseado de un
        // buffer vacio es 0 y no se usa: quien escribe `AT END` sabe que ahi
        // no hay dato.
        let al_end = self.fresh_label();
        let fin = self.fresh_label();
        self.load_slot(estado);
        self.code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
        self.emit_jcc(0x84, al_end); // je → se acabo
        for s in si_hay {
            self.emit_statement(s);
        }
        self.emit_jmp(fin);
        self.bind_label(al_end);
        for s in al_final {
            self.emit_statement(s);
        }
        self.bind_label(fin);
    }

    /// `WRITE <registro>` — el valor del registro como una linea.
    fn emit_write(&mut self, registro: &str) {
        let reg = registro.trim().to_ascii_uppercase();
        // `WRITE X FROM Y` todavia no: se dice en vez de escribir X callando
        // que se ignoro el FROM.
        if reg.contains(' ') {
            self.errors.push(CobolError::new(
                0,
                format!("WRITE {registro}: solo `WRITE <registro>` por ahora (sin FROM)"),
            ));
            return;
        }
        let Some(&off) = self.record_owner.get(&reg) else {
            self.errors.push(CobolError::new(
                0,
                format!("'{registro}' no es el registro de ningun FD: WRITE no sabe a que fichero va"),
            ));
            return;
        };
        let escala = self.var_scale(&reg);
        let plantilla = self.var_edicion.get(&reg).cloned();
        self.load_var(&reg);
        // El texto al buffer de pila. Dos formas del MISMO reparto: editar (o
        // formatear) es una cosa y publicar es otra.
        //
        // Un registro con PIC editada escribe su LINEA, no su numero: eso es un
        // informe bancario, y es la razon de que exista `emitir_en_buffer`.
        // Escribir aqui el numero crudo callando que habia mascara seria
        // exactamente el fallo que este compilador no comete.
        //
        // El numero sin mascara se llena HACIA ATRAS desde el tope, asi que
        // `r8 + r9` cae justo en el final del hueco reservado: escribir ahi el
        // salto de linea seria un byte FUERA, sobre la pila del llamante. El
        // salto va en una segunda escritura, usando la parte baja del mismo
        // buffer —que esta libre porque un numero nunca lo llena entero.
        let pila = match &plantilla {
            Some(p) => match p.emitir_en_buffer(&mut self.code) {
                Ok(total) => total,
                Err(e) => {
                    self.errors.push(CobolError::new(0, format!("WRITE {registro}: {e}")));
                    return;
                }
            },
            None => {
                bmo_lower::fmt::formatear_decimal_scaled(&mut self.code, escala);
                bmo_lower::fmt::BUFFER
            }
        };
        // El handle DESPUES de editar y nunca antes: `emitir_en_buffer` usa
        // `r10` para recorrer los digitos, y ahi es donde va el handle.
        self.load_slot(off);
        self.emit_asm(|a| { a.mov_reg(Reg::R10, Reg::Rax).unwrap(); });
        bmo_lower::archivo::escribir_buffer(&mut self.code);
        // El salto, aparte: un registro por linea es lo que `leer_linea` sabe
        // deshacer.
        x86::lea_r64_rsp_disp8(&mut self.code, x86::R8, 0);
        x86::mov_byte_at_reg_imm8(&mut self.code, x86::R8, b'\n');
        x86::mov_r32_imm32(&mut self.code, x86::R9, 1);
        bmo_lower::archivo::escribir_buffer(&mut self.code);
        x86::add_r64_imm8(&mut self.code, x86::RSP, pila);
    }

    fn emit_statement(&mut self, stmt: &CobolStatement) {
        match stmt {
            CobolStatement::Syscall(def, args) => {
                if let Some(operation) = surface::task_operation_for_legacy_syscall(def.nr) {
                    self.emit_v2_task_invoke(operation, args);
                } else {
                    for (i, arg) in args.iter().enumerate() {
                        if i < ARG_REGS.len() {
                            let value: u64 = arg.parse().unwrap_or(0);
                            self.emit_imm64_syscall_arg(i, value);
                        }
                    }
                    self.emit_mov_eax_syscall(def.nr);
                }
            }
            CobolStatement::Display(arg) => match arg {
                DisplayArg::Literal(s) => self.emit_display(s),
                DisplayArg::Variable(v) => self.emit_display_var(v),
            },
            // ★ ACCEPT YA SE COMPILA.
            //
            // El error que habia aqui —"no hay puerta de entrada en la
            // superficie congelada"— dejo de ser verdad: `TASK_OP_CONSOLE_READ`
            // existe y el terminal que lanza el programa le pasa lo que se
            // teclea. Un mensaje de error que se queda cuando el motivo se ha
            // arreglado es peor que no tenerlo: manda a no intentarlo.
            CobolStatement::Accept(destino) => self.emit_accept(destino),

            // Decimal EXACTO: el literal se escala a la escala del destino,
            // así $10.05 en un PIC 9(3)V99 se guarda como el entero 1005.
            CobolStatement::Move(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_operand(src, sc);
                self.store_var(dst);
            }
            // ADD a misma escala: ambos operandos en centavos → `add` es
            // suma decimal exacta (sin float, sin redondeo). El alma bancaria.
            CobolStatement::Add(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);
                self.code.push(0x50);                    // push rax
                self.load_operand(src, sc);
                self.code.push(0x5A);                    // pop rdx
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                self.store_var(dst);
            }
            // SUBTRACT src FROM dst → dst = dst - src.
            //
            // La versión anterior cargaba `src` DOS veces y hacía un baile
            // de push/pop que no restaba lo que decía. Aquí el orden es
            // explícito: rdx = dst, rax = src, rdx -= rax.
            CobolStatement::Subtract(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);
                self.code.push(0x50);                             // push rax (dst)
                self.load_operand(src, sc);                       // rax = src
                self.code.push(0x5A);                             // pop rdx (dst)
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]); // sub rdx, rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
                self.store_var(dst);
            }
            // MULTIPLY decimal exacto: dst y src están en escala `sc`
            // (centavos). El producto queda en escala 2·sc → se REESCALA
            // dividiendo entre 10^sc para volver a `sc`. $2.00 × 3 = $6.00.
            CobolStatement::Multiply(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);                              // rax = dst_scaled
                self.code.push(0x50);                            // push rax
                self.load_operand(src, sc);                      // rax = src_scaled
                self.code.push(0x5A);                            // pop rdx
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                if sc > 0 {
                    let p = 10u64.pow(sc);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); }); // mov rcx, 10^sc
                    self.code.extend_from_slice(&[0x48, 0x99]);         // cqo
                    self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]);   // idiv rcx
                }
                self.store_var(dst);
            }
            // DIVIDE decimal exacto: dst / src. Para conservar la escala, el
            // dividendo se PREESCALA ×10^sc antes de dividir (si no, el
            // resultado quedaría en escala 0). $10.00 / 4 = $2.50.
            CobolStatement::Divide(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_operand(src, sc);                      // rax = src_scaled (divisor)
                self.code.push(0x50);                            // push rax
                self.load_var(dst);                              // rax = dst_scaled (dividendo)
                if sc > 0 {
                    let p = 10u64.pow(sc);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); }); // mov rcx, 10^sc
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]);    // imul rax, rcx
                }
                self.code.push(0x59);                            // pop rcx (divisor)
                self.code.extend_from_slice(&[0x48, 0x99]);       // cqo
                self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
                self.store_var(dst);
            }
            CobolStatement::Compute(dst, expr) => {
                let sc = self.var_scale(dst);
                self.emit_compute(expr, sc);
                self.store_var(dst);
            }
            // IF/ELSE con bifurcación REAL. Las condiciones se conjugan con
            // AND: la primera que falle salta a ELSE sin evaluar el resto
            // (cortocircuito, como manda el estándar).
            CobolStatement::If(cond, then_stmts, else_stmts) => {
                let else_label = self.fresh_label();
                let end_label = self.fresh_label();

                for c in cond {
                    self.emit_jump_if_condition_false(c, else_label);
                }
                for s in then_stmts {
                    self.emit_statement(s);
                }
                self.emit_jmp(end_label);

                self.bind_label(else_label);
                for s in else_stmts {
                    self.emit_statement(s);
                }
                self.bind_label(end_label);
            }

            // PERFORM <n> TIMES con un contador REAL en la pila.
            //
            // El contador vive en `[rsp]` y no en un registro porque el
            // cuerpo puede contener cualquier cosa —un DISPLAY hace un
            // `syscall`, que destruye rcx y r11—. Todos los emisores de
            // sentencias dejan la pila equilibrada, así que `[rsp]` sigue
            // apuntando al contador en cada iteración.
            CobolStatement::PerformTimes(n, body) => {
                let top = self.fresh_label();
                let done = self.fresh_label();

                self.emit_asm(|a| { a.mov_imm64(Reg::Rax, *n as u64).unwrap(); });
                self.code.push(0x50); // push rax → contador

                self.bind_label(top);
                self.code.extend_from_slice(&[0x48, 0x83, 0x3C, 0x24, 0x00]); // cmp qword [rsp], 0
                self.emit_jcc(0x8E, done); // jle done

                for s in body {
                    self.emit_statement(s);
                }

                self.code.extend_from_slice(&[0x48, 0xFF, 0x0C, 0x24]); // dec qword [rsp]
                self.emit_jmp(top);

                self.bind_label(done);
                self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
            }

            // PERFORM UNTIL <cond>: se prueba ANTES de cada iteración
            // (`WITH TEST BEFORE`, el default del estándar) y se sale cuando
            // la condición se cumple.
            CobolStatement::PerformUntil(cond, body) => {
                let top = self.fresh_label();
                let body_label = self.fresh_label();
                let done = self.fresh_label();

                self.bind_label(top);
                // Salir exige que TODAS las condiciones sean ciertas: la
                // primera falsa manda al cuerpo.
                for c in cond {
                    self.emit_jump_if_condition_false(c, body_label);
                }
                self.emit_jmp(done);

                self.bind_label(body_label);
                for s in body {
                    self.emit_statement(s);
                }
                self.emit_jmp(top);

                self.bind_label(done);
            }
            // E/S de ficheros y ACCEPT: se RECHAZAN en vez de emitir el
            // `syscall NR_FS_*` / `NR_INPUT_POLL_EVENT` de antes, números
            // planos que el kernel no despacha. Un programa que "compila" y
            // cuyo READ no lee nada es peor que uno que no compila: el
            // fichero se necesita como capability sobre BMO Channel, y esa
            // capa todavía no existe.
            // ★ E/S DE FICHEROS. El error que habia aqui —"necesita una
            // capability de sistema de ficheros que todavia no existe"— dejo
            // de ser verdad: `KIND_ARCHIVO` existe y `bmo-lower::archivo` es
            // su puerta.
            CobolStatement::Open(modo, fichero) => self.emit_open(modo, fichero),
            CobolStatement::Close(fichero) => self.emit_close(fichero),
            CobolStatement::Read(fichero, al_final, si_hay) => {
                self.emit_read(fichero, al_final, si_hay)
            }
            CobolStatement::Write(registro) => self.emit_write(registro),
            CobolStatement::StopRun => {}
            CobolStatement::Expr(_) => {}
        }
    }

    /// `COMPUTE dst = <expresión>` con precedencia real.
    ///
    /// Antes esto llamaba a `load_scaled_imm(expr)`, que intenta parsear la
    /// expresión ENTERA como un número: `COMPUTE T = A + B` no fallaba, se
    /// evaluaba a 0 y seguía.
    ///
    /// Toda la expresión se evalúa **en la escala del destino**: los
    /// operandos se cargan reescalados, el producto se divide entre 10^s y
    /// el dividendo se preescala. Es el mismo criterio que ya usaban
    /// MULTIPLY y DIVIDE, aplicado de forma uniforme, y es lo que mantiene
    /// el decimal exacto sin tocar punto flotante.
    fn emit_compute(&mut self, expr: &str, scale: u32) {
        let tokens = Self::tokenize_expr(expr);
        let mut pos = 0usize;
        self.emit_expr_sum(&tokens, &mut pos, scale);
        if pos != tokens.len() {
            self.errors.push(CobolError::new(
                0,
                format!("sobra '{}' al final de la expresion COMPUTE", tokens[pos..].join(" ")),
            ));
        }
    }

    /// Parte la expresión en operandos, operadores y paréntesis.
    fn tokenize_expr(expr: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = String::new();
        for ch in expr.chars() {
            if "+-*/()".contains(ch) {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
                out.push(ch.to_string());
            } else if ch.is_whitespace() {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                }
                current.clear();
            } else {
                current.push(ch);
            }
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_string());
        }
        out
    }

    /// `suma := producto (('+'|'-') producto)*`
    fn emit_expr_sum(&mut self, tokens: &[String], pos: &mut usize, scale: u32) {
        self.emit_expr_product(tokens, pos, scale);
        while *pos < tokens.len() && (tokens[*pos] == "+" || tokens[*pos] == "-") {
            let op = tokens[*pos].clone();
            *pos += 1;
            self.code.push(0x50); // push rax (izquierdo)
            self.emit_expr_product(tokens, pos, scale);
            self.code.push(0x5A); // pop rdx (izquierdo)
            if op == "+" {
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
            } else {
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]); // sub rdx, rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
            }
        }
    }

    /// `producto := factor (('*'|'/') factor)*`
    fn emit_expr_product(&mut self, tokens: &[String], pos: &mut usize, scale: u32) {
        self.emit_expr_factor(tokens, pos, scale);
        while *pos < tokens.len() && (tokens[*pos] == "*" || tokens[*pos] == "/") {
            let op = tokens[*pos].clone();
            *pos += 1;
            self.code.push(0x50); // push rax (izquierdo)
            self.emit_expr_factor(tokens, pos, scale);
            self.code.push(0x5A); // pop rdx (izquierdo)
            if op == "*" {
                // Ambos vienen en escala s; el producto queda en 2s.
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                if scale > 0 {
                    let p = 10u64.pow(scale);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); });
                    self.code.extend_from_slice(&[0x48, 0x99]);       // cqo
                    self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
                }
            } else {
                // rax = divisor, rdx = dividendo. Preescalar el dividendo
                // para que el cociente conserve la escala.
                self.code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax (divisor)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
                if scale > 0 {
                    let p = 10u64.pow(scale);
                    self.code.push(0x50); // push rax
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rax, p).unwrap(); });
                    self.code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                    self.code.push(0x58); // pop rax
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                }
                self.code.extend_from_slice(&[0x48, 0x99]);       // cqo
                self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
            }
        }
    }

    /// `factor := '(' suma ')' | operando`
    fn emit_expr_factor(&mut self, tokens: &[String], pos: &mut usize, scale: u32) {
        let Some(token) = tokens.get(*pos).cloned() else {
            self.errors.push(CobolError::new(0, "expresion COMPUTE incompleta"));
            return;
        };
        *pos += 1;

        if token == "(" {
            self.emit_expr_sum(tokens, pos, scale);
            if tokens.get(*pos).map(String::as_str) == Some(")") {
                *pos += 1;
            } else {
                self.errors
                    .push(CobolError::new(0, "falta ')' en la expresion COMPUTE"));
            }
            return;
        }

        if token == "-" {
            // Menos unario.
            self.emit_expr_factor(tokens, pos, scale);
            self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
            return;
        }

        if !self.is_variable(&token) && token.parse::<f64>().is_err() {
            self.errors.push(CobolError::new(
                0,
                format!("'{token}' no es un dato declarado ni un literal numerico"),
            ));
            return;
        }
        self.load_operand(&token, scale);
    }

    /// Compara los dos operandos y salta a `label` si la condición es FALSA.
    ///
    /// Es la primitiva de todo el flujo de control: un `IF` salta al `ELSE`
    /// cuando falla, y un `PERFORM UNTIL` salta al cuerpo cuando la
    /// condición de salida aún no se cumple.
    ///
    /// Deja `rdx` = izquierdo y `rax` = derecho, y compara `rdx - rax`, de
    /// modo que el código de condición se lee en el mismo orden que el
    /// COBOL: `A > B` es `jg`. La versión anterior cargaba los operandos
    /// cruzados y elegía condiciones invertidas a ojo — otra fuente de
    /// error que aquí desaparece.
    fn emit_jump_if_condition_false(&mut self, cond: &CobolCondition, label: u32) {
        let (a, b, cc_when_false) = match cond {
            CobolCondition::Equal(a, b) => (a, b, 0x85),          // jne
            CobolCondition::NotEqual(a, b) => (a, b, 0x84),       // je
            CobolCondition::Greater(a, b) => (a, b, 0x8E),        // jle
            CobolCondition::Less(a, b) => (a, b, 0x8D),           // jge
            CobolCondition::GreaterOrEqual(a, b) => (a, b, 0x8C), // jl
            CobolCondition::LessOrEqual(a, b) => (a, b, 0x8F),    // jg
        };

        let scale = self.comparison_scale(a, b);
        let (a, b) = (a.clone(), b.clone());

        self.load_operand(&a, scale);
        self.code.push(0x50); // push rax (izquierdo)
        self.load_operand(&b, scale); // rax = derecho
        self.code.push(0x5A); // pop rdx (izquierdo)
        self.code.extend_from_slice(&[0x48, 0x39, 0xC2]); // cmp rdx, rax
        self.emit_jcc(cc_when_false, label);
    }

    fn emit_mov_eax_syscall(&mut self, nr: u32) {
        self.code.extend_from_slice(&[0xB8]);
        self.code.extend_from_slice(&nr.to_le_bytes());
        self.emit_call_to_syscall_stub();
    }

    fn emit_v2_task_invoke(&mut self, operation: u64, args: &[String]) {
        self.emit_imm64_syscall_arg(0, surface::CURRENT_TASK);
        self.emit_imm64_syscall_arg(1, operation);
        for index in 0..4 {
            let value = args.get(index).and_then(|arg| arg.parse().ok()).unwrap_or(0);
            self.emit_imm64_syscall_arg(index + 2, value);
        }
        self.emit_mov_eax_syscall(surface::NR_INVOKE);
    }

    fn emit_imm64_syscall_arg(&mut self, index: usize, value: u64) {
        // mov rax, imm64 ; mov <arg_reg>, rax — todo por el encoder sem-asm.
        let dst = ARG_REGS[index];
        self.emit_asm(|a| {
            a.mov_imm64(Reg::Rax, value).unwrap();
            a.mov_reg(dst, Reg::Rax).unwrap();
        });
    }

    fn emit_call_to_syscall_stub(&mut self) {
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc { offset: self.code.len(), target: "__bmo_syscall_stub".to_string() });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn patch_string_fixups(&mut self) {
        let code_end = self.code.len();
        let mut str_off = code_end;
        for (idx, s) in self.strings.iter().enumerate() {
            for f in &self.str_fixups {
                if f.string_idx == idx {
                    let rip = f.lea_offset + 4;
                    let disp = str_off as i64 - rip as i64;
                    self.code[f.lea_offset..f.lea_offset + 4].copy_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            self.code.extend_from_slice(s.as_bytes());
            self.code.push(0);
            str_off += s.len() + 1;
        }
    }

    fn patch_call_relocs(&mut self) {
        for reloc in &self.call_relocs {
            if let Some(&t) = self.function_offsets.get(&reloc.target) {
                let d = t as i32 - (reloc.offset as i32 + 4);
                self.code[reloc.offset..reloc.offset + 4].copy_from_slice(&d.to_le_bytes());
            }
        }
    }

    fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(all));
        b.entry_offset = 0;
        b.build().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::Codegen;

    #[test]
    fn decimal_literals_scale_to_exact_cents() {
        // El alma bancaria: literal decimal → entero escalado exacto.
        assert_eq!(Codegen::scaled_imm("10.05", 2), 1005); // $10.05 → 1005 centavos
        assert_eq!(Codegen::scaled_imm("3.2", 2), 320);    // $3.20  → 320
        assert_eq!(Codegen::scaled_imm("7", 2), 700);      // $7.00  → 700
        assert_eq!(Codegen::scaled_imm("0.99", 2), 99);    // 99 centavos
        // Suma exacta de centavos, sin float:
        assert_eq!(
            Codegen::scaled_imm("10.05", 2) + Codegen::scaled_imm("3.20", 2),
            1325 // $13.25 exacto
        );
    }

    #[test]
    fn scale_zero_is_plain_integer() {
        // Enteros: comportamiento previo intacto (no rompe nada).
        assert_eq!(Codegen::scaled_imm("42", 0), 42);
        assert_eq!(Codegen::scaled_imm("1000", 0), 1000);
    }

    #[test]
    fn truncates_extra_decimals() {
        // COBOL sin ROUNDED trunca (no redondea).
        assert_eq!(Codegen::scaled_imm("1.999", 2), 199);
    }

    /// Un literal negativo tiene que llegar negativo. Sin esto, `MOVE -120.00`
    /// guardaba +12000 y el `CR` de un extracto no salía nunca.
    #[test]
    fn negative_literals_keep_their_sign() {
        assert_eq!(Codegen::scaled_imm("-120.00", 2) as i64, -12_000);
        assert_eq!(Codegen::scaled_imm("-7", 0) as i64, -7);
        assert_eq!(Codegen::scaled_imm("-0.05", 2) as i64, -5);
        // El `+` explícito sigue siendo positivo.
        assert_eq!(Codegen::scaled_imm("+120.00", 2) as i64, 12_000);
    }
}
