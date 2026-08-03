use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::syscalls::surface;
use bmo_sem_asm::Instructions;
use bmo_sem_asm::x86_64::{Asm, Reg};
use bmo_lower::x86;
use crate::ast::{CobolProgram, CobolStatement, CobolCondition, Condicion, DisplayArg, Redondeo};
use crate::ast::error::CobolError;
use crate::edicion::Plantilla;

type Result<T> = core::result::Result<T, CobolError>;

// BMO x86-64 SYSCALL argument registers: RDI, RSI, RDX, R10, R8, R9.
// RCX lo pisa el CPU con la dirección de retorno del usuario. Antes esto era
// la tabla REG_MOV de bytes a mano; ahora el mov reg,rax lo emite el encoder
// sem-asm (mismos bytes, leídos de la tabla TOML).
const ARG_REGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::R10, Reg::R8, Reg::R9];

pub fn compile_to_bef_bytes(program: &CobolProgram) -> Result<Vec<u8>> {
    // ★ Sin PROCEDURE DIVISION no hay programa.
    //
    // Antes, un fichero VACÍO —o uno con sólo WORKING-STORAGE— producía un
    // `.bex` de 4 192 bytes que se escribía sin quejarse. El punto de entrada
    // apuntaba al principio de una sección de código vacía.
    //
    // Es la misma clase de fallo que tenía C (un fichero vacío daba un BEF de
    // 8 240 bytes sin `main`) y se arregla por el mismo motivo: un binario con
    // punto de entrada inventado falla en el metal y no en la compilación, que
    // es donde se puede leer el porqué.
    //
    // Se comprueba aquí y no en el parser a propósito: el parser puede leer
    // legítimamente un programa sin sentencias mientras construye; quien no
    // puede entregar un binario vacío es el que lo escribe.
    // ★ "Sin nada que ejecutar" son las DOS listas vacías, no sólo la primera:
    // desde que hay párrafos, un programa puede tener el cuerpo principal vacío
    // y todo el trabajo dentro de ellos — que es una de las dos formas
    // corrientes de escribirlo.
    let sin_parrafos_con_algo = program.parrafos.iter().all(|p| p.statements.is_empty());
    if program.statements.is_empty() && sin_parrafos_con_algo {
        return Err(CobolError::new(0, format!(
            "'{}' no tiene PROCEDURE DIVISION con sentencias: un programa sin nada \
             que ejecutar no puede producir un binario",
            if program.program_id.is_empty() { "el programa" } else { &program.program_id },
        )));
    }
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
    /// Las variables `COMP-3`: cuántos bytes ocupa el empaquetado y si su PIC
    /// llevaba `S`.
    ///
    /// A diferencia de la edición, esto SÍ cambia el dato: el campo no guarda
    /// un entero de 64 bits, guarda nibbles. Por eso lo miran `load_var` y
    /// `store_var` —las dos únicas puertas a la memoria de una variable— y no
    /// se entera nadie más. La aritmética sigue viendo el entero escalado de
    /// siempre, que es lo que la mantiene exacta y ajena a la representación.
    var_packed: HashMap<String, (usize, bool)>,
    /// La PIC ya analizada de cada dato. La necesita el ÁREA: para escribir un
    /// campo zonado hay que saber cuántos dígitos declara y si lleva `S`, y eso
    /// no se puede sacar de la escala.
    pic_fields: HashMap<String, crate::pic::PicField>,
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
    /// Las TABLAS (`OCCURS`): cuántos elementos y cuántos bytes ocupa cada uno.
    ///
    /// El paso es el mismo hueco alineado que se le daría al dato suelto, así
    /// la regla de reparto de la pila es UNA: cada elemento vive donde viviría
    /// él solo. Un elemento y un dato suelto se cargan con el mismo `mov`.
    tablas: HashMap<String, (u32, i32)>,
    /// Los NOMBRES DE CONDICIÓN (nivel 88): apodo → (dato del que cuelga,
    /// valor con el que se compara). No ocupan memoria: son una comparación
    /// con nombre, y por eso viven en un mapa y no en la pila.
    cond_88: HashMap<String, (String, Vec<crate::ast::Valor88>)>,
    /// La etiqueta del bloque "subíndice fuera de rango" de cada tabla.
    ///
    /// Uno por tabla y no uno por acceso: el bloque termina el programa, así
    /// que se llega por `jmp` y nadie vuelve. Cada acceso cuesta doce bytes
    /// (un `cmp` y un `jae`) en vez de arrastrar el mensaje entero.
    oob_labels: HashMap<String, u32>,
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
    /// La ranura de pila donde vive "en qué párrafo hay que volver".
    ///
    /// `None` = el programa no tiene párrafos y no se reserva nada. Ver
    /// `emit_parrafos` para por qué un número en memoria y no un `ret` a secas.
    perform_exit: Option<i32>,
    /// La disposición de los registros: qué byte ocupa cada campo dentro de su
    /// `01`. Ver `registro.rs`.
    disposicion: crate::registro::Disposicion,
    /// El ÁREA DE REGISTRO de cada `01` que es un grupo: la ranura de pila
    /// donde viven sus bytes.
    ///
    /// Es el camino B de `PLAN_BANCA.md` §1.0: el área es la representación
    /// EXTERNA —lo que va y viene del disco— y cada campo conserva además su
    /// ranura de trabajo de 64 bits. La traducción entre las dos vive
    /// exactamente en los puntos donde el registro cruza, que es lo que dice
    /// COBOL: el área sólo vale entre un `READ` y el siguiente.
    areas: HashMap<String, i32>,
    /// Los párrafos por nombre → su número de orden (1..n). El **orden manda**:
    /// un `PERFORM A THRU B` ejecuta todo lo que hay entre los dos, así que
    /// comparar índices es lo que dice si el rango tiene sentido.
    parrafos: HashMap<String, u32>,
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
            var_packed: HashMap::new(),
            pic_fields: HashMap::new(),
            files: HashMap::new(),
            file_handles: HashMap::new(),
            file_estado: HashMap::new(),
            record_owner: HashMap::new(),
            tablas: HashMap::new(),
            cond_88: HashMap::new(),
            oob_labels: HashMap::new(),
            next_label: 0,
            label_offsets: HashMap::new(),
            jump_fixups: Vec::new(),
            perform_exit: None,
            disposicion: Default::default(),
            areas: HashMap::new(),
            parrafos: HashMap::new(),
            errors: Vec::new(),
            stack_size: 0,
            isa: Instructions::load_x86_64().expect("tablas sem-asm x86-64 (forge/sem-asm/tables)"),
        }
    }

    /// Escala decimal de una variable (0 = entero / no declarada).
    /// La escala de un dato — y de un elemento de tabla es la de su tabla: el
    /// `OCCURS` repite el campo, no cambia dónde cae la coma.
    fn var_scale(&self, name: &str) -> u32 {
        *self.var_scales.get(&Self::nombre_base(name)).unwrap_or(&0)
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
        self.load_scaled_imm_redondeado(lit, scale, Redondeo::Truncar);
    }

    /// Igual, pero aplicando el modo a los decimales que no caben.
    ///
    /// Se resuelve **al compilar**, con la misma regla que el código emitido:
    /// `bmo_lower::redondeo::dividir_en_rust` es la hermana de `dividir`, y hay
    /// un test que las compara valor a valor. Un literal nunca llega a
    /// ejecutarse — se convierte en un inmediato antes— así que si las dos
    /// reglas divergieran, `ADD 1.005` daría una cosa y `ADD IMPORTE` otra con
    /// el mismo número dentro.
    fn load_scaled_imm_redondeado(&mut self, lit: &str, scale: u32, redondeo: Redondeo) {
        let v = if redondeo == Redondeo::Truncar {
            Self::scaled_imm(lit, scale)
        } else {
            // Un dígito de más, y luego la regla. Es la única forma de saber si
            // había que subir: con la escala justa, el dígito que lo decide ya
            // se tiró.
            let con_uno_mas = Self::scaled_imm(lit, scale + 1) as i64;
            bmo_lower::redondeo::dividir_en_rust(con_uno_mas, 10, redondeo) as u64
        };
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
        match Self::subindice(name) {
            Some((base, _)) => self.var_offsets.contains_key(&base),
            None => self.var_offsets.contains_key(name),
        }
    }

    // ── TABLAS (`OCCURS`) ────────────────────────────────────────────────
    //
    // Un elemento de tabla se escribe `TOTAL(I)` y se guarda como cualquier
    // otro dato: un entero escalado en un hueco de la pila. Lo único que
    // cambia es que la DIRECCIÓN se calcula, y por eso todo pasa por los
    // mismos cuatro sitios que un dato suelto (`is_variable`, `var_scale`,
    // `load_var`, `store_var`). Así `MOVE`, `ADD`, `IF` y `DISPLAY` heredan los
    // subíndices sin tocar ni una línea de sus emisores.

    /// Parte `TOTAL(I)` en `("TOTAL", "I")`. `None` si no lleva subíndice.
    ///
    /// El subíndice puede ser un literal o el nombre de un dato — que es como
    /// COBOL recorre una tabla, con la variable del bucle.
    fn subindice(name: &str) -> Option<(String, String)> {
        let abre = name.find('(')?;
        let cierra = name.rfind(')')?;
        if cierra < abre {
            return None;
        }
        let base = name[..abre].trim().to_string();
        let idx = name[abre + 1..cierra].trim().to_string();
        if base.is_empty() || idx.is_empty() {
            return None;
        }
        Some((base, idx))
    }

    /// El nombre sin subíndice — para preguntar por la escala o la edición, que
    /// son de la tabla entera y no de un elemento.
    fn nombre_base(name: &str) -> String {
        Self::subindice(name).map(|(b, _)| b).unwrap_or_else(|| name.to_string())
    }

    /// La plantilla de edición del dato, mirando por su nombre base.
    fn edicion_de(&self, name: &str) -> Option<Plantilla> {
        self.var_edicion.get(&Self::nombre_base(name)).cloned()
    }

    /// Deja en `rcx` la DIRECCIÓN del elemento, o `None` si algo no cuadra.
    ///
    /// Ensucia `rax`, `rcx` y `rdx`. Los llamantes que traigan un valor vivo en
    /// `rax` lo apilan antes: eso es cosa de `store_var`.
    fn emit_direccion_elemento(&mut self, base: &str, idx: &str) -> Option<()> {
        let Some(&off) = self.var_offsets.get(base) else {
            self.errors
                .push(CobolError::new(0, format!("'{base}' no esta declarado en el DATA DIVISION")));
            return None;
        };
        let Some(&(n, paso)) = self.tablas.get(base) else {
            self.errors.push(CobolError::new(
                0,
                format!("'{base}' no es una tabla: solo se le puede poner subindice a un OCCURS"),
            ));
            return None;
        };

        // ── El subíndice literal se resuelve al COMPILAR ──
        //
        // Es el caso corriente (`TOTAL(1)`) y sale gratis: ni multiplicación ni
        // comprobación en ejecución. Y si se sale de la tabla, no compila: un
        // `TOTAL(13)` sobre doce elementos es un error del programa, no una
        // desgracia que descubrir de noche.
        if let Ok(fijo) = idx.parse::<i64>() {
            if fijo < 1 || fijo > n as i64 {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "{base}({fijo}) se sale: la tabla tiene {n} elementos, \
                         asi que el subindice va de 1 a {n}"
                    ),
                ));
                return None;
            }
            let elem = off + (fijo as i32 - 1) * paso;
            // lea rcx, [rbp + elem]
            self.code.extend_from_slice(&[0x48, 0x8D, 0x8D]);
            self.code.extend_from_slice(&elem.to_le_bytes());
            return Some(());
        }

        // ── El subíndice variable se resuelve en EJECUCIÓN ──
        let escala = self.var_scale(idx);
        if escala != 0 {
            self.errors.push(CobolError::new(
                0,
                format!(
                    "el subindice {idx} tiene decimales (escala {escala}): \
                     un elemento de tabla se cuenta con enteros"
                ),
            ));
            return None;
        }
        self.load_operand(idx, 0); // rax = el subindice, base 1
        self.code.extend_from_slice(&[0x48, 0xFF, 0xC8]); // dec rax → base 0

        // El guarda. `jae` (SIN signo) coge los dos lados con UNA comparacion:
        // el subindice 0 se convirtio en -1, que sin signo es enorme.
        let fuera = *self
            .oob_labels
            .entry(base.to_string())
            .or_insert_with(|| {
                let l = self.next_label;
                self.next_label += 1;
                l
            });
        // `cmp r/m64, imm32` (grupo 81 /7) y no la forma corta `3D`: es la que
        // ya emite el resto del sistema, o sea la que el emulador EJECUTA. Un
        // opcode nuevo aqui seria una forma mas que mantener en dos sitios.
        self.code.extend_from_slice(&[0x48, 0x81, 0xF8]);
        self.code.extend_from_slice(&(n as i32).to_le_bytes());
        self.emit_jcc(0x83, fuera); // jae (SIN signo) → fuera de rango

        // rax *= paso, por `mov rcx, paso` + `imul rax, rcx` — la misma pareja
        // que usa `rescale`, en vez de un `imul rax, imm32` que nadie mas emite.
        self.emit_asm(|a| {
            a.mov_imm64(Reg::Rcx, paso as u64).unwrap();
        });
        self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]); // imul rax, rcx
        // lea rcx,[rbp+off] ; add rcx, rax
        self.code.extend_from_slice(&[0x48, 0x8D, 0x8D]);
        self.code.extend_from_slice(&off.to_le_bytes());
        self.code.extend_from_slice(&[0x48, 0x01, 0xC1]);
        Some(())
    }

    /// Los bloques de "subindice fuera de rango", uno por tabla.
    ///
    /// Termina el programa DICIENDO qué tabla fue. La alternativa era seguir
    /// con una direccion inventada: en un batch eso escribe encima del campo
    /// de al lado y el descuadre aparece semanas despues, en otro sitio. Un
    /// proceso bancario que se para y lo cuenta es infinitamente mejor que uno
    /// que sigue con la memoria de otro.
    fn emit_bloques_fuera_de_rango(&mut self) {
        for (base, label) in std::mem::take(&mut self.oob_labels) {
            let (n, _) = self.tablas[&base];
            self.bind_label(label);
            let msg = format!("SUBINDICE FUERA DE RANGO EN {base} (1..{n})\n");
            bmo_lower::console::write_const(&mut self.code, msg.as_bytes());
            bmo_lower::task::exit(&mut self.code);
        }
    }

    /// Carga un operando en `rax`, reescalado a `scale`.
    ///
    /// El reescalado es lo que hace que el decimal siga siendo exacto al
    /// mezclar datos de distinta PIC: sumar un `PIC 9(3)` (escala 0) a un
    /// `PIC 9(3)V99` (escala 2) exige multiplicar por 100 primero, si no se
    /// sumarían centavos con pesos.
    fn load_operand(&mut self, name: &str, scale: u32) {
        self.load_operand_redondeado(name, scale, Redondeo::Truncar);
    }

    /// La escala DECIMAL de un operando, sea dato o literal.
    ///
    /// Hace falta para elegir la escala en la que se calcula: **la operación se
    /// hace donde no se pierde nada, y se redondea AL FINAL**. Cargar un
    /// `1.005` en un campo de dos decimales antes de sumarlo redondearía el
    /// OPERANDO en vez del RESULTADO, y con los modos asimétricos eso da otro
    /// número — el techo de `-9.995` es `-9.99`, pero si primero se redondea el
    /// `9.995` a `10.00` sale `-10.00`.
    fn escala_operando(&self, name: &str) -> u32 {
        if self.is_variable(name) {
            return self.var_scale(name);
        }
        name.trim()
            .split_once('.')
            .map(|(_, frac)| frac.chars().take_while(|c| c.is_ascii_digit()).count() as u32)
            .unwrap_or(0)
    }

    /// Igual, pero diciendo qué hacer con los decimales que no caben.
    fn load_operand_redondeado(&mut self, name: &str, scale: u32, redondeo: Redondeo) {
        if self.is_variable(name) {
            let from = self.var_scale(name);
            self.load_var(name);
            self.rescale_redondeado(from, scale, redondeo);
        } else {
            self.load_scaled_imm_redondeado(name, scale, redondeo);
        }
    }

    /// Lleva `rax` de la escala `from` a la escala `to`, truncando lo que sobre.
    fn rescale(&mut self, from: u32, to: u32) {
        self.rescale_redondeado(from, to, Redondeo::Truncar);
    }

    fn rescale_redondeado(&mut self, from: u32, to: u32, redondeo: Redondeo) {
        if from == to {
            return;
        }
        if to > from {
            // Subir de escala es EXACTO: multiplicar por una potencia de diez no
            // pierde nada, así que aquí no hay nada que redondear.
            let factor = 10u64.pow(to - from);
            self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, factor).unwrap(); });
            self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]); // imul rax, rcx
        } else {
            let factor = 10u64.pow(from - to);
            self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, factor).unwrap(); });
            bmo_lower::redondeo::dividir(&mut self.code, redondeo);
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
        // ★ La disposición ANTES de repartir la pila: dice qué grupos hay y
        // cuánto mide el área de cada uno. Y de paso caza los nombres
        // duplicados, que sin ella se resolvían quedándose con uno de los dos.
        self.disposicion = crate::registro::calcular(&program.data_items)?;
        for item in &program.data_items {
            // ★ Un 88 NO es un dato: no se le reserva ni un byte. Es el apodo
            // de una comparación sobre el campo del que cuelga.
            if item.level == 88 {
                if let Some(padre) = &item.padre {
                    self.cond_88.insert(
                        item.name.to_ascii_uppercase(),
                        (padre.clone(), item.valores.clone()),
                    );
                }
                continue;
            }
            let size = item.storage_size();
            let aligned = (size as i32 + 7) & !7;
            // Una tabla son `n` huecos seguidos. El offset que se guarda es el
            // del PRIMER elemento, así que se reserva todo y se apunta al
            // principio: los offsets crecen hacia abajo (`-stack_size`) y el
            // subíndice suma hacia arriba.
            let n = item.elementos();
            self.stack_size += aligned * n as i32;
            if item.occurs.is_some() {
                self.tablas.insert(item.name.clone(), (n, aligned));
            }
            self.var_offsets.insert(item.name.clone(), -(self.stack_size));
            // Recuerda la escala decimal del PIC para la aritmética exacta.
            self.var_scales.insert(item.name.clone(), item.scale());
            if let Some(p) = &item.edicion {
                self.var_edicion.insert(item.name.clone(), p.clone());
            }
            // ★ COMP-3: el hueco reservado ya es el del empaquetado (lo da
            // `storage_size`), y aqui se apunta COMO se lee y se escribe.
            if item.usage == crate::pic::Usage::Comp3 {
                if let Some(campo) = &item.pic_field {
                    self.var_packed.insert(item.name.clone(), (campo.size(), campo.signed));
                }
            }
            if let Some(campo) = &item.pic_field {
                self.pic_fields.insert(item.name.to_ascii_uppercase(), campo.clone());
            }
        }
        // ★ El ÁREA DE REGISTRO de cada `01` que es un grupo. Va DESPUÉS de los
        // datos y con el mismo mecanismo: es una variable más, sólo que su
        // contenido son los bytes tal cual irían al disco.
        for raiz in self.disposicion.raices().to_vec() {
            let Some(campo) = self.disposicion.campo(&raiz) else { continue };
            if !campo.es_grupo || campo.bytes == 0 {
                continue;
            }
            let bytes = ((campo.bytes as i32) + 7) & !7;
            self.stack_size += bytes;
            self.areas.insert(raiz.clone(), -(self.stack_size));
        }

        // ★ La ranura del PERFORM: en qué párrafo tiene que volver el que está
        // corriendo ahora. Vive en la pila y no en un registro por la razón de
        // siempre — entre el `call` y el `ret` pasa el párrafo entero, y
        // cualquier `DISPLAY` de dentro hace un `syscall` que destruye medio
        // banco de registros.
        if !program.parrafos.is_empty() {
            self.stack_size += 8;
            self.perform_exit = Some(-(self.stack_size));
            for (i, p) in program.parrafos.iter().enumerate() {
                self.parrafos.insert(p.nombre.to_ascii_uppercase(), (i + 1) as u32);
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

        // ★ Los VALUE, antes de la primera sentencia y despues de repartir la
        // pila: el valor inicial de un dato tiene que estar puesto ANTES de que
        // el programa mire nada, y la direccion donde ponerlo no existe hasta
        // que el reparto esta hecho.
        self.emit_valores_iniciales(program);

        // ── El CUERPO PRINCIPAL ──
        //
        // Si el programa empieza directamente con un párrafo (la otra forma
        // corriente de escribirlo), el cuerpo principal está vacío y arrancar
        // seria salir sin hacer nada. En ese caso se ejecuta el PRIMER párrafo,
        // que es lo que dice el estándar: la PROCEDURE DIVISION se recorre de
        // arriba abajo.
        if program.statements.is_empty() && !program.parrafos.is_empty() {
            let primero = program.parrafos[0].nombre.clone();
            self.emit_statement(&CobolStatement::PerformFuera {
                desde: primero,
                hasta: program.parrafos.last().map(|p| p.nombre.clone()),
                veces: None,
                hasta_que: None,
            });
        }
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

        // ★ Los PÁRRAFOS, después del final del cuerpo principal.
        self.emit_parrafos(program);

        // Los bloques de subindice fuera de rango van DESPUES del final: son
        // camino de no volver, no estorban al codigo que corre, y se comparten
        // entre todos los accesos a la misma tabla.
        self.emit_bloques_fuera_de_rango();

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

    // ── PÁRRAFOS ────────────────────────────────────────────────────────
    //
    // ## Por qué un número en memoria y no un `ret` a secas
    //
    // Si cada párrafo terminara en `ret`, `PERFORM A` funcionaría y
    // `PERFORM A THRU C` no: al acabar `A` volvería en vez de seguir por `B`.
    // Y no se puede decidir al compilar cuál de las dos cosas hace `A`, porque
    // el MISMO párrafo puede ser el final de un rango en una línea y estar en
    // medio de otro en la de abajo.
    //
    // Así que la decisión se toma en EJECUCIÓN, con una pregunta de dos
    // instrucciones al final de cada párrafo:
    //
    // ```text
    //   PERFORM A THRU C:                    fin de cada parrafo P:
    //     push [salida]      guardar           cmp [salida], <id de P>
    //     mov  [salida], id(C)                 jne (caer al siguiente)
    //     call A                               ret
    //     pop  [salida]      restaurar
    // ```
    //
    // El `push`/`pop` alrededor es lo que deja que un párrafo llame a otro: la
    // salida del de fuera se guarda en la pila de máquina, debajo de la
    // dirección de retorno, y vuelve a su sitio al terminar. Sin eso, un
    // `PERFORM` anidado se comería la salida del que lo contiene y el de fuera
    // no volvería nunca.
    //
    // Es la misma forma que usa GnuCOBOL, y por el mismo motivo.

    /// El nombre con el que un párrafo entra en `function_offsets`.
    fn simbolo_parrafo(nombre: &str) -> String {
        // El `:` es ilegal en un nombre de COBOL, así que un párrafo llamado
        // como un símbolo interno no puede chocar. Mismo truco que el punto de
        // `funcion.variable` en BMO C.
        format!("parrafo:{}", nombre.to_ascii_uppercase())
    }

    /// `push qword [rbp+off]`.
    fn emit_push_mem(&mut self, off: i32) {
        self.code.extend_from_slice(&[0xFF, 0xB5]);
        self.code.extend_from_slice(&off.to_le_bytes());
    }

    /// `pop qword [rbp+off]`.
    fn emit_pop_mem(&mut self, off: i32) {
        self.code.extend_from_slice(&[0x8F, 0x85]);
        self.code.extend_from_slice(&off.to_le_bytes());
    }

    /// `mov qword [rbp+off], imm32`.
    fn emit_store_imm_mem(&mut self, off: i32, valor: u32) {
        self.code.extend_from_slice(&[0x48, 0xC7, 0x85]);
        self.code.extend_from_slice(&off.to_le_bytes());
        self.code.extend_from_slice(&valor.to_le_bytes());
    }

    /// `cmp qword [rbp+off], imm32`.
    fn emit_cmp_mem_imm(&mut self, off: i32, valor: u32) {
        self.code.extend_from_slice(&[0x48, 0x81, 0xBD]);
        self.code.extend_from_slice(&off.to_le_bytes());
        self.code.extend_from_slice(&valor.to_le_bytes());
    }

    /// Los párrafos, en el orden en que se escribieron.
    ///
    /// El orden no es estético: un `PERFORM A THRU C` **cae** de `A` a `B` y de
    /// `B` a `C` porque están seguidos en el código. Reordenarlos cambiaría lo
    /// que hace el programa.
    fn emit_parrafos(&mut self, program: &CobolProgram) {
        let Some(salida) = self.perform_exit else { return };
        for (i, p) in program.parrafos.iter().enumerate() {
            let id = (i + 1) as u32;
            let off = self.code.len();
            self.function_offsets.insert(Self::simbolo_parrafo(&p.nombre), off);
            for s in &p.statements {
                self.emit_statement(s);
            }
            // El epílogo: ¿es aquí donde había que volver?
            let sigue = self.fresh_label();
            self.emit_cmp_mem_imm(salida, id);
            self.emit_jcc(0x85, sigue); // jne → cae al párrafo siguiente
            self.code.push(0xC3); // ret
            self.bind_label(sigue);
        }
        // Detrás del último no hay párrafo al que caer. Llegar aquí querría
        // decir que la salida apuntaba a uno que ya pasó; el `ret` devuelve el
        // control a quien llamara, que es lo menos malo y no inventa un salto.
        self.code.push(0xC3);
    }

    /// `PERFORM <párrafo> [THRU <otro>] [<n> TIMES | UNTIL <cond>]`.
    fn emit_perform_fuera(
        &mut self,
        desde: &str,
        hasta: Option<&str>,
        veces: Option<u32>,
        hasta_que: Option<&Condicion>,
    ) {
        let Some(salida) = self.perform_exit else {
            self.errors.push(CobolError::new(
                0,
                format!("PERFORM {desde}: este programa no tiene ningun parrafo"),
            ));
            return;
        };
        let Some(&i_desde) = self.parrafos.get(&desde.to_ascii_uppercase()) else {
            self.errors.push(CobolError::new(
                0,
                format!(
                    "PERFORM {desde}: no hay ningun parrafo con ese nombre. Un parrafo se \
                     declara escribiendo su nombre solo y con punto, en su propia linea"
                ),
            ));
            return;
        };
        let i_hasta = match hasta {
            None => i_desde,
            Some(h) => match self.parrafos.get(&h.to_ascii_uppercase()) {
                Some(&i) => i,
                None => {
                    self.errors.push(CobolError::new(
                        0,
                        format!("PERFORM {desde} THRU {h}: no hay ningun parrafo llamado {h}"),
                    ));
                    return;
                }
            },
        };
        // Un rango al reves no es un rango. El estandar lo deja "indefinido", y
        // aqui indefinido significa que el programa se sale de los parrafos y
        // ejecuta lo que haya detras — asi que se dice.
        if i_hasta < i_desde {
            self.errors.push(CobolError::new(
                0,
                format!(
                    "PERFORM {desde} THRU {}: el final esta ANTES del principio. Un rango \
                     va hacia abajo, en el orden en que estan escritos los parrafos",
                    hasta.unwrap_or("")
                ),
            ));
            return;
        }

        // El cuerpo: guardar la salida de fuera, fijar la nuestra, llamar, y
        // devolverla. Va en un cierre para poder envolverlo en el bucle que
        // toque sin repetirlo tres veces.
        let simbolo = Self::simbolo_parrafo(desde);
        macro_rules! llamada {
            ($yo:expr) => {{
                $yo.emit_push_mem(salida);
                $yo.emit_store_imm_mem(salida, i_hasta);
                $yo.code.push(0xE8); // call rel32
                $yo.call_relocs
                    .push(CallReloc { offset: $yo.code.len(), target: simbolo.clone() });
                $yo.code.extend_from_slice(&[0, 0, 0, 0]);
                $yo.emit_pop_mem(salida);
            }};
        }

        match (veces, hasta_que) {
            (None, None) => llamada!(self),

            // `PERFORM P <n> TIMES` — el contador en la pila, igual que el
            // PERFORM en línea, y por el mismo motivo: el párrafo de dentro
            // puede hacer un `syscall` y llevarse los registros por delante.
            (Some(n), None) => {
                let top = self.fresh_label();
                let done = self.fresh_label();
                self.emit_asm(|a| { a.mov_imm64(Reg::Rax, n as u64).unwrap(); });
                self.code.push(0x50); // push rax → contador
                self.bind_label(top);
                self.code.extend_from_slice(&[0x48, 0x83, 0x3C, 0x24, 0x00]); // cmp qword [rsp], 0
                self.emit_jcc(0x8E, done); // jle
                llamada!(self);
                self.code.extend_from_slice(&[0x48, 0xFF, 0x0C, 0x24]); // dec qword [rsp]
                self.emit_jmp(top);
                self.bind_label(done);
                self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
            }

            // `PERFORM P UNTIL <cond>` — se prueba ANTES de cada vuelta
            // (`WITH TEST BEFORE`, el default). Es EL bucle de un batch: el
            // párrafo lee un registro y el UNTIL mira si se acabó.
            (None, Some(cond)) => {
                let top = self.fresh_label();
                let cuerpo = self.fresh_label();
                let done = self.fresh_label();
                self.bind_label(top);
                let cond = cond.clone();
                self.emit_jump_if_false(&cond, cuerpo);
                self.emit_jmp(done);
                self.bind_label(cuerpo);
                llamada!(self);
                self.emit_jmp(top);
                self.bind_label(done);
            }

            (Some(_), Some(_)) => {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "PERFORM {desde}: `<n> TIMES` y `UNTIL` a la vez no se compila. \
                         Son dos formas de decir cuando parar y hay que elegir una"
                    ),
                ));
            }
        }
    }

    // ── EL ÁREA DE REGISTRO ─────────────────────────────────────────────
    //
    // Camino B de `PLAN_BANCA.md` §1.0. Un grupo tiene DOS representaciones:
    //
    //   las RANURAS de trabajo   un entero escalado de 64 bits por campo,
    //                            que es donde se calcula
    //   el ÁREA                  los bytes tal cual irían al disco: zonado
    //                            para un DISPLAY, nibbles para un COMP-3
    //
    // Y la traducción entre las dos vive **sólo** donde el registro cruza:
    // empaquetar antes de sacarlo, desempaquetar después de traerlo. Eso no es
    // un rodeo — es lo que dice COBOL, que el área de registro sólo vale entre
    // un `READ` y el siguiente.
    //
    // Lo que se paga, dicho en el plan: un `REDEFINES` no aliasa de verdad,
    // porque dos vistas del mismo espacio serían dos juegos de ranuras.

    /// `lea rcx, [rbp + area(raiz) + offset]` — dónde vive un campo dentro de
    /// su área.
    fn emit_direccion_en_area(&mut self, raiz: &str, offset: u32) -> Option<()> {
        let base = *self.areas.get(raiz)?;
        self.emit_direccion_dato(base + offset as i32);
        Some(())
    }

    /// Vuelca las ranuras de trabajo del grupo a su área, campo por campo.
    fn emit_empaquetar_area(&mut self, raiz: &str) {
        let hojas: Vec<(String, crate::registro::Campo)> = self
            .disposicion
            .hojas_de(raiz)
            .into_iter()
            .map(|(n, c)| (n.clone(), c.clone()))
            .collect();
        for (nombre, campo) in hojas {
            self.load_var(&nombre);
            if self.emit_direccion_en_area(raiz, campo.offset).is_none() {
                return;
            }
            match self.packed_de(&nombre) {
                Some((bytes, signo)) => {
                    bmo_lower::packed::empaquetar(&mut self.code, bytes, signo)
                }
                // Un `DISPLAY` en el área es ZONADO: un byte por dígito y el
                // signo sobrepunzado en el último. Dentro sigue siendo el mismo
                // entero escalado de siempre.
                None => {
                    let (digitos, signo) = self.digitos_de(&nombre);
                    bmo_lower::zoned::escribir(&mut self.code, digitos, signo);
                }
            }
        }
    }

    /// Y al revés: del área a las ranuras.
    fn emit_desempaquetar_area(&mut self, raiz: &str) {
        let hojas: Vec<(String, crate::registro::Campo)> = self
            .disposicion
            .hojas_de(raiz)
            .into_iter()
            .map(|(n, c)| (n.clone(), c.clone()))
            .collect();
        for (nombre, campo) in hojas {
            if self.emit_direccion_en_area(raiz, campo.offset).is_none() {
                return;
            }
            match self.packed_de(&nombre) {
                Some((bytes, _)) => bmo_lower::packed::desempaquetar(&mut self.code, bytes),
                None => {
                    let (digitos, _) = self.digitos_de(&nombre);
                    bmo_lower::zoned::leer(&mut self.code, digitos);
                }
            }
            self.store_var(&nombre);
        }
    }

    /// Cuántos dígitos declara la PIC de un campo, y si lleva `S`.
    fn digitos_de(&self, nombre: &str) -> (u32, bool) {
        self.pic_fields
            .get(&Self::nombre_base(nombre))
            .map(|f| (f.total_digits(), f.signed))
            .unwrap_or((1, false))
    }

    /// `MOVE <grupo> TO <grupo>` — **una copia de BYTES**, no campo a campo.
    ///
    /// Y eso importa: el estándar dice que un `MOVE` de grupo mueve el área tal
    /// cual, sin mirar qué hay dentro. Hacerlo campo a campo daría otra cosa en
    /// cuanto los dos grupos no tuvieran la misma forma — que es justo el caso
    /// en el que un programa de banca lo usa, para reinterpretar un registro.
    fn emit_move_grupo(&mut self, origen: &str, destino: &str) {
        let (o, d) = (origen.to_ascii_uppercase(), destino.to_ascii_uppercase());
        let (Some(co), Some(cd)) = (
            self.disposicion.campo(&o).cloned(),
            self.disposicion.campo(&d).cloned(),
        ) else {
            return;
        };
        // Se mueve lo que quepa en el destino, que es lo que manda el estándar:
        // si el origen es más corto, lo que sobra del destino se queda como
        // estaba (rellenar con espacios pide el texto, que es la tarea 0.7).
        let n = co.bytes.min(cd.bytes);
        let (Some(&base_o), Some(&base_d)) = (self.areas.get(&o), self.areas.get(&d)) else {
            self.errors.push(CobolError::new(
                0,
                format!("MOVE {origen} TO {destino}: alguno de los dos no tiene area de registro"),
            ));
            return;
        };

        self.emit_empaquetar_area(&o);
        // rdi = destino, rsi = origen, rcx = cuántos. El contrato de `copiar`.
        self.code.extend_from_slice(&[0x48, 0x8D, 0xBD]); // lea rdi, [rbp+disp32]
        self.code.extend_from_slice(&base_d.to_le_bytes());
        self.code.extend_from_slice(&[0x48, 0x8D, 0xB5]); // lea rsi, [rbp+disp32]
        self.code.extend_from_slice(&base_o.to_le_bytes());
        self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, n as u64).unwrap(); });
        bmo_lower::memoria::copiar(&mut self.code);
        self.emit_desempaquetar_area(&d);
    }

    /// `VALUE` — el valor con el que arranca cada dato.
    ///
    /// Se emite como una tanda de `MOVE` implicitos al principio del programa,
    /// y **pasa por `store_var`** a proposito: es la unica puerta a la memoria
    /// de una variable, asi que un campo `COMP-3` se inicializa EMPAQUETADO sin
    /// que esta funcion tenga que saber que existen los nibbles.
    ///
    /// Un `VALUE` sobre una tabla inicializa **todos** los elementos, que es lo
    /// que dice el estandar. Se emite un `store` por casilla en vez de un bucle
    /// porque el numero se sabe al compilar y una tabla de banca son doce meses
    /// o cuatro conceptos, no un millon.
    fn emit_valores_iniciales(&mut self, program: &CobolProgram) {
        for item in &program.data_items {
            // Un 88 no es un dato: su VALUE es el valor con el que COMPARA, no
            // uno que guardar. No tiene memoria donde ponerlo.
            if item.level == 88 {
                continue;
            }
            let Some(valor) = item.value.clone() else { continue };
            let escala = item.scale();
            match item.occurs {
                None => {
                    self.load_scaled_imm(&valor, escala);
                    self.store_var(&item.name);
                }
                Some(n) => {
                    for i in 1..=n {
                        self.load_scaled_imm(&valor, escala);
                        self.store_var(&format!("{}({})", item.name, i));
                    }
                }
            }
        }
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

    /// Antes, un nombre que no existía hacía que estas dos funciones no
    /// emitieran NADA: `DISPLAY PEPE` imprimía lo que hubiera en `rax` y
    /// `MOVE 1 TO PEPE` se perdía. Ahora falta el dato o falta el subíndice, y
    /// las dos cosas se dicen.
    fn exige_declarado(&mut self, name: &str) -> Option<i32> {
        match self.var_offsets.get(name) {
            Some(&off) if !self.tablas.contains_key(name) => Some(off),
            Some(_) => {
                let n = self.tablas[name].0;
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "'{name}' es una tabla de {n}: hace falta el subindice, \
                         `{name}(I)`. Sin el no se sabe de que elemento se habla"
                    ),
                ));
                None
            }
            None => {
                self.errors.push(CobolError::new(
                    0,
                    format!("'{name}' no esta declarado en el DATA DIVISION"),
                ));
                None
            }
        }
    }

    /// El empaquetado de esta variable, si lo tiene: (bytes, con signo).
    fn packed_de(&self, name: &str) -> Option<(usize, bool)> {
        self.var_packed.get(&Self::nombre_base(name)).copied()
    }

    /// `lea rcx, [rbp + off]` — la dirección de un dato suelto.
    ///
    /// Un dato normal no necesita su dirección: se lee y se escribe con `mov`
    /// sobre `[rbp+off]` directamente. Un COMP-3 sí, porque el emisor de
    /// nibbles trabaja sobre un puntero — el mismo que ya le da
    /// `emit_direccion_elemento` a un elemento de tabla.
    fn emit_direccion_dato(&mut self, off: i32) {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x8D]);
        self.code.extend_from_slice(&off.to_le_bytes());
    }

    fn load_var(&mut self, name: &str) {
        let packed = self.packed_de(name);
        if let Some((base, idx)) = Self::subindice(name) {
            if self.emit_direccion_elemento(&base, &idx).is_some() {
                match packed {
                    Some((bytes, _)) => bmo_lower::packed::desempaquetar(&mut self.code, bytes),
                    None => self.code.extend_from_slice(&[0x48, 0x8B, 0x01]), // mov rax, [rcx]
                }
            }
            return;
        }
        let Some(off) = self.exige_declarado(name) else { return };
        if let Some((bytes, _)) = packed {
            self.emit_direccion_dato(off);
            bmo_lower::packed::desempaquetar(&mut self.code, bytes);
            return;
        }
        if off >= -128 && off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x8B, 0x45, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    fn store_var(&mut self, name: &str) {
        let packed = self.packed_de(name);
        if let Some((base, idx)) = Self::subindice(name) {
            // El valor que hay que guardar viene en `rax`, y calcular la
            // direccion lo destruye: se aparta en la pila. Apilar y no en otro
            // registro porque el subindice puede ser OTRO elemento de tabla, y
            // ese calculo vuelve a usar los mismos tres registros.
            self.code.push(0x50); // push rax
            let ok = self.emit_direccion_elemento(&base, &idx).is_some();
            self.code.push(0x58); // pop rax
            if ok {
                match packed {
                    Some((bytes, signo)) => {
                        bmo_lower::packed::empaquetar(&mut self.code, bytes, signo)
                    }
                    None => self.code.extend_from_slice(&[0x48, 0x89, 0x01]), // mov [rcx], rax
                }
            }
            return;
        }
        let Some(off) = self.exige_declarado(name) else { return };
        if let Some((bytes, signo)) = packed {
            // La direccion se calcula en `rcx`, que el emisor de nibbles NO
            // toca — asi el valor sigue en `rax` sin apilar nada.
            self.emit_direccion_dato(off);
            bmo_lower::packed::empaquetar(&mut self.code, bytes, signo);
            return;
        }
        if off >= -128 && off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x89, 0x45, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
            self.code.extend_from_slice(&off.to_le_bytes());
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
        if let Some(p) = self.edicion_de(nombre) {
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
        let plantilla = self.edicion_de(&reg);
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
                // ★ Un MOVE de GRUPO no es un MOVE de campo con otro nombre:
                // mueve el ÁREA tal cual, sin mirar qué hay dentro. Se
                // distingue aquí porque sólo el codegen sabe quién es grupo.
                if self.disposicion.es_grupo(src) && self.disposicion.es_grupo(dst) {
                    let (src, dst) = (src.clone(), dst.clone());
                    self.emit_move_grupo(&src, &dst);
                    return;
                }
                if self.disposicion.es_grupo(src) || self.disposicion.es_grupo(dst) {
                    self.errors.push(CobolError::new(
                        0,
                        format!(
                            "MOVE {src} TO {dst}: uno es un GRUPO y el otro un campo. \
                             Mezclarlos pide relleno con espacios, y eso necesita que \
                             exista el texto (PIC X)"
                        ),
                    ));
                    return;
                }
                let sc = self.var_scale(dst);
                self.load_operand(src, sc);
                self.store_var(dst);
            }
            // ADD a misma escala: ambos operandos en centavos → `add` es
            // suma decimal exacta (sin float, sin redondeo). El alma bancaria.
            // ★ La suma se hace en la escala MAYOR de las dos y se redondea AL
            // FINAL. Subir de escala es exacto —multiplicar por diez no pierde
            // nada—, así que no se tira ningún dígito antes de tiempo.
            //
            // Ésa es la diferencia entre redondear el RESULTADO y redondear un
            // OPERANDO, y con los modos asimétricos **no dan lo mismo**: el
            // techo de `-9.995` es `-9.99`, pero si primero se redondea el
            // `9.995` a `10.00` y luego se resta, sale `-10.00`.
            CobolStatement::Add(src, dst, redondeo) => {
                let sc = self.var_scale(dst);
                let calc = sc.max(self.escala_operando(src));
                self.load_var(dst);
                self.rescale(sc, calc);                  // exacto: hacia arriba
                self.code.push(0x50);                    // push rax
                self.load_operand(src, calc);            // exacto: su escala o más
                self.code.push(0x5A);                    // pop rdx
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                self.rescale_redondeado(calc, sc, *redondeo);
                self.store_var(dst);
            }
            // SUBTRACT src FROM dst → dst = dst - src. Misma regla de escala
            // que el ADD: se calcula donde no se pierde nada y se redondea al
            // final.
            CobolStatement::Subtract(src, dst, redondeo) => {
                let sc = self.var_scale(dst);
                let calc = sc.max(self.escala_operando(src));
                self.load_var(dst);
                self.rescale(sc, calc);                           // exacto
                self.code.push(0x50);                             // push rax (dst)
                self.load_operand(src, calc);                     // exacto
                self.code.push(0x5A);                             // pop rdx (dst)
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]); // sub rdx, rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
                self.rescale_redondeado(calc, sc, *redondeo);
                self.store_var(dst);
            }
            // ★ El multiplicando se carga EN SU PROPIA ESCALA, no en la del
            // destino: así el producto no pierde ni un dígito antes de tiempo.
            // `3.003 × 3.33` con destino de dos decimales se calcula entero y
            // se redondea UNA vez; cargando el `3.003` en dos decimales primero
            // se estaría multiplicando por `3.00`.
            CobolStatement::Multiply(src, dst, redondeo) => {
                let so = self.escala_operando(src);
                self.load_var(dst);                              // escala del destino
                self.code.push(0x50);                            // push rax
                self.load_operand(src, so);                      // su escala: exacto
                self.code.push(0x5A);                            // pop rdx
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                // El producto quedó en escala sc+so. Volver a sc es dividir
                // entre 10^so, y AHÍ es donde manda la cláusula ROUNDED.
                if so > 0 {
                    let p = 10u64.pow(so);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); });
                    bmo_lower::redondeo::dividir(&mut self.code, *redondeo);
                }
                self.store_var(dst);
            }
            // El divisor tambien va en SU escala: el dividendo se preescala
            // por 10^so, no por 10^sc, y asi el cociente sale en la escala del
            // destino sea cual sea la del divisor.
            CobolStatement::Divide(src, dst, redondeo) => {
                let so = self.escala_operando(src);
                self.load_operand(src, so);                      // divisor, exacto
                self.code.push(0x50);                            // push rax
                self.load_var(dst);                              // dividendo, escala sc
                if so > 0 {
                    let p = 10u64.pow(so);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); });
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]);    // imul rax, rcx
                }
                self.code.push(0x59);                            // pop rcx (divisor)
                // Una division casi nunca es exacta, asi que este es el sitio
                // donde ROUNDED cambia el numero mas a menudo: `100.00 / 3`.
                bmo_lower::redondeo::dividir(&mut self.code, *redondeo);
                self.store_var(dst);
            }
            CobolStatement::Compute(dst, expr, redondeo) => {
                let sc = self.var_scale(dst);
                self.emit_compute(expr, sc, *redondeo);
                self.store_var(dst);
            }
            // IF/ELSE con bifurcación REAL. Las condiciones se conjugan con
            // AND: la primera que falle salta a ELSE sin evaluar el resto
            // (cortocircuito, como manda el estándar).
            CobolStatement::If(cond, then_stmts, else_stmts) => {
                let else_label = self.fresh_label();
                let end_label = self.fresh_label();

                let cond = cond.clone();
                self.emit_jump_if_false(&cond, else_label);
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
            // ★ `EVALUATE` — la primera rama que acierta gana, y las de abajo
            // ni se prueban.
            //
            // Que se emita en cinco líneas es la prueba de que la forma del AST
            // es la buena: las dos sintaxis —con sujeto y `EVALUATE TRUE`— ya
            // llegan aquí como el MISMO árbol de condiciones, así que heredan el
            // cortocircuito y la precedencia sin una línea de más. Si el codegen
            // tuviera que distinguirlas, el parser habría hecho mal su trabajo.
            CobolStatement::Evaluate(ramas) => {
                let ramas = ramas.clone();
                let fin = self.fresh_label();
                for (cond, cuerpo) in &ramas {
                    match cond {
                        Some(c) => {
                            let siguiente = self.fresh_label();
                            self.emit_jump_if_false(c, siguiente);
                            for s in cuerpo {
                                self.emit_statement(s);
                            }
                            self.emit_jmp(fin);
                            self.bind_label(siguiente);
                        }
                        // `WHEN OTHER`: no compara, y el parser ya garantiza que
                        // es el último.
                        None => {
                            for s in cuerpo {
                                self.emit_statement(s);
                            }
                            self.emit_jmp(fin);
                        }
                    }
                }
                self.bind_label(fin);
            }

            CobolStatement::PerformFuera { desde, hasta, veces, hasta_que } => {
                let (desde, hasta, veces, hasta_que) =
                    (desde.clone(), hasta.clone(), *veces, hasta_que.clone());
                self.emit_perform_fuera(&desde, hasta.as_deref(), veces, hasta_que.as_ref());
            }

            // `EXIT` y `CONTINUE` no emiten nada, y eso es lo correcto: son el
            // hueco explícito. El destino de un `PERFORM … THRU X-SALIR` tiene
            // que existir como párrafo, no como instrucción.
            CobolStatement::Exit => {}

            CobolStatement::PerformUntil(cond, body) => {
                let top = self.fresh_label();
                let body_label = self.fresh_label();
                let done = self.fresh_label();

                self.bind_label(top);
                // Se SALE cuando la condición se cumple, así que mientras sea
                // falsa se va al cuerpo. La condición compuesta se emite
                // entera: `UNTIL FIN = 1 OR ERROR = 1` para con cualquiera de
                // las dos, y ese `OR` es la forma en la que un batch de verdad
                // dice "hasta que se acabe o hasta que algo vaya mal".
                let cond = cond.clone();
                self.emit_jump_if_false(&cond, body_label);
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
            // ★ `STOP RUN` TERMINA EL PROGRAMA, y hasta hoy no emitía nada.
            //
            // Colaba porque siempre era la última línea y detrás venía el
            // `exit` implícito del final. En cuanto hay párrafos deja de colar:
            // el `STOP RUN` del cuerpo principal tiene los párrafos DETRÁS, así
            // que no emitir nada significaba caerse dentro del primero y
            // ejecutarlo por segunda vez.
            //
            // Y ya estaba mal antes: un `STOP RUN` dentro de un `IF` —la forma
            // normal de abortar un batch cuando algo no cuadra— se ignoraba en
            // silencio y el proceso seguía.
            CobolStatement::StopRun => {
                bmo_lower::task::exit(&mut self.code);
            }
            CobolStatement::Expr(_) => {}
        }
    }

    /// `COMPUTE dst = <expresión>` con precedencia real.
    ///
    /// Antes esto llamaba a `load_scaled_imm(expr)`, que intenta parsear la
    /// expresión ENTERA como un número: `COMPUTE T = A + B` no fallaba, se
    /// evaluaba a 0 y seguía.
    ///
    /// ## En qué escala se calcula, y por qué no en la del destino
    ///
    /// Se evalúa en la escala **más alta que aparezca** —la del destino o la
    /// del operando que más decimales traiga— y se baja a la del destino
    /// **una sola vez, al final**, aplicando ahí la cláusula `ROUNDED`.
    ///
    /// Antes se evaluaba directamente en la del destino, y eso tenía un fallo
    /// que no se veía: `COMPUTE R = BASE * 0.075` con `R PIC V99` cargaba el
    /// literal en dos decimales, o sea **multiplicaba por `0.07`**. El
    /// resultado salía mal en el tercer decimal y ningún `ROUNDED` podía
    /// arreglarlo, porque para cuando llegaba, el dígito ya no estaba.
    ///
    /// ⚠ **Dónde sigue sin llegar al estándar**: COBOL manda que los
    /// intermedios lleven precisión de sobra, no la del operando más largo. Con
    /// una división en medio (`A / 3 * 3`) eso se nota. Está dicho aquí para
    /// que no se descubra en un cuadre.
    fn emit_compute(&mut self, expr: &str, scale: u32, redondeo: Redondeo) {
        // Un subindice DENTRO de la expresion no se puede leer aqui: para este
        // tokenizador un `(` abre un grupo de precedencia, asi que `TOTAL(I)`
        // seria "la tabla TOTAL" y luego "(I)" aparte. Se dice, en vez de
        // calcular otra cosa: la forma que si corre es sacarlo con un MOVE.
        for tabla in self.tablas.keys() {
            let aguja = format!("{tabla}(");
            if expr.to_ascii_uppercase().replace(' ', "").contains(&aguja) {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "COMPUTE no admite subindices todavia ({tabla}(...)): para este \
                         analizador el parentesis es de precedencia. Saca el elemento \
                         antes con `MOVE {tabla}(I) TO <aux>` y usa el auxiliar"
                    ),
                ));
                return;
            }
        }
        let tokens = Self::tokenize_expr(expr);
        // La escala de trabajo: la del destino o la del operando más largo.
        // Subir de escala es exacto, así que calcular arriba nunca empeora.
        let calc = tokens
            .iter()
            .filter(|t| !matches!(t.as_str(), "+" | "-" | "*" | "/" | "(" | ")"))
            .map(|t| self.escala_operando(t))
            .fold(scale, u32::max);
        let mut pos = 0usize;
        self.emit_expr_sum(&tokens, &mut pos, calc, redondeo);
        // Y la bajada a la escala del destino, una sola vez y con la cláusula.
        self.rescale_redondeado(calc, scale, redondeo);
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
    fn emit_expr_sum(&mut self, tokens: &[String], pos: &mut usize, scale: u32, redondeo: Redondeo) {
        self.emit_expr_product(tokens, pos, scale, redondeo);
        while *pos < tokens.len() && (tokens[*pos] == "+" || tokens[*pos] == "-") {
            let op = tokens[*pos].clone();
            *pos += 1;
            self.code.push(0x50); // push rax (izquierdo)
            self.emit_expr_product(tokens, pos, scale, redondeo);
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
    fn emit_expr_product(&mut self, tokens: &[String], pos: &mut usize, scale: u32, redondeo: Redondeo) {
        self.emit_expr_factor(tokens, pos, scale, redondeo);
        while *pos < tokens.len() && (tokens[*pos] == "*" || tokens[*pos] == "/") {
            let op = tokens[*pos].clone();
            *pos += 1;
            self.code.push(0x50); // push rax (izquierdo)
            self.emit_expr_factor(tokens, pos, scale, redondeo);
            self.code.push(0x5A); // pop rdx (izquierdo)
            if op == "*" {
                // Ambos vienen en escala s; el producto queda en 2s.
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                if scale > 0 {
                    let p = 10u64.pow(scale);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); });
                    bmo_lower::redondeo::dividir(&mut self.code, redondeo);
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
                bmo_lower::redondeo::dividir(&mut self.code, redondeo);
            }
        }
    }

    /// `factor := '(' suma ')' | operando`
    fn emit_expr_factor(&mut self, tokens: &[String], pos: &mut usize, scale: u32, redondeo: Redondeo) {
        let Some(token) = tokens.get(*pos).cloned() else {
            self.errors.push(CobolError::new(0, "expresion COMPUTE incompleta"));
            return;
        };
        *pos += 1;

        if token == "(" {
            self.emit_expr_sum(tokens, pos, scale, redondeo);
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
            self.emit_expr_factor(tokens, pos, scale, redondeo);
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
    /// Salta a `label` si la condición COMPUESTA es **falsa**.
    ///
    /// Es la primitiva que usan `IF` (salta al `ELSE`) y `PERFORM UNTIL`.
    ///
    /// ## El cortocircuito no es una optimización
    ///
    /// `A AND B` salta al primer fallo sin evaluar `B`, y `A OR B` deja de
    /// mirar en cuanto una acierta. Se emite así porque es lo que dice el
    /// estándar, y porque evaluar de más en COBOL no es gratis: un operando
    /// puede ser un elemento de tabla, y ahí la evaluación lleva **guarda de
    /// rango** — un `IF I <= 12 AND TOTAL(I) > 0` con `I = 13` tiene que parar
    /// en la primera, no reventar en la segunda.
    fn emit_jump_if_false(&mut self, cond: &Condicion, label: u32) {
        match cond {
            Condicion::Simple(c) => {
                let c = c.clone();
                self.emit_jump_if_condition_false(&c, label);
            }
            // Las dos tienen que valer: cualquiera que falle manda al mismo
            // sitio, y la segunda ni se mira si la primera ya falló.
            Condicion::Y(izq, der) => {
                self.emit_jump_if_false(izq, label);
                self.emit_jump_if_false(der, label);
            }
            // Basta una. Si la primera acierta se salta POR ENCIMA de la
            // segunda; si falla, se cae en ella y decide sola.
            Condicion::O(izq, der) => {
                let vale = self.fresh_label();
                self.emit_jump_if_true(izq, vale);
                self.emit_jump_if_false(der, label);
                self.bind_label(vale);
            }
        }
    }

    /// La otra mitad: salta a `label` si la condición es **verdadera**.
    ///
    /// Hace falta por el `OR`, y con ella el emisor queda simétrico — no hay
    /// forma de tener una rama del árbol sin su contraria.
    fn emit_jump_if_true(&mut self, cond: &Condicion, label: u32) {
        match cond {
            Condicion::Simple(c) => {
                let c = c.clone();
                self.emit_jump_if_condition_true(&c, label);
            }
            // Para que un AND sea verdad tienen que serlo las dos: se salta
            // fuera al primer fallo, y sólo se llega al salto final si ninguna
            // falló.
            Condicion::Y(izq, der) => {
                let falla = self.fresh_label();
                self.emit_jump_if_false(izq, falla);
                self.emit_jump_if_false(der, falla);
                self.emit_jmp(label);
                self.bind_label(falla);
            }
            Condicion::O(izq, der) => {
                self.emit_jump_if_true(izq, label);
                self.emit_jump_if_true(der, label);
            }
        }
    }

    /// Un nivel 88 convertido en la condición que de verdad es.
    ///
    /// La expansión la hace [`Condicion::de_valores`], compartida con el `WHEN`
    /// de un `EVALUATE` con sujeto: son la misma pregunta —"¿está este campo en
    /// este conjunto?"— y tenerla dos veces sería copiar el mismo error de
    /// extremo abierto en dos sitios.
    fn expandir_88(padre: &str, valores: &[crate::ast::Valor88]) -> Condicion {
        // Un 88 sin valores no llega hasta aquí: el parser lo rechaza. Si
        // llegara, comparar contra nada es falso, no verdadero.
        Condicion::de_valores(padre, valores).unwrap_or_else(|| {
            Condicion::Simple(CobolCondition::NotEqual("0".to_string(), "0".to_string()))
        })
    }

    /// Salta si una comparación SIMPLE es verdadera.
    ///
    /// Comparte con su contraria la carga de operandos y la expansión de los
    /// nombres de condición; lo único que cambia es el código de condición del
    /// `jcc`, y por eso viven en la misma función con un interruptor en vez de
    /// duplicadas — dos copias del reparto `push`/`pop` es donde se cuela un
    /// operando cruzado.
    fn emit_jump_if_condition_true(&mut self, cond: &CobolCondition, label: u32) {
        self.emit_comparacion(cond, label, true);
    }

    fn emit_jump_if_condition_false(&mut self, cond: &CobolCondition, label: u32) {
        self.emit_comparacion(cond, label, false);
    }

    fn emit_comparacion(&mut self, cond: &CobolCondition, label: u32, salta_si_cierta: bool) {
        // ── Un nombre de condición se expande AQUÍ ──
        //
        // `IF FIN-DE-FICHERO` es `IF FIN = 1` con otro nombre, y el otro nombre
        // es el que se lee. La expansión vive en el codegen y no en el parser
        // porque sólo aquí se sabe qué datos existen — y por tanto sólo aquí se
        // puede decir "eso no es ningún 88" en vez de tratarlo como una
        // variable que no existe y comparar contra basura.
        if let CobolCondition::Nombre(n) = cond {
            let Some((padre, valores)) = self.cond_88.get(n).cloned() else {
                self.errors.push(CobolError::new(
                    0,
                    format!(
                        "'{n}' no es un nombre de condicion: declaralo con un nivel 88 debajo \
                         de su dato, o escribe la comparacion entera"
                    ),
                ));
                return;
            };
            // ★ Un 88 con varios valores es un OR, y uno con THRU es un AND de
            // dos comparaciones. Los dos se expanden AQUÍ y bajan por el mismo
            // emisor de árboles que un `IF A > 1 OR B = 2` escrito a mano: no
            // hay un camino especial para los 88, y por eso heredan el
            // cortocircuito gratis.
            let expandida = Self::expandir_88(&padre, &valores);
            if salta_si_cierta {
                self.emit_jump_if_true(&expandida, label);
            } else {
                self.emit_jump_if_false(&expandida, label);
            }
            return;
        }

        // `cc_falsa` salta cuando la comparación NO se cumple; `cc_cierta`
        // cuando sí. Van en pareja para que no haya forma de escribir una sin
        // su contraria y que se despareen con el tiempo.
        let (a, b, cc_falsa, cc_cierta) = match cond {
            // Ya se resolvió arriba; llegar aquí sería un bug del emisor.
            CobolCondition::Nombre(_) => return,
            CobolCondition::Equal(a, b) => (a, b, 0x85, 0x84),          // jne / je
            CobolCondition::NotEqual(a, b) => (a, b, 0x84, 0x85),       // je  / jne
            CobolCondition::Greater(a, b) => (a, b, 0x8E, 0x8F),        // jle / jg
            CobolCondition::Less(a, b) => (a, b, 0x8D, 0x8C),           // jge / jl
            CobolCondition::GreaterOrEqual(a, b) => (a, b, 0x8C, 0x8D), // jl  / jge
            CobolCondition::LessOrEqual(a, b) => (a, b, 0x8F, 0x8E),    // jg  / jle
        };

        let scale = self.comparison_scale(a, b);
        let (a, b) = (a.clone(), b.clone());

        self.load_operand(&a, scale);
        self.code.push(0x50); // push rax (izquierdo)
        self.load_operand(&b, scale); // rax = derecho
        self.code.push(0x5A); // pop rdx (izquierdo)
        self.code.extend_from_slice(&[0x48, 0x39, 0xC2]); // cmp rdx, rax
        self.emit_jcc(if salta_si_cierta { cc_cierta } else { cc_falsa }, label);
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
