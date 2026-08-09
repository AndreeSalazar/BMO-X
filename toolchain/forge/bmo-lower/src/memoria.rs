//! **L1 -- bloques de bytes**: copiar, rellenar, comparar, medir.
//!
//! # Por que esto vive en L1 y no en el frontend de C
//!
//! Lo que C escribe `memcpy(a,b,n)`, COBOL lo escribe `MOVE` de un grupo a
//! otro y Ada lo escribe con una asignacion de array. **Es la misma operacion
//! y la misma emision**: mover `n` bytes de un sitio a otro. Lo unico distinto
//! es como se deletrea.
//!
//! Esa es exactamente la frontera que este crate defiende: L1 tiene lo
//! expresable sin semantica de lenguaje, y "mueve estos bytes" no tiene
//! ninguna. Si esto se escribiera en `lang/c`, el dia que COBOL necesite mover
//! un grupo grande habria dos copias del mismo bucle -- y una de las dos
//! tendria el bug.
//!
//! # Por que se emite EN LINEA y no se llama a una libreria
//!
//! Porque aqui no hay libreria que enlazar, y eso no es una carencia: es el
//! modelo. Un `.bex` es una imagen entera y BEF no tiene relocaciones que
//! resolver contra un `.so`. Emitir el bucle cuesta ~30 bytes y ahorra un
//! enlazador, un formato de libreria y un cargador dinamico -- las tres cosas
//! que en otros sistemas hay que pelear antes de imprimir "hola".
//!
//! # Lo que NO hay, y se dice
//!
//! No hay version vectorizada. Un `memcpy` de glibc elige entre una docena de
//! rutas segun el tamano y el modelo de CPU; este copia de byte en byte. Para
//! mover el framebuffer de DOOM (64 000 bytes por fotograma) eso se va a
//! notar, y cuando se note se cambia por copias de 8 bytes con cola --
//! **medido primero**. Optimizar un bucle que nadie ha cronometrado es
//! adivinar.

use crate::x86::*;

// -- Saltos que NO se cuentan a mano -----------------------------------
//
// * La primera version de este modulo llevaba los desplazamientos escritos a
// mano (`jz +10`, `jnz +11`) y **tres de los cuatro estaban mal por uno**. El
// emulador lo cazo de la peor forma posible y por eso de la mejor: salto a
// mitad de una instruccion y siguio decodificando, y lo que salio fue
// `opcode 0xEA no emitido por BMO` -- un opcode que nadie habia emitido nunca.
//
// Contar bytes de instruccion a mano es exactamente el trabajo que una maquina
// hace sin equivocarse. Se emite un hueco, se apunta donde esta, y se rellena
// cuando ya se sabe la distancia.

/// Emite un salto corto con destino pendiente. Devuelve donde esta el hueco.
fn salto_pendiente(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.push(opcode);
    code.push(0); // el hueco
    code.len() - 1
}

/// Rellena el hueco para que caiga **aqui**.
fn aterriza_aqui(code: &mut Vec<u8>, free_slot: usize) {
    let destino = code.len() as i64;
    let origen = free_slot as i64 + 1; // el salto es relativo al final del salto
    code[free_slot] = (destino - origen) as i8 as u8;
}

/// Salto corto HACIA ATRAS, a una posicion ya conocida.
fn salto_atras(code: &mut Vec<u8>, opcode: u8, destino: usize) {
    code.push(opcode);
    let origen = code.len() as i64 + 1;
    code.push((destino as i64 - origen) as i8 as u8);
}

/// `copiar(dst, src, n)` -- mueve `n` bytes. Espera `RDI`=dst, `RSI`=src,
/// `RCX`=n. No devuelve nada util en `RAX` (quien llama ya tiene `dst`).
///
/// Copia **hacia adelante**, asi que solapamientos con `dst < src` salen bien
/// y con `dst > src` no. Es exactamente el contrato de `memcpy` -- el que
/// aguanta los dos es `memmove`, y se dice aqui para que nadie lo suponga.
pub fn copiar(code: &mut Vec<u8>) {
    // copiar cero bytes es valido y frecuente (un bucle recien vaciado)
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

/// `mover(dst, src, n)` -- lo que `copiar` promete y no cumple: **aguanta el
/// solapamiento**. `RDI`=dst, `RSI`=src, `RCX`=n.
///
/// # La unica idea que hay aqui
///
/// Si el destino esta POR ENCIMA del origen y los bloques se tocan, copiar de
/// frente pisa bytes que todavia no se han leido. Entonces se copia **desde el
/// final**. Si el destino esta por debajo, de frente va bien -- y es el caso
/// comun, asi que no se paga la vuelta.
///
/// La comparacion es SIN SIGNO (`jbe`): son direcciones, no numeros. Con `jle`
/// una direccion por encima de la mitad del espacio se leeria como negativa y
/// elegiria la rama contraria, que es la clase de fallo que solo aparece
/// cuando el programa crece.
///
/// # Por que esto no estaba
///
/// `memmove` compartia arm con `memcpy` y emitia [`copiar`] -- o sea que era un
/// `memcpy` con otro nombre. El comentario del codegen lo reconocia por escrito
/// desde hacia tiempo, y esa es justamente la trampa: **un fallo confesado en
/// prosa sigue siendo un fallo**. Nada lo ejecutaba, asi que nada lo cazaba.
///
/// Lo cazo una fila del banco que copia `b[0..4]` sobre `b[2..6]` y espera
/// `ababcd`. Salia `ababab`.
pub fn mover(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xC9]); // test rcx, rcx
    let fin_vacio = salto_pendiente(code, 0x74); // jz fin

    // dst <= src -> de frente. Sin signo: son direcciones.
    code.extend_from_slice(&[0x48, 0x39, 0xF7]); // cmp rdi, rsi
    let adelante = salto_pendiente(code, 0x76);  // jbe adelante

    // -- Hacia atras: los dos punteros al ULTIMO byte del bloque.
    code.extend_from_slice(&[0x48, 0x01, 0xCF]); // add rdi, rcx
    code.extend_from_slice(&[0x48, 0x01, 0xCE]); // add rsi, rcx
    code.extend_from_slice(&[0x48, 0xFF, 0xCF]); // dec rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xCE]); // dec rsi
    let bucle_atras = code.len();
    code.extend_from_slice(&[0x8A, 0x06]);       // mov al, [rsi]
    code.extend_from_slice(&[0x88, 0x07]);       // mov [rdi], al
    code.extend_from_slice(&[0x48, 0xFF, 0xCE]); // dec rsi
    code.extend_from_slice(&[0x48, 0xFF, 0xCF]); // dec rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC9]); // dec rcx
    salto_atras(code, 0x75, bucle_atras);        // jnz bucle_atras
    let fin_atras = salto_pendiente(code, 0xEB); // jmp fin

    // -- De frente: el mismo bucle que `copiar`.
    aterriza_aqui(code, adelante);
    let bucle = code.len();
    code.extend_from_slice(&[0x8A, 0x06]);       // mov al, [rsi]
    code.extend_from_slice(&[0x88, 0x07]);       // mov [rdi], al
    code.extend_from_slice(&[0x48, 0xFF, 0xC6]); // inc rsi
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]); // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC9]); // dec rcx
    salto_atras(code, 0x75, bucle);              // jnz bucle

    aterriza_aqui(code, fin_atras);
    aterriza_aqui(code, fin_vacio);
}

/// `rellenar(dst, valor, n)` -- pone `n` bytes al mismo valor.
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

/// `largo(s)` -- cuantos bytes hay antes del cero. `RDI`=s, resultado en `RAX`.
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

/// `comparar(a, b)` -- 0 si son iguales; distinto de 0 si no.
/// `RDI`=a, `RSI`=b, resultado en `RAX`.
///
/// * Devuelve la DIFERENCIA del primer byte que cambia, con signo, igual que
/// `strcmp`. Un `comparar` que solo dijera "iguales / distintas" pareceria
/// suficiente hasta el dia que alguien ordene una lista con el.
pub fn comparar(code: &mut Vec<u8>) {
    let bucle = code.len();
    code.extend_from_slice(&[0x0F, 0xB6, 0x07]);       // movzx eax, byte [rdi]
    code.extend_from_slice(&[0x0F, 0xB6, 0x0E]);       // movzx ecx, byte [rsi]
    code.extend_from_slice(&[0x29, 0xC8]);             // sub eax, ecx
    let distintos = salto_pendiente(code, 0x75);       // jnz fin
    code.extend_from_slice(&[0x84, 0xC9]);             // test cl, cl (fin de cadena?)
    let iguales = salto_pendiente(code, 0x74);         // jz fin
    code.extend_from_slice(&[0x48, 0xFF, 0xC7]);       // inc rdi
    code.extend_from_slice(&[0x48, 0xFF, 0xC6]);       // inc rsi
    salto_atras(code, 0xEB, bucle);                    // jmp bucle
    aterriza_aqui(code, distintos);
    aterriza_aqui(code, iguales);
    // rax ya lleva la diferencia; se extiende el signo a 64 bits
    code.extend_from_slice(&[0x48, 0x63, 0xC0]);       // movsxd rax, eax
}

/// `comparar_n(a, b, n)` -- como [`comparar`] pero **con tope**.
///
/// `RDI`=a, `RSI`=b, `RDX`=n. Resultado en `RAX`: la diferencia con signo del
/// primer byte que cambia, o `0` si los `n` primeros bytes son iguales.
///
/// `parar_en_cero` decide cual de las dos funciones de C es:
///
/// - `true` -> **`strncmp`**: ademas del tope, se para en el terminador. Si los
///   dos llegan al `\0` a la vez son iguales aunque queden bytes de cupo.
/// - `false` -> **`memcmp`**: solo el tope. El cero es un byte mas.
///
/// * La diferencia importa y se paga cara al confundirla: `memcmp` sobre dos
/// nombres cortos seguiria comparando **lo que hubiera detras del cero**, que
/// es memoria de otro y basura distinta en cada ejecucion. Es el fallo que da
/// "a veces si y a veces no" y se busca en el sitio equivocado.
pub fn comparar_n(code: &mut Vec<u8>, parar_en_cero: bool) {
    // Con n = 0 son iguales por definicion, y hay que salir ANTES de leer: los
    // punteros pueden ser invalidos si no hay nada que comparar, y eso es legal
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

    // Se agoto el cupo sin diferencias: iguales. `rax` trae el ultimo `sub`,
    // que fue cero -- pero se pone a cero explicitamente porque depender de eso
    // es depender de por donde se salio.
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

/// `find_by(s, c)` -- **`strchr`**: direccion de la primera `c` en `s`, o `0`.
///
/// `RDI`=s, `RSI`=c (byte en `SIL`). Resultado en `RAX`.
///
/// * Buscar el `\0` **encuentra el terminador**, no devuelve `0`. Es lo que
/// dice el estandar y es la forma normal de saber donde acaba una cadena; un
/// `strchr` que tratara el cero como "no encontrado" fallaria solo en el caso
/// en que alguien lo use a proposito.
/// [!] **Sin registros de 8 bits con prefijo REX.** La primera version usaba
/// `cmp al, sil` (`40 38 F0`) y el emulador la rechazo con
/// *"opcode 0x38 no emitido por BMO"* -- que es exactamente para lo que sirve
/// ese `panic!`: el decodificador solo entiende lo que BMO emite de verdad, asi
/// que estrenar una forma nueva se nota en el acto en vez de dar un resultado
/// raro. Aqui se compara con `movzx` + `sub`, igual que [`comparar`].
pub fn find_by(code: &mut Vec<u8>) {
    // El byte buscado, aislado en `rcx`: llega en `rsi` como entero y hay que
    // quedarse solo con el byte bajo, o `strchr(s, 0x141)` encontraria una 'A'.
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

/// `absoluto(n)` -- el valor absoluto de `RAX`, en `RAX`. Sin ramas.
///
/// La forma clasica: propagar el signo con un desplazamiento aritmetico y
/// hacer `(n XOR s) - s`. Vale para `INT_MIN` de la misma forma que vale en C
/// --es decir, se desborda igual--, y eso tambien es el contrato.
pub fn absoluto(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xC1]);       // mov rcx, rax
    code.extend_from_slice(&[0x48, 0xC1, 0xF9, 0x3F]); // sar rcx, 63
    code.extend_from_slice(&[0x48, 0x31, 0xC8]);       // xor rax, rcx
    code.extend_from_slice(&[0x48, 0x29, 0xC8]);       // sub rax, rcx
}
