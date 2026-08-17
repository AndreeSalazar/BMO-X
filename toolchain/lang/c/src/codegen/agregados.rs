//! **Structs y uniones POR VALOR**: copiarlos, pasarlos y devolverlos.
//!
//! === Por que esto es un fichero aparte ===
//!
//! Todo lo demas que emite BMO C cabe en un registro. Un `int`, un puntero, un
//! `char`: se calculan en `rax` y se guardan. Un agregado **no cabe**, y esa
//! sola diferencia cambia las tres cosas que un valor sabe hacer --asignarse,
//! viajar a una funcion y volver de ella-- en tres mecanismos distintos que no
//! se parecen a los de un escalar.
//!
//! Metidos en `emit_expr`, cada uno seria una rama mas en un `match` que ya
//! tiene sesenta. Aqui son tres funciones con nombre, y la ABI de BMO para
//! agregados se lee de un tiron.
//!
//! === Como lo hacen los que saben ===
//!
//! El x86-64 **SysV** (Linux, macOS, BSD) clasifica cada agregado con un
//! algoritmo de verdad: se parte en trozos de 8 bytes ("eightbytes"), cada uno
//! se etiqueta INTEGER / SSE / MEMORY, y segun eso el struct viaja en registros
//! enteros, en registros SSE, mezclado, o por memoria. Un `struct {double a;
//! double b;}` va en `xmm0`+`xmm1`; un `struct {int a; float b;}` va en `rax`
//! entero. `X86_64ABIInfo::classify` de Clang es de las piezas mas sutiles de
//! todo el backend, y GCC tiene la suya (`classify_argument`) con los mismos
//! casos de esquina. La razon de tanta complicacion es el RENDIMIENTO: pasar
//! dos `double` en registros ahorra ir a memoria.
//!
//! **Win64** hace lo contrario: si el agregado no mide exactamente 1, 2, 4 u 8
//! bytes, se pasa **por referencia oculta** -- el llamante hace una copia y pasa
//! su direccion. Una regla, sin clasificacion.
//!
//! === Lo que hace BMO, y por que ===
//!
//! BMO **es dueno de su propia ABI** --todos los argumentos van por la pila, no
//! en registros-- asi que la clasificacion de SysV no compra nada aqui: no hay
//! registros de argumento que repartir. Se elige la regla mas simple que sea
//! correcta, al estilo Win64 pero sin la copia extra:
//!
//! - **Argumento**: el agregado ocupa `techo(tamano/8)` ranuras CONSECUTIVAS de
//!   la pila, copiado por el llamante. Es *por valor* de forma literal: lo que
//!   la funcion recibe son sus bytes, en su marco.
//!
//! - **Retorno**: **puntero oculto en `rdi`**. El llamante reserva el hueco en
//!   su propio marco y pasa la direccion; la funcion escribe ahi dentro. Es lo
//!   mismo que hace SysV para los agregados de clase MEMORY (`sret`), y lo que
//!   hace todo compilador para lo que no cabe en dos registros.
//!
//!   `rdi` y no una ranura mas de pila para no correr los indices de los
//!   parametros -- un cambio que tocaria el llamante, el prologo y `build_var_map`
//!   a la vez. Se pone **justo antes del `call`**, cuando ya no queda ninguna
//!   expresion por evaluar: un argumento que contenga `__syscall(...)` usa `rdi`
//!   para el suyo, y ponerlo antes lo perderia.
//!
//! - **Asignacion**: copia byte a byte del tamano exacto.
//!
//! === Lo que habia antes, y era mudo ===
//!
//! `p = q` emitia `mov rax,[q]` + `mov [p],rax`: **ocho bytes**, los que
//! quepan. Un `struct` de 12 se copiaba a medias y los otros cuatro se
//! quedaban con lo que hubiera. Y pasar uno a una funcion empujaba una sola
//! palabra, asi que la funcion recibia el primer campo y basura detras.
//! Ninguno de los dos avisaba.

use crate::ast::*;
use super::Codegen;

/// Cuantas ranuras de 8 bytes ocupa un valor de este tipo como argumento.
///
/// Uno siempre, aunque el tipo sea de un byte: la pila se mueve de 8 en 8 y
/// romper esa regla desalinearia todo lo de detras.
/// La regla vive en `bmo_abi::types::disposicion::ranuras`: **es la convencion
/// de llamada de BMO**, no una decision de C. Estaba aqui como `pub(super)` y a
/// la vez documentada como ABI en `toolchain/lang/cpp/CPP_ABI.md` -- y una regla que un
/// documento llama ABI y el arbol guarda dentro de un lenguaje es una regla que
/// el segundo lenguaje copia.
pub(super) use bmo_abi::types::ranuras;

impl Codegen {
    /// Este tipo viaja por valor como agregado (no cabe en un registro)?
    ///
    /// Un `struct` de 8 bytes o menos **tambien** cuenta: podria caber en `rax`,
    /// pero tratarlo distinto obligaria a que el llamante y la funcion se
    /// pusieran de acuerdo sobre el tamano, y ese es justo el desacuerdo que
    /// produce basura silenciosa. Una regla, sin casos de esquina.
    pub(super) fn es_agregado(&self, t: &TypeSpec) -> bool {
        matches!(t, TypeSpec::StructRef(_) | TypeSpec::UnionRef(_))
    }

    /// Copia `bytes` de `[rsi]` a `[rdi]`. Deja los dos punteros movidos.
    ///
    /// De 8 en 8 mientras quepa, y el resto byte a byte. Sin `rep movsb` a
    /// proposito: el emulador no lo tiene, y el bucle desenrollado es mas corto
    /// que la instruccion de cadena para los tamanos de un struct de verdad.
    fn emit_copia_rsi_rdi(&mut self, bytes: u32) {
        let mut hecho = 0u32;
        while bytes - hecho >= 8 {
            // mov rax, [rsi+disp] ; mov [rdi+disp], rax
            self.emit_mov_rax_mem(0x06, hecho);
            self.emit_mov_mem_rax(0x07, hecho);
            hecho += 8;
        }
        while hecho < bytes {
            // movzx eax, byte [rsi+disp] ; mov [rdi+disp], al
            self.code.extend_from_slice(&[0x0F, 0xB6]);
            self.emit_modrm_disp(0x06, hecho);
            self.code.extend_from_slice(&[0x88]);
            self.emit_modrm_disp(0x07, hecho);
            hecho += 1;
        }
    }

    /// `mov rax, [reg+disp]` con el ModRM del registro base dado.
    fn emit_mov_rax_mem(&mut self, base_rm: u8, disp: u32) {
        self.code.extend_from_slice(&[0x48, 0x8B]);
        self.emit_modrm_disp(base_rm, disp);
    }

    /// `mov [reg+disp], rax`.
    fn emit_mov_mem_rax(&mut self, base_rm: u8, disp: u32) {
        self.code.extend_from_slice(&[0x48, 0x89]);
        self.emit_modrm_disp(base_rm, disp);
    }

    /// El ModRM `reg=rax, rm=<base>` con desplazamiento de 8 o 32 bits.
    fn emit_modrm_disp(&mut self, base_rm: u8, disp: u32) {
        if disp <= 127 {
            self.code.push(0x40 | base_rm); // mod=01 (disp8), reg=000 (rax)
            self.code.push(disp as u8);
        } else {
            self.code.push(0x80 | base_rm); // mod=10 (disp32)
            self.code.extend_from_slice(&disp.to_le_bytes());
        }
    }

    /// **Asignar** un agregado: `destino = origen`, copiando su tamano exacto.
    ///
    /// `destino` y `origen` se evaluan COMO DIRECCION, no como valor -- un
    /// agregado no tiene "valor" que quepa en `rax`; lo que hay es donde vive.
    pub(super) fn emit_asigna_agregado(&mut self, destino: &Expr, origen: &Expr, bytes: u32) {
        // El orden importa: se calcula el origen, se aparca, y luego el
        // destino. Al reves, evaluar el destino podria pisar `rsi`.
        self.emit_expr_as_ptr(origen);
        self.code.push(0x50); // push rax
        self.emit_expr_as_ptr(destino);
        self.code.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
        self.code.push(0x5E); // pop rsi
        self.emit_copia_rsi_rdi(bytes);
        // El valor de la expresion es la direccion del destino, por si alguien
        // encadena `a = b = c`.
        self.code.extend_from_slice(&[0x48, 0x89, 0xF8]); // mov rax, rdi
    }

    /// **Empujar** un agregado como argumento: sus palabras, en ranuras.
    ///
    /// * De la ULTIMA a la primera. La pila crece hacia abajo, asi que empujar
    /// primero la palabra alta deja la baja en la direccion menor -- que es donde
    /// la funcion espera el byte 0 del struct. Al reves, el struct llega dado la
    /// vuelta y todos los campos salen cambiados de sitio.
    pub(super) fn emit_empuja_agregado(&mut self, arg: &Expr, bytes: u32) {
        let n = ranuras(bytes);
        self.emit_expr_as_ptr(arg);
        self.code.extend_from_slice(&[0x48, 0x89, 0xC6]); // mov rsi, rax
        for k in (0..n).rev() {
            // mov rax, [rsi + k*8] ; push rax
            self.emit_mov_rax_mem(0x06, k * 8);
            self.code.push(0x50);
        }
    }
}
