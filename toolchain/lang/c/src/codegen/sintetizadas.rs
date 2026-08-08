//! **Las funciones SINTETIZADAS** -- el catalogo, y quien emite cada cuerpo.
//!
//! Esto vivia dentro de `codegen/mod.rs`, que llego a **2962 lineas**. Salio
//! aqui por la misma razon por la que salieron `agregados` y `entrada`: no
//! porque el fichero fuera largo, sino porque **este trozo tiene una frontera
//! de verdad**. Dentro no se sabe que es una expresion de C, ni un tipo, ni un
//! `printf`: solo hay nombres y los bytes que los implementan.
//!
//! ## El reparto con `mod.rs`, que es lo que hace util el corte
//!
//! ```text
//!   AQUI            el CATALOGO: nombre -> quien emite sus bytes
//!                   y los cuerpos, que no tocan el estado del Codegen
//!
//!   mod.rs          la PASADA: recorrer las relocs pendientes, inyectar lo
//!                   que haga falta y registrar su offset
//! ```
//!
//! La frontera se puede comprobar de un vistazo: aqui no aparece `self` ni una
//! sola vez. Todo lo de este fichero son funciones libres que reciben un
//! `&mut Vec<u8>`, que es exactamente la forma de los emisores de `bmo_lower`.
//! Por eso una entrada de la tabla puede ser un emisor de L1 sin envoltorio.
//!
//! ## Y lo que NO puede entrar todavia
//!
//! `malloc` y `free` siguen emitiendose en linea en `emitir_biblioteca`, y no
//! por descuido: usan `fresh_label()`, que es estado del `Codegen`, y un
//! [`Sintetizador`] solo recibe `&mut Vec<u8>`. Meterlos pide que la tabla
//! acepte emisores con etiquetas -- un cambio de la tabla, no de las funciones.

use super::CallReloc;
use std::collections::HashMap;

/// Quien emite el cuerpo de una funcion SINTETIZADA: apendiza x86-64 crudo,
/// igual que los emisores de `bmo_lower`. Misma forma a proposito -- asi una
/// entrada de la tabla puede ser un emisor de L1 sin envoltorio.
type Sintetizador = fn(&mut Vec<u8>);

/// * LA TABLA DE FUNCIONES SINTETIZABLES -- nombre -> quien emite sus bytes.
///
/// # Que problema resuelve
///
/// Hasta ahora este codegen tenia DOS formas de dar una funcion y ninguna
/// intermedia:
///
/// ```text
///   EN LINEA      el bucle entero, otra vez, en CADA sitio de llamada
///                 -> perfecto para las seis funciones de un programa pequeno
///                 -> y cada llamada paga su copia
///
///   NADA          `patch_call_relocs` falla: "no existe la funcion 'X'"
/// ```
///
/// Un programa que llama a `memcpy` doscientas veces --o sea DOOM, donde por
/// ahi pasa el blit de cada fotograma-- pagaba doscientas copias del mismo
/// bucle. La regla que decide, y que ya estaba escrita en `bmo-rt/src/lib.rs`:
///
/// > **En linea lo que no tiene semantica de lenguaje y se usa poco. Enlazado
/// > lo que tiene estado, tamano, o se llama desde muchos sitios.**
///
/// # Como funciona
///
/// El mecanismo NO es nuevo, y eso es lo mejor que tiene: `__bmo_syscall_stub`
/// llevaba semanas corriendo en el Ryzen exactamente asi --un cuerpo emitido
/// una vez, y `call rel32` parcheado por `patch_call_relocs`--, solo que
/// cableado a mano para un unico nombre. Esto es esa misma via convertida en
/// tabla, y por eso el stub es su primera entrada: si la tabla no supiera
/// reproducir el caso que ya funciona, no serviria.
///
/// # La ABI que un cuerpo de aqui tiene que respetar
///
/// La de BMO C, que **no es SysV**: los argumentos van por la PILA, empujados
/// de derecha a izquierda (ver el `.rev()` del sitio de llamada), asi que tras
/// `push rbp; mov rbp, rsp` quedan en `[rbp+16]`, `[rbp+24]`, `[rbp+32]`..., y
/// el retorno en `rax`. Confundir esto con SysV daria una funcion que compila
/// y lee los argumentos de registros que nadie relleno.
const SINTETIZABLES: &[(&str, Sintetizador)] = &[
    // La puerta de syscalls: `syscall; ret`. Tres bytes, y el caso que
    // demuestra que la tabla subsume lo que ya corria cableado.
    ("__bmo_syscall_stub", sintetiza_syscall_stub),
    // `memcpy(dst, src, n)` -> dst. Ver `sintetiza_memcpy`.
    ("memcpy", sintetiza_memcpy),
    // * LAS CONVERSIONES DE `printf`. Estas son las que de verdad se repiten:
    // ningun ejemplo del repo llama a `memcpy` y **todos** llaman a `printf`.
    //
    // No reciben sus argumentos por la pila: el valor llega **en `rax`**, que
    // es la convencion que ya tenian cuando se emitian en linea (la pone
    // `emit_cargar_de_pila` en el sitio de llamada). Por eso su cuerpo es el
    // emisor y un `ret`, sin prologo ni marco -- y por eso no hay aqui ninguna
    // traduccion de ABI que poder equivocar.
    ("__bmo_fmt_i64", sintetiza_fmt_i64),
    ("__bmo_fmt_u64_dec", sintetiza_fmt_u64_dec),
    ("__bmo_fmt_u64_hex", sintetiza_fmt_u64_hex),
    ("__bmo_fmt_char", sintetiza_fmt_char),
    ("__bmo_fmt_cstr", sintetiza_fmt_cstr),
    // * LAS CADENAS -- la pieza 5, que cierra el enlazador.
    //
    // Se convirtieron ESTAS y no todas, y el criterio fue medido: enlazar
    // cuesta ~10 bytes por llamada (empujar + `call` + devolver la pila) y en
    // linea cuesta ~3 mas el cuerpo. O sea que enlazar gana cuando el cuerpo
    // pasa de unos 7 bytes. Los cuerpos, medidos:
    //
    //   comparar_n (strncmp/memcmp)  46      buscar   (strchr)  39
    //   comparar   (strcmp)          25      largo    (strlen)  15
    //   rellenar   (memset)          15      copiar   (memcpy)  20
    //   absoluto   (abs)             13  <-- se queda EN LINEA
    //
    // `abs` no entra: trece bytes apenas pasan del coste de llamarlo, y con el
    // prologo el cambio saldria a perder en cualquier programa que no lo llame
    // muchas veces. La regla que lo decide no es "todo a la tabla".
    ("strlen", sintetiza_strlen),
    ("strcpy", sintetiza_strcpy),
    ("memset", sintetiza_memset),
    ("strcmp", sintetiza_strcmp),
    ("strchr", sintetiza_strchr),
    ("strncmp", sintetiza_strncmp),
    ("memcmp", sintetiza_memcmp),
];

// -- Los ladrillos de un cuerpo sintetizado ----------------------------
//
// Existen para no escribir `[rbp+16]` a mano siete veces, que es exactamente
// como se cuela un `[rbp+24]` donde iba `[rbp+16]`: el binario compila, el
// emulador lo ejecuta, y la funcion lee el argumento de al lado.

/// El ModRM de `mov <r64>, [rbp+disp8]` para los registros que usan los
/// emisores de L1. El byte es `0b01_reg_101`: modo disp8, base `rbp`.
const A_RAX: u8 = 0x45;
const A_RCX: u8 = 0x4D;
const A_RDX: u8 = 0x55;
const A_RSI: u8 = 0x75;
const A_RDI: u8 = 0x7D;

/// `push rbp; mov rbp, rsp` -- lo que hace que `[rbp+16]` sea el argumento 0.
fn prologo(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);
}

/// `pop rbp; ret`.
fn epilogo(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x5D, 0xC3]);
}

/// `mov <reg>, [rbp + 16 + 8*n]` -- el argumento n-esimo a un registro.
///
/// El 16 es la direccion de retorno mas el `rbp` empujado; el resto sale del
/// orden de empuje del sitio de llamada, que es de DERECHA A IZQUIERDA (el
/// `.rev()`), asi que el argumento 0 es el que queda mas cerca.
fn carga_arg(code: &mut Vec<u8>, reg: u8, n: u8) {
    code.extend_from_slice(&[0x48, 0x8B, reg, 16 + 8 * n]);
}

/// `syscall; ret` -- el cuerpo que estaba cableado en `emit_program`.
fn sintetiza_syscall_stub(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x0F, 0x05, 0xC3]);
}

/// Las cinco conversiones de `printf`, cada una **una sola vez**.
///
/// # Por que basta el emisor y un `ret`
///
/// Los tres hechos que lo permiten, comprobados antes de envolverlos y no
/// supuestos --si alguno dejara de ser cierto, esto se rompe en metal y no en
/// compilacion--:
///
/// 1. **El valor llega en `rax`.** Es lo que ya hacia el sitio de llamada con
///    `emit_cargar_de_pila`; convertir a `call` no cambia de donde sale.
/// 2. **Estan equilibrados en `rsp`.** `write_i64` hace `sub rsp,32` ... `add
///    rsp,32`, y su `lea r8,[rsp+32]` no sale de su propio marco. Por eso el
///    `call` --que empuja ocho bytes de direccion de retorno-- no descoloca los
///    accesos relativos a `rsp` del `printf` que sigue: la carga del argumento
///    ocurre ANTES del `call`, y el `ret` devuelve la pila.
/// 3. **Sus saltos son relativos internos**, asi que reubicar el bloque no lo
///    rompe.
///
/// # Que NO se comparte, y no es un descuido
///
/// Los trozos literales del formato siguen EN LINEA. `console::write_const`
/// mete el texto **dentro de las instrucciones** como inmediatos --por eso no
/// necesita `.rodata` ni fixup--, asi que su cuerpo es distinto en cada llamada
/// y no hay nada que compartir. Lo que se comparte son las conversiones, que
/// es donde esta el formateador.
fn sintetiza_fmt_i64(code: &mut Vec<u8>) {
    bmo_lower::fmt::write_i64(code);
    code.push(0xC3); // ret
}

/// `%u` -- decimal sin signo. Hermana de [`sintetiza_fmt_u64_hex`]: mismo
/// emisor con otra base. Son dos funciones y no una con parametro porque la
/// tabla guarda `fn`, no cierres.
fn sintetiza_fmt_u64_dec(code: &mut Vec<u8>) {
    bmo_lower::fmt::write_u64_radix(code, 10);
    code.push(0xC3);
}

/// `%x` -- hexadecimal.
fn sintetiza_fmt_u64_hex(code: &mut Vec<u8>) {
    bmo_lower::fmt::write_u64_radix(code, 16);
    code.push(0xC3);
}

/// `%c` -- un caracter.
fn sintetiza_fmt_char(code: &mut Vec<u8>) {
    bmo_lower::fmt::write_char(code);
    code.push(0xC3);
}

/// `%s` -- una cadena terminada en cero, cuyo puntero llega en `rax`.
fn sintetiza_fmt_cstr(code: &mut Vec<u8>) {
    bmo_lower::fmt::write_cstr(code);
    code.push(0xC3);
}

// -- LA PIEZA 5: las cadenas -------------------------------------------
//
// Las convenciones de registro de cada emisor estan LEIDAS DE SU FUENTE
// (`bmo_lower::memoria`), no copiadas del sitio de llamada que se sustituye:
// si el sitio de llamada tuviera un error, copiarlo lo habria conservado.
//
//   largo      RDI=s                    -> RAX
//   rellenar   RDI=dst RAX=val RCX=n
//   comparar   RDI=a   RSI=b            -> RAX
//   comparar_n RDI=a   RSI=b   RDX=n    -> RAX
//   buscar     RDI=s   RSI=c (en SIL)   -> RAX

/// `strlen(s)` -> largo.
fn sintetiza_strlen(code: &mut Vec<u8>) {
    prologo(code);
    carga_arg(code, A_RDI, 0);
    bmo_lower::memoria::largo(code);
    epilogo(code);
}

/// `memset(dst, val, n)` -> dst.
fn sintetiza_memset(code: &mut Vec<u8>) {
    prologo(code);
    carga_arg(code, A_RDI, 0);
    carga_arg(code, A_RAX, 1);
    carga_arg(code, A_RCX, 2);
    bmo_lower::memoria::rellenar(code);
    carga_arg(code, A_RAX, 0); // devuelve dst
    epilogo(code);
}

/// `strcmp(a, b)` -> diferencia con signo.
fn sintetiza_strcmp(code: &mut Vec<u8>) {
    prologo(code);
    carga_arg(code, A_RDI, 0);
    carga_arg(code, A_RSI, 1);
    bmo_lower::memoria::comparar(code);
    epilogo(code);
}

/// `strchr(s, c)` -> puntero al byte, o cero.
fn sintetiza_strchr(code: &mut Vec<u8>) {
    prologo(code);
    carga_arg(code, A_RDI, 0);
    carga_arg(code, A_RSI, 1);
    bmo_lower::memoria::buscar(code);
    epilogo(code);
}

/// `strncmp(a, b, n)` -- para en el terminador.
fn sintetiza_strncmp(code: &mut Vec<u8>) {
    sintetiza_comparar_n(code, true);
}

/// `memcmp(a, b, n)` -- NO para en el terminador: compara los `n` bytes.
///
/// Es el mismo emisor que `strncmp` con un booleano distinto, y esa diferencia
/// de un bit es toda la diferencia entre las dos funciones de C.
fn sintetiza_memcmp(code: &mut Vec<u8>) {
    sintetiza_comparar_n(code, false);
}

fn sintetiza_comparar_n(code: &mut Vec<u8>, parar_en_cero: bool) {
    prologo(code);
    carga_arg(code, A_RDI, 0);
    carga_arg(code, A_RSI, 1);
    carga_arg(code, A_RDX, 2);
    bmo_lower::memoria::comparar_n(code, parar_en_cero);
    epilogo(code);
}

/// `strcpy(dst, src)` -> dst.
///
/// El unico que COMPONE dos emisores, y el orden no es libre: `largo` ensucia
/// `cl`, asi que la medida tiene que salir ANTES de cargar `rcx` con ella. Al
/// reves, `rcx` llegaria machacado al bucle de copia y se copiarian los bytes
/// que dijera la basura.
///
/// El `inc rax` es el terminador: `largo` no lo cuenta --que es lo que dice
/// `strlen`-- pero `strcpy` si lo copia, y sin el la cadena destino se quedaria
/// sin cerrar y el siguiente `strlen` leeria memoria ajena.
fn sintetiza_strcpy(code: &mut Vec<u8>) {
    prologo(code);
    carga_arg(code, A_RDI, 1); // src
    bmo_lower::memoria::largo(code); // rax = largo(src)
    code.extend_from_slice(&[0x48, 0xFF, 0xC0]); // inc rax  (el terminador)
    code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax
    carga_arg(code, A_RDI, 0); // dst
    carga_arg(code, A_RSI, 1); // src
    bmo_lower::memoria::copiar(code);
    carga_arg(code, A_RAX, 0); // devuelve dst
    epilogo(code);
}

/// `memcpy(dst, src, n)` -> `dst`, UNA vez, llamada con `call`.
///
/// El cuerpo es el mismo `bmo_lower::memoria::copiar` que se emitia en linea
/// --no hay una segunda implementacion de "mueve bytes", que seria la clase de
/// duplicado que `bmo-lower` existe para evitar--: lo unico que se anade es el
/// prologo que traduce la ABI de pila de BMO C a los registros que ese emisor
/// espera (`rdi`=dst, `rsi`=src, `rcx`=n), y el `mov rax, [rbp+16]` del final,
/// porque **`memcpy` devuelve el destino** y `copiar` se lleva `rdi` por
/// delante al avanzar.
///
/// `copiar` es apto para esto y se comprobo antes de envolverlo: toca
/// `rsi`/`rdi`/`rcx`/`al`, no toca `rbp`, no desequilibra la pila y sus saltos
/// son relativos internos -- o sea que reubicarlo no lo rompe.
fn sintetiza_memcpy(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x55]);                   // push rbp
    code.extend_from_slice(&[0x48, 0x89, 0xE5]);       // mov rbp, rsp
    code.extend_from_slice(&[0x48, 0x8B, 0x7D, 0x10]); // mov rdi, [rbp+16]  dst
    code.extend_from_slice(&[0x48, 0x8B, 0x75, 0x18]); // mov rsi, [rbp+24]  src
    code.extend_from_slice(&[0x48, 0x8B, 0x4D, 0x20]); // mov rcx, [rbp+32]  n
    bmo_lower::memoria::copiar(code);
    code.extend_from_slice(&[0x48, 0x8B, 0x45, 0x10]); // mov rax, [rbp+16]  -> dst
    code.extend_from_slice(&[0x5D]);                   // pop rbp
    code.extend_from_slice(&[0xC3]);                   // ret
}

// -- LA CONSULTA, que es todo lo que `mod.rs` necesita de aqui ---------

/// Quien emite el cuerpo de `nombre`, si es de los que este modulo sabe hacer.
///
/// Es la UNICA puerta: `mod.rs` no ve la tabla ni los emisores. Anadir una
/// funcion sintetizable es tocar este fichero y nada mas.
pub(super) fn buscar(nombre: &str) -> Option<Sintetizador> {
    SINTETIZABLES
        .iter()
        .find(|(n, _)| *n == nombre)
        .map(|(_, e)| *e)
}

/// Inyecta el cuerpo de cada funcion del catalogo a la que alguien llama y que
/// no esta definida en la unidad. **Una sola vez cada una**, que es el punto
/// entero: lo que antes se copiaba en cada sitio de llamada se emite aqui y se
/// alcanza con `call rel32`.
///
/// # Una pasada basta, y conviene decir por que
///
/// Una funcion sintetizada no puede llamar a otra: su emisor recibe solo
/// `&mut Vec<u8>`, asi que no tiene forma de empujar una `CallReloc`. Por eso
/// aqui no hay bucle hasta punto fijo -- seria una rama que ninguna entrada de
/// la tabla puede ejercer, o sea codigo sin probar disfrazado de prevision.
/// **Si algun dia un emisor necesita llamar a otro, esto tiene que volverse un
/// bucle, y este parrafo es el aviso.**
pub(super) fn inyectar(
    code: &mut Vec<u8>,
    relocs: &[CallReloc],
    offsets: &mut HashMap<String, usize>,
) {
    let mut pendientes: Vec<&str> = Vec::new();
    for reloc in relocs {
        if offsets.contains_key(&reloc.target) || pendientes.contains(&reloc.target.as_str()) {
            continue;
        }
        if let Some(&(nombre, _)) = SINTETIZABLES.iter().find(|(n, _)| *n == reloc.target.as_str()) {
            pendientes.push(nombre);
        }
    }
    for nombre in pendientes {
        let emisor = buscar(nombre).expect("el nombre sale de la propia tabla");
        let off = code.len();
        emisor(code);
        offsets.insert(nombre.to_string(), off);
    }
}
