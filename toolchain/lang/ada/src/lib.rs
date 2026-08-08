//! **BMO Ada** -- Ada compilado a nativo, sin runtime y sin biblioteca.
//!
//! ## Por que Ada, y por que salio barato
//!
//! El objetivo de BMO es banca, y para banca hacen falta dos cosas: decimal
//! exacto y un compilador del que uno se pueda fiar. Ada trae las dos de
//! fabrica -- y trae una tercera que casi nadie menciona:
//!
//! **El Annex F de Ada (Information Systems) copio las reglas de COBOL.** Los
//! tipos decimales (`delta 0.01 digits 12`) y la edicion con `PICTURE` de
//! `Ada.Text_IO.Editing` estan definidos sobre ANSI X3.23-1985, que es COBOL.
//! Por eso este frontend nace con el decimal ya resuelto: es la misma
//! aritmetica de escala entera que ya estaba escrita y probada.
//!
//! ## Que es y que NO es
//!
//! Es un frontend **completo y propio**: lexer, analisis y emisor de bytes,
//! sin pasar por ningun cerebro compartido. No depende de `lang/cobol` -- la
//! regla del proyecto es que cada lenguaje mantiene su esencia de principio a
//! fin, y lo unico compartido son contratos (el contenedor BEF, los 3
//! syscalls) y librerias **opcionales** (`bmo-lower`).
//!
//! **No es GNAT.** Lo que compila hoy esta en la matriz de conformidad de
//! abajo, y todo lo demas se rechaza **con su motivo**: `package`, genericos,
//! tareas, `for`, `elsif`, y cualquier `with` que no sea `Ada.Text_IO`.
//! Prometer Ada entero seria mentir en la primera linea.
//!
//! ## El limite que importa: un fichero, una unidad
//!
//! Ada de verdad son especificacion y cuerpo con **orden de elaboracion**
//! (RM 10.2.1). Eso es semantica del lenguaje y no se puede fingir, asi que
//! aqui se acota a un `procedure` suelto --forma que el estandar permite-- y se
//! rechaza lo que pida el modelo de unidades. Es la misma decision que en los
//! otros frontends: alcance acotado y dicho en voz alta.

pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;

pub use ast::AdaError;

/// Analiza el fuente.
pub fn analizar(fuente: &str) -> Result<ast::Programa, AdaError> {
    parser::Parser::nuevo(fuente).programa()
}

/// Compila Ada a un ejecutable BEX nativo.
pub fn compilar(fuente: &str) -> Result<Vec<u8>, AdaError> {
    let p = analizar(fuente)?;
    codegen::compilar(&p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La seccion de codigo del BEF, para darsela al emulador.
    ///
    /// Se lee la tabla de secciones a mano, igual que el cargador del kernel:
    /// asi el test cruza el MISMO formato que va a cruzar la maquina, y un BEF
    /// mal construido se cae aqui y no en el arranque.
    fn seccion_codigo(bef: &[u8]) -> Vec<u8> {
        use bmo_abi::bef::sections::{SectionEntry, SectionKind};
        let sec_off = u64::from_le_bytes(bef[32..40].try_into().unwrap()) as usize;
        let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
        for i in 0..hdr.section_count as usize {
            let entry = sec_off + i * SectionEntry::SIZE;
            if bef[entry] == SectionKind::Code as u8 {
                let off = u64::from_le_bytes(bef[entry + 8..entry + 16].try_into().unwrap()) as usize;
                let size = u64::from_le_bytes(bef[entry + 16..entry + 24].try_into().unwrap()) as usize;
                return bef[off..off + size].to_vec();
            }
        }
        panic!("el BEF no tiene seccion CODE");
    }

    /// Compila y EJECUTA. Devuelve lo que el programa escribio.
    ///
    /// Ejecutar y no mirar los bytes: un `if` que no bifurca se ve igual que
    /// uno que si en un volcado, y `while` que no repite tambien.
    fn correr(fuente: &str) -> String {
        use bmo_lower::emu::{run, Machine};
        let bef = compilar(fuente).expect("el programa debe compilar");
        let m = Machine::new(seccion_codigo(&bef));
        let m = run(m, 2_000_000);
        assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
        m.console
    }

    /// Envuelve un cuerpo en el programa minimo.
    fn programa(decls: &str, cuerpo: &str) -> String {
        format!(
            "with Ada.Text_IO; use Ada.Text_IO;\n\
             procedure Prueba is\n{decls}\nbegin\n{cuerpo}\nend Prueba;\n"
        )
    }

    /// * MATRIZ DE CONFORMIDAD DE ADA.
    ///
    /// Cada cosa que este compilador dice compilar tiene su fila, y la fila se
    /// EJECUTA. Al anadir una caracteristica al emisor hay que anadirle la
    /// suya -- es la misma regla que en C y en COBOL.
    #[test]
    fn matriz_de_ada_corre_correctamente() {
        let casos: &[(&str, &str, &str, &str)] = &[
            ("Put_Line texto", "", "Put_Line(\"hola\");", "hola\n"),
            ("varios Put_Line", "", "Put_Line(\"a\");\nPut_Line(\"b\");", "a\nb\n"),
            // Enteros
            ("Integer inicial", "N : Integer := 42;", "Put_Line(N);", "42\n"),
            ("Integer sin inicial es cero", "N : Integer;", "Put_Line(N);", "0\n"),
            ("asignar", "N : Integer;", "N := 7;\nPut_Line(N);", "7\n"),
            ("sumar", "N : Integer := 2;", "N := N + 3;\nPut_Line(N);", "5\n"),
            ("restar", "N : Integer := 9;", "N := N - 4;\nPut_Line(N);", "5\n"),
            ("multiplicar", "N : Integer := 3;", "N := N * 4;\nPut_Line(N);", "12\n"),
            ("dividir", "N : Integer := 12;", "N := N / 4;\nPut_Line(N);", "3\n"),
            ("negativo", "N : Integer := 5;", "N := N - 12;\nPut_Line(N);", "-7\n"),
            ("precedencia", "N : Integer;", "N := 2 + 3 * 4;\nPut_Line(N);", "14\n"),
            ("parentesis", "N : Integer;", "N := (2 + 3) * 4;\nPut_Line(N);", "20\n"),
            ("guiones bajos", "N : Integer := 1_000;", "Put_Line(N);", "1000\n"),
            // * El decimal de Annex F: la razon de que Ada este aqui.
            (
                "decimal exacto",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 19.99;",
                "S := S + 0.01;\nPut_Line(S);",
                "20.00\n",
            ),
            (
                "tres por 19.99",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 19.99;",
                "S := S * 3;\nPut_Line(S);",
                "59.97\n",
            ),
            (
                "dividir decimal",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 10.00;",
                "S := S / 4;\nPut_Line(S);",
                "2.50\n",
            ),
            (
                "decimal negativo",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := -120.00;",
                "Put_Line(S);",
                "-120.00\n",
            ),
            (
                "centimos no se pierden",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 0.05;",
                "S := S + 0.05;\nPut_Line(S);",
                "0.10\n",
            ),
            // Mezclar escalas: un entero sumado a un decimal se reescala.
            (
                "entero mas decimal",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 1.50;\nN : Integer := 2;",
                "S := S + N;\nPut_Line(S);",
                "3.50\n",
            ),
            // Control
            ("if verdadero", "N : Integer := 5;", "if N = 5 then\nPut_Line(\"ok\");\nend if;", "ok\n"),
            ("if falso", "N : Integer := 1;", "if N = 5 then\nPut_Line(\"no\");\nend if;", ""),
            ("if else", "N : Integer := 1;", "if N = 5 then\nPut_Line(\"no\");\nelse\nPut_Line(\"ok\");\nend if;", "ok\n"),
            ("distinto", "N : Integer := 1;", "if N /= 5 then\nPut_Line(\"ok\");\nend if;", "ok\n"),
            ("menor", "N : Integer := 1;", "if N < 5 then\nPut_Line(\"ok\");\nend if;", "ok\n"),
            ("mayor o igual", "N : Integer := 5;", "if N >= 5 then\nPut_Line(\"ok\");\nend if;", "ok\n"),
            (
                "comparar decimales",
                "type Saldo is delta 0.01 digits 12;\nS : Saldo := 0.10;",
                "if S > 0.09 then\nPut_Line(\"ok\");\nend if;",
                "ok\n",
            ),
            ("while", "N : Integer := 0;", "while N < 3 loop\nN := N + 1;\nend loop;\nPut_Line(N);", "3\n"),
            ("while que no entra", "N : Integer := 9;", "while N < 3 loop\nN := N + 1;\nend loop;\nPut_Line(N);", "9\n"),
            (
                "while anidado",
                "I : Integer := 0;\nJ : Integer := 0;\nT : Integer := 0;",
                "while I < 3 loop\nJ := 0;\nwhile J < 2 loop\nT := T + 1;\nJ := J + 1;\nend loop;\nI := I + 1;\nend loop;\nPut_Line(T);",
                "6\n",
            ),
            // * El bucle que suma dinero: el batch, en Ada.
            (
                "totalizar en decimal",
                "type Saldo is delta 0.01 digits 12;\nT : Saldo := 0.00;\nI : Integer := 0;",
                "while I < 3 loop\nT := T + 19.99;\nI := I + 1;\nend loop;\nPut_Line(T);",
                "59.97\n",
            ),
            ("comentarios", "N : Integer := 1;", "-- esto no cuenta\nN := N + 1; -- ni esto\nPut_Line(N);", "2\n"),
            ("mayusculas dan igual", "Saldo : Integer := 5;", "SALDO := saldo + 1;\nPut_Line(Saldo);", "6\n"),
        ];

        let mut rotos = Vec::new();
        for (nombre, decls, cuerpo, esperado) in casos {
            let src = programa(decls, cuerpo);
            let salio = std::panic::catch_unwind(|| correr(&src))
                .unwrap_or_else(|_| "<no ejecuta>".into());
            if salio != *esperado {
                rotos.push(format!("  {nombre:<26} => {salio:?}  (esperado {esperado:?})"));
            }
        }
        let total = casos.len();
        assert!(
            rotos.is_empty(),
            "\n{}/{} FUNCIONAN. ROTOS:\n{}",
            total - rotos.len(),
            total,
            rotos.join("\n")
        );
    }

    // -- Lo que se RECHAZA, y diciendo que hacer -------------------------

    fn error_de(fuente: &str) -> String {
        format!("{}", compilar(fuente).unwrap_err())
    }

    /// `=` compara y `:=` asigna. Confundirlos **no compila**, que es la razon
    /// por la que Ada se eligio para lo critico: en C, `if (x = 5)` asigna.
    #[test]
    fn confundir_asignar_con_comparar_no_compila() {
        let e = error_de(&programa("N : Integer;", "N = 5;"));
        assert!(e.contains("compara, no asigna") && e.contains(":="), "{e}");
    }

    /// Un `package` son dos unidades con orden de elaboracion. Se dice, en vez
    /// de compilar media cosa.
    #[test]
    fn un_package_se_rechaza_explicando_por_que() {
        let e = error_de("package Banco is\nend Banco;\n");
        assert!(e.contains("DOS unidades") && e.contains("procedure"), "{e}");
    }

    /// Las tareas piden planificador. El perfil declarado es ZFP secuencial.
    #[test]
    fn las_tareas_se_rechazan_nombrando_el_perfil() {
        let e = error_de("task Motor;\n");
        assert!(e.contains("planificador") && e.contains("ZFP"), "{e}");
    }

    /// No hay biblioteca estandar detras. Aceptar cualquier `with` seria
    /// prometerla.
    #[test]
    fn un_with_que_no_existe_se_rechaza() {
        let e = error_de("with Ada.Calendar;\nprocedure P is\nbegin\nnull;\nend P;\n");
        assert!(e.contains("Ada.Text_IO") || e.contains("ADA.TEXT_IO"), "{e}");
    }

    /// Un tipo sin declarar no se toma por entero: se dice como declararlo.
    #[test]
    fn un_tipo_que_no_existe_se_rechaza_ensenando_la_forma() {
        let e = error_de(&programa("S : Saldo := 1.00;", "Put_Line(S);"));
        assert!(e.contains("delta") && e.contains("digits"), "{e}");
    }

    /// Mas de 18 cifras no caben en el entero de 64 bits donde vive el
    /// decimal exacto. Se dice al declarar, no al desbordar.
    #[test]
    fn mas_de_18_digitos_se_rechaza() {
        let e = error_de(&programa("type Grande is delta 0.01 digits 30;\nS : Grande;", "Put_Line(S);"));
        assert!(e.contains("18") && e.contains("64 bits"), "{e}");
    }

    /// El `end` con nombre tiene que cuadrar. Es lo que caza un `end` puesto
    /// en el sitio equivocado.
    #[test]
    fn el_end_con_otro_nombre_se_rechaza() {
        let e = error_de("procedure Uno is\nbegin\nnull;\nend Dos;\n");
        assert!(e.contains("no cuadra"), "{e}");
    }

    /// Una variable que nadie declaro. Antes de existir esta comprobacion,
    /// `cargar` habria emitido nada y el valor seria lo que hubiera en `rax`.
    #[test]
    fn una_variable_sin_declarar_se_rechaza() {
        let e = error_de(&programa("N : Integer;", "M := 1;"));
        assert!(e.contains("no esta declarada"), "{e}");
    }

    /// Declarar dos veces el mismo nombre es un error del programa, no un
    /// "gana el ultimo".
    #[test]
    fn declarar_dos_veces_se_rechaza() {
        let e = error_de(&programa("N : Integer;\nN : Integer;", "N := 1;"));
        assert!(e.contains("dos veces"), "{e}");
    }

    /// El ejemplo entero, ejecutado: es la prueba de que la cadena completa
    /// --fuente Ada, analisis, emisor, BEF, CPU-- produce lo que dice.
    #[test]
    fn el_cierre_de_ada_produce_su_informe() {
        let salida = correr(include_str!("../examples/1-basico/cierre.adb"));
        let esperado = [
            "CIERRE EN ADA - BANCO BMO",
            "total de tres cuotas:",
            "59.97",
            "tras la devolucion:",
            "39.98",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
    }
}
