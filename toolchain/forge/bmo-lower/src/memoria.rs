//! **L1 — bloques de bytes**: copiar, rellenar, comparar, medir.
//!
//! # Por qué esto vive en L1 y no en el frontend de C
//!
//! Lo que C escribe `memcpy(a,b,n)`, COBOL lo escribe `MOVE` de un grupo a
//! otro y Ada lo escribe con una asignación de array. **Es la misma operación
//! y la misma emisión**: mover `n` bytes de un sitio a otro. Lo único distinto
//! es cómo se deletrea.
//!
//! Ésa es exactamente la frontera que este crate defiende: L1 tiene lo
//! expresable sin semántica de lenguaje, y "mueve estos bytes" no tiene
//! ninguna. Si esto se escribiera en `lang/c`, el día que COBOL necesite mover
//! un grupo grande habría dos copias del mismo bucle — y una de las dos
//! tendría el bug.
//!
//! # Por qué se emite EN LÍNEA y no se llama a una librería
//!
//! Porque aquí no hay librería que enlazar, y eso no es una carencia: es el
//! modelo. Un `.bex` es una imagen entera y BEF no tiene relocaciones que
//! resolver contra un `.so`. Emitir el bucle cuesta ~30 bytes y ahorra un
//! enlazador, un formato de librería y un cargador dinámico — las tres cosas
//! que en otros sistemas hay que pelear antes de imprimir "hola".
//!
//! # Lo que NO hay, y se dice
//!
//! No hay versión vectorizada. Un `memcpy` de glibc elige entre una docena de
//! rutas según el tamaño y el modelo de CPU; éste copia de byte en byte. Para
//! mover el framebuffer de DOOM (64 000 bytes por fotograma) eso se va a
//! notar, y cuando se note se cambia por copias de 8 bytes con cola —
//! **medido primero**. Optimizar un bucle que nadie ha cronometrado es
//! adivinar.

use crate::x86::*;

// ── Saltos que NO se cuentan a mano ───────────────────────────────────
//
// ★ La primera versión de este módulo llevaba los desplazamientos escritos a
// mano (`jz +10`, `jnz +11`) y **tres de los cuatro estaban mal por uno**. El
// emulador lo cazó de la peor forma posible y por eso de la mejor: saltó a
// mitad de una instrucción y siguió decodificando, y lo que salió fue
// `opcode 0xEA no emitido por BMO` — un opcode que nadie había emitido nunca.
//
// Contar bytes de instrucción a mano es exactamente el trabajo que una máquina
// hace sin equivocarse. Se emite un hueco, se apunta dónde está, y se rellena
// cuando ya se sabe la distancia.

/// Emite un salto corto con destino pendiente. Devuelve dónde está el hueco.
fn salto_pendiente(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.push(opcode);
    code.push(0); // el hueco
    code.len() - 1
}

/// Rellena el hueco para que caiga **aquí**.
fn aterriza_aqui(code: &mut Vec<u8>, hueco: usize) {
    let destino = code.len() as i64;
    let origen = hueco as i64 + 1; // el salto es relativo al final del salto
    code[hueco] = (destino - origen) as i8 as u8;
}

/// Salto corto HACIA ATRÁS, a una posición ya conocida.
fn salto_atras(code: &mut Vec<u8>, opcode: u8, destino: usize) {
    code.push(opcode);
    let origen = code.len() as i64 + 1;
    code.push((destino as i64 - origen) as i8 as u8);
}

/// `copiar(dst, src, n)` — mueve `n` bytes. Espera `RDI`=dst, `RSI`=src,
/// `RCX`=n. No devuelve nada útil en `RAX` (quien llama ya tiene `dst`).
///
/// Copia **hacia adelante**, así que solapamientos con `dst < src` salen bien
/// y con `dst > src` no. Es exactamente el contrato de `memcpy` — el que
/// aguanta los dos es `memmove`, y se dice aquí para que nadie lo suponga.
pub fn copiar(code: &mut Vec<u8>) {
    // copiar cero bytes es válido y frecuente (un bucle recién vaciado)
    code.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
    let fin = salto_pendiente(code, 0x74);       // jz fin
    let bucle = code.len();
    code.extend_from_slice(&[0x8A, 0x06]);       // mov al, [rsi]
    code.extend_from_slice(&[0x88, 0x07]);       // mov [rdi], al
    code.extend_from_slice(&[0x48, 0xFF, 0xC6]); // inc rsi
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]); // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC9]); // dec rcx
    salto_atras(code, 0x75, bucle);              // jnz bucle
    aterriza_aqui(code, fin);
}

/// `rellenar(dst, valor, n)` — pone `n` bytes al mismo valor.
/// `RDI`=dst, `RAX`=valor (se usa `al`), `RCX`=n.
pub fn rellenar(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
    let fin = salto_pendiente(code, 0x74);       // jz fin
    let bucle = code.len();
    code.extend_from_slice(&[0x88, 0x07]);       // mov [rdi], al
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]); // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC9]); // dec rcx
    salto_atras(code, 0x75, bucle);              // jnz bucle
    aterriza_aqui(code, fin);
}

/// `largo(s)` — cuántos bytes hay antes del cero. `RDI`=s, resultado en `RAX`.
///
/// El terminador NO se cuenta, que es lo que dice `strlen` y lo que casi todo
/// el mundo se equivoca al reimplementarlo.
pub fn largo(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax  (contador)
    let bucle = code.len();
    code.extend_from_slice(&[0x8A, 0x0C, 0x07]); // mov cl, [rdi+rax]
    code.extend_from_slice(&[0x84, 0xC9]);       // test cl, cl
    let fin = salto_pendiente(code, 0x74);       // jz fin
    code.extend_from_slice(&[0x48, 0xFF, 0xC0]); // inc rax
    salto_atras(code, 0xEB, bucle);              // jmp bucle
    aterriza_aqui(code, fin);
}

/// `comparar(a, b)` — 0 si son iguales; distinto de 0 si no.
/// `RDI`=a, `RSI`=b, resultado en `RAX`.
///
/// ★ Devuelve la DIFERENCIA del primer byte que cambia, con signo, igual que
/// `strcmp`. Un `comparar` que sólo dijera "iguales / distintas" parecería
/// suficiente hasta el día que alguien ordene una lista con él.
pub fn comparar(code: &mut Vec<u8>) {
    let bucle = code.len();
    code.extend_from_slice(&[0x0F, 0xB6, 0x07]);       // movzx eax, byte [rdi]
    code.extend_from_slice(&[0x0F, 0xB6, 0x0E]);       // movzx ecx, byte [rsi]
    code.extend_from_slice(&[0x29, 0xC8]);             // sub eax, ecx
    let distintos = salto_pendiente(code, 0x75);       // jnz fin
    code.extend_from_slice(&[0x84, 0xC9]);             // test cl, cl (¿fin de cadena?)
    let iguales = salto_pendiente(code, 0x74);         // jz fin
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]);       // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC6]);       // inc rsi
    salto_atras(code, 0xEB, bucle);                    // jmp bucle
    aterriza_aqui(code, distintos);
    aterriza_aqui(code, iguales);
    // rax ya lleva la diferencia; se extiende el signo a 64 bits
    code.extend_from_slice(&[0x48, 0x63, 0xC0]);       // movsxd rax, eax
}

/// `comparar_n(a, b, n)` — como [`comparar`] pero **con tope**.
///
/// `RDI`=a, `RSI`=b, `RDX`=n. Resultado en `RAX`: la diferencia con signo del
/// primer byte que cambia, o `0` si los `n` primeros bytes son iguales.
///
/// `parar_en_cero` decide cuál de las dos funciones de C es:
///
/// - `true` → **`strncmp`**: además del tope, se para en el terminador. Si los
///   dos llegan al `\0` a la vez son iguales aunque queden bytes de cupo.
/// - `false` → **`memcmp`**: sólo el tope. El cero es un byte más.
///
/// ★ La diferencia importa y se paga cara al confundirla: `memcmp` sobre dos
/// nombres cortos seguiría comparando **lo que hubiera detrás del cero**, que
/// es memoria de otro y basura distinta en cada ejecución. Es el fallo que da
/// "a veces sí y a veces no" y se busca en el sitio equivocado.
pub fn comparar_n(code: &mut Vec<u8>, parar_en_cero: bool) {
    // Con n = 0 son iguales por definición, y hay que salir ANTES de leer: los
    // punteros pueden ser inválidos si no hay nada que comparar, y eso es legal
    // en C.
    code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax
    code.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
    let vacio = salto_pendiente(code, 0x74); // jz fin

    let bucle = code.len();
    code.extend_from_slice(&[0x0F, 0xB6, 0x07]); // movzx eax, byte [rdi]
    code.extend_from_slice(&[0x0F, 0xB6, 0x0E]); // movzx ecx, byte [rsi]
    code.extend_from_slice(&[0x29, 0xC8]); // sub eax, ecx
    let distintos = salto_pendiente(code, 0x75); // jnz fin

    let iguales = if parar_en_cero {
        code.extend_from_slice(&[0x84, 0xC9]); // test cl, cl
        Some(salto_pendiente(code, 0x74)) // jz fin (los dos acabaron)
    } else {
        None
    };

    code.extend_from_slice(&[0x48, 0xFF, 0xC7]); // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC6]); // inc rsi
    code.extend_from_slice(&[0x48, 0xFF, 0xCA]); // dec rdx
    code.extend_from_slice(&[0x48, 0x85, 0xD2]); // test rdx, rdx
    let agotado = salto_pendiente(code, 0x74); // jz fin
    salto_atras(code, 0xEB, bucle); // jmp bucle

    // Se agotó el cupo sin diferencias: iguales. `rax` trae el último `sub`,
    // que fue cero — pero se pone a cero explícitamente porque depender de eso
    // es depender de por dónde se salió.
    aterriza_aqui(code, agotado);
    code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax
    let sal = salto_pendiente(code, 0xEB);

    aterriza_aqui(code, distintos);
    if let Some(h) = iguales {
        aterriza_aqui(code, h);
    }
    code.extend_from_slice(&[0x48, 0x63, 0xC0]); // movsxd rax, eax
    aterriza_aqui(code, sal);
    aterriza_aqui(code, vacio);
}

/// `buscar(s, c)` — **`strchr`**: dirección de la primera `c` en `s`, o `0`.
///
/// `RDI`=s, `RSI`=c (byte en `SIL`). Resultado en `RAX`.
///
/// ★ Buscar el `\0` **encuentra el terminador**, no devuelve `0`. Es lo que
/// dice el estándar y es la forma normal de saber dónde acaba una cadena; un
/// `strchr` que tratara el cero como "no encontrado" fallaría sólo en el caso
/// en que alguien lo use a propósito.
/// ⚠️ **Sin registros de 8 bits con prefijo REX.** La primera versión usaba
/// `cmp al, sil` (`40 38 F0`) y el emulador la rechazó con
/// *"opcode 0x38 no emitido por BMO"* — que es exactamente para lo que sirve
/// ese `panic!`: el decodificador sólo entiende lo que BMO emite de verdad, así
/// que estrenar una forma nueva se nota en el acto en vez de dar un resultado
/// raro. Aquí se compara con `movzx` + `sub`, igual que [`comparar`].
pub fn buscar(code: &mut Vec<u8>) {
    // El byte buscado, aislado en `rcx`: llega en `rsi` como entero y hay que
    // quedarse sólo con el byte bajo, o `strchr(s, 0x141)` encontraría una 'A'.
    code.extend_from_slice(&[0x48, 0x89, 0xF1]); // mov rcx, rsi
    code.extend_from_slice(&[0x0F, 0xB6, 0xC9]); // movzx ecx, cl

    let bucle = code.len();
    code.extend_from_slice(&[0x0F, 0xB6, 0x07]); // movzx eax, byte [rdi]
    code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
    let cero = salto_pendiente(code, 0x74); // jz -> es el final
    code.extend_from_slice(&[0x0F, 0xB6, 0x07]); // movzx eax, byte [rdi]
    code.extend_from_slice(&[0x29, 0xC8]); // sub eax, ecx
    let encontrado = salto_pendiente(code, 0x74); // jz encontrado
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]); // inc rdi
    salto_atras(code, 0xEB, bucle);

    // El terminador: se encuentra si es lo que se buscaba, y si no, no hay.
    aterriza_aqui(code, cero);
    code.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
    let buscaba_el_cero = salto_pendiente(code, 0x74); // jz encontrado
    code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax
    let sal = salto_pendiente(code, 0xEB);

    aterriza_aqui(code, encontrado);
    aterriza_aqui(code, buscaba_el_cero);
    code.extend_from_slice(&[0x48, 0x89, 0xF8]); // mov rax, rdi
    aterriza_aqui(code, sal);
}

/// `absoluto(n)` — el valor absoluto de `RAX`, en `RAX`. Sin ramas.
///
/// La forma clásica: propagar el signo con un desplazamiento aritmético y
/// hacer `(n XOR s) - s`. Vale para `INT_MIN` de la misma forma que vale en C
/// —es decir, se desborda igual—, y eso también es el contrato.
pub fn absoluto(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xC1]);       // mov rcx, rax
    code.extend_from_slice(&[0x48, 0xC1, 0xF9, 0x3F]); // sar rcx, 63
    code.extend_from_slice(&[0x48, 0x31, 0xC8]);       // xor rax, rcx
    code.extend_from_slice(&[0x48, 0x29, 0xC8]);       // sub rax, rcx
}
