//! Los ANDAMIOS que comparten todas las pruebas.
//!
//! Compilar un fuente, correrlo en el emulador, armarle un disco: nueve
//! funciones que no prueban nada por si mismas y sin las cuales no se puede
//! probar nada. Estaban mezcladas entre los tests, que es donde no se ven.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;

// -- Banco de pruebas: EJECUTAR el programa, no mirarlo --------------
//
// El flujo de control de COBOL estuvo fingiendo durante toda la vida
// del frontend: `IF` emitia un `jcc` con desplazamiento 0 que nadie
// parcheaba (o sea, ejecutaba las DOS ramas) y `PERFORM` emitia
// `xor rax,rax` repetido. Compilaba, validaba el BEF y no hacia nada de
// lo que decia. Ningun test de bytes lo habria cazado -- por eso estos
// corren el programa en el emulador de `bmo-lower` y comparan lo que el
// kernel habria pintado.

/// Extrae la seccion CODE del BEF para poder ejecutarla.
pub(crate) fn code_section(bef: &[u8]) -> Vec<u8> {
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

/// Compila y ejecuta, devolviendo lo que el kernel habria mostrado.
pub(crate) fn run_cobol(src: &str) -> String {
    use bmo_lower::emu::{run, Machine};
    let bef = compile_source_to_bef(src).expect("el programa debe compilar");
    let machine = run(Machine::new(code_section(&bef)), 200_000);
    assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
    machine.console
}

/// Compila y ejecuta **sembrando lo que el terminal habria tecleado**.
///
/// Hasta hoy no existia, y por eso `ACCEPT` era la unica sentencia del lenguaje
/// que nadie habia ejecutado nunca en el banco: los demas ejemplos o no leen
/// nada o leen de un fichero. Lo que destapo escribirlo fue que **la lectura de
/// linea perdia bytes** -- ver `TASK_OP_CONSOLE_READ` en la superficie del ABI.
///
/// [!] La entrada se siembra EXACTA, sin relleno: si el programa pide mas
/// lineas de las que hay, `read_line` cede el turno en bucle y la prueba muere
/// por presupuesto de pasos. Eso es correcto -- en la maquina estaria esperando
/// a que alguien teclee-- y ademas es util: un `ACCEPT` de mas se ve.
pub(crate) fn run_cobol_con_entrada(src: &str, entrada: &str) -> String {
    use bmo_lower::emu::{run, Machine};
    let bef = compile_source_to_bef(src).expect("el programa debe compilar");
    let mut m = Machine::new(code_section(&bef));
    m.poner_entrada(entrada);
    let machine = run(m, 200_000);
    assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
    machine.console
}

/// Compila y ejecuta CON DISCO: se siembran los ficheros de entrada y se
/// devuelve `(consola, maquina)` para poder mirar lo que quedo escrito.
///
/// Sin esto, `OPEN`/`READ`/`WRITE` solo se distinguirian de un no-op
/// leyendo el ensamblador -- que es exactamente lo que este banco de
/// pruebas existe para no tener que hacer.
pub(crate) fn run_cobol_con_disco(

    src: &str,
    entrada: &[(&str, &str)],
) -> (String, bmo_lower::emu::Machine) {
    use bmo_lower::emu::{run, Machine};
    let bef = compile_source_to_bef(src).expect("el programa debe compilar");
    let mut m = Machine::new(code_section(&bef));
    for (ruta, datos) in entrada {
        m.poner_archivo(ruta, datos.as_bytes());
    }
    let m = run(m, 2_000_000);
    assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
    (m.console.clone(), m)
}

/// Igual, pero con el disco NEGANDOSE a guardar las rutas que se le digan.
///
/// Es el unico ayudante que puede probar el camino del `CLOSE` que falla, y
/// hace falta porque ese camino **no se puede provocar desde COBOL**: el
/// programa hace lo mismo en los dos casos y es el disco el que decide. Sin
/// esto, `emit_close` podia escribir `"00"` a pelo y ninguna prueba lo veia.
pub(crate) fn run_cobol_sin_poder_guardar(

    src: &str,
    entrada: &[(&str, &str)],
    no_guardables: &[&str],
) -> (String, bmo_lower::emu::Machine) {
    use bmo_lower::emu::{run, Machine};
    let bef = compile_source_to_bef(src).expect("el programa debe compilar");
    let mut m = Machine::new(code_section(&bef));
    for (ruta, datos) in entrada {
        m.poner_archivo(ruta, datos.as_bytes());
    }
    for ruta in no_guardables {
        m.fallar_al_guardar(ruta);
    }
    let m = run(m, 2_000_000);
    assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
    (m.console.clone(), m)
}

/// Igual, pero sembrando BYTES CRUDOS. Hace falta desde que un fichero
/// puede no ser texto: un registro binario tiene nibbles dentro, y pasarlo
/// por un `&str` lo destrozaria.
pub(crate) fn run_cobol_con_disco_bytes(

    src: &str,
    entrada: &[(&str, &[u8])],
) -> (String, bmo_lower::emu::Machine) {
    use bmo_lower::emu::{run, Machine};
    let bef = compile_source_to_bef(src).expect("el programa debe compilar");
    let mut m = Machine::new(code_section(&bef));
    for (ruta, datos) in entrada {
        m.poner_archivo(ruta, datos);
    }
    let m = run(m, 2_000_000);
    assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
    (m.console.clone(), m)
}

/// Un programa con DOS ficheros ya declarados: `ENTRADA` (`d/e.txt`) y
/// `SALIDA` (`d/s.txt`). Cada caso escribe su propia `FILE SECTION` porque
/// el PIC del registro es justo lo que cambia de un caso a otro.
pub(crate) fn programa_con_ficheros(decls: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\n\
         ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
         SELECT ENTRADA ASSIGN TO \"d/e.txt\".\n\
         SELECT SALIDA ASSIGN TO \"d/s.txt\".\n\
         DATA DIVISION.\n{decls}\nPROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
    )
}

pub(crate) fn program(data: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\nDATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
    )
}

/// Igual, pero **sin** anadir el `STOP RUN` del final.
///
/// Con parrafos, el `STOP RUN` que `program` pega al final ya no cae donde
/// debe: cae DENTRO del ultimo parrafo, asi que el programa termina la
/// primera vez que alguien hace `PERFORM` de el. Quien escribe parrafos
/// tiene que decir donde acaba el cuerpo principal, y por eso este ayudante
/// no lo decide por el.
pub(crate) fn programa_con_parrafos(data: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\nDATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\n{body}\n"
    )
}

/// La version con ficheros, tambien sin `STOP RUN` implicito.
pub(crate) fn ficheros_con_parrafos(decls: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\n\
         ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
         SELECT ENTRADA ASSIGN TO \"d/e.txt\".\n\
         SELECT SALIDA ASSIGN TO \"d/s.txt\".\n\
         DATA DIVISION.\n{decls}\nPROCEDURE DIVISION.\n{body}\n"
    )
}

// -- FILE STATUS: lo que un batch mira despues de CADA operacion -----
//
// No es ceremonia: un batch nocturno que revienta es peor que uno que
// escribe "no pude abrir el maestro" y para ordenadamente.

/// Un programa con UN fichero y su `FILE STATUS` declarado. El ayudante
/// general no sirve: sus `SELECT` no lo llevan, y ese es justo el trozo que
/// se esta probando.
pub(crate) fn programa_con_estado(decls: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. T.
         ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
         SELECT ENTRADA ASSIGN TO \"d/e.txt\" FILE STATUS IS ST.
         DATA DIVISION.
{decls}
PROCEDURE DIVISION.
{body}
STOP RUN.
"
    )
}

/// Un programa que ESCRIBE `d/s.txt` y mira su estado despues del `CLOSE`.
pub(crate) fn programa_que_guarda(body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
         ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
         SELECT SALIDA ASSIGN TO \"d/s.txt\" FILE STATUS IS ST.\n\
         DATA DIVISION.\nFILE SECTION.\nFD SALIDA.\n01 R PIC 9(4).\n\
         WORKING-STORAGE SECTION.\n01 ST PIC XX VALUE \"??\".\n\
         PROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
    )
}

