//! **La entrada de C**: `getchar` y `scanf`.
//!
//! === Por que esto es un fichero aparte ===
//!
//! Es la mitad que faltaba de `printf`, y no se parece a ella en nada. Escribir
//! es empujar bytes por una puerta que no puede fallar. **Leer es esperar**: hay
//! que ceder el turno, guardar lo que llega de mas, y decidir que significa lo
//! que una persona tecleo. Tres problemas que `emit_printf_variadic` no tiene.
//!
//! === Lo que hereda de L1 ===
//!
//! Casi todo. `bmo_lower::console::{read_char, read_line}` ya saben esperar sin
//! comerse el quantum, y `fmt::parse_decimal_scaled` ya sabe leer digitos. Aqui
//! solo se decide **que significa cada `%`** -- que es lo especifico de C y no
//! tiene por que bajar a una libreria compartida. La misma frontera que en la
//! salida: el formato es de C, mover bytes es de todos.
//!
//! === Lo que NO hace, y se dice ===
//!
//! Un `scanf` de verdad lee **varias conversiones de un flujo**, con reglas de
//! espacio en blanco que ocupan pagina y media del estandar (section 7.21.6.2), un
//! valor de retorno que cuenta asignaciones, y `%n`, `%[`, anchuras... Aqui se
//! admite **una conversion por llamada**, y mas de una se rechaza diciendo que
//! se parta en dos. Fingir el resto seria peor que no tenerlo: un `scanf` que
//! ignora la mitad de su formato es un programa que lee mal en silencio.

use crate::ast::*;
use super::Codegen;

/// El buffer de [`bmo_lower::console::read_char`]: 8 bytes de datos + 1 de
/// cuenta, redondeado a 16 para no descolocar lo que venga detras.
///
/// Es una global OCULTA, y tiene que serlo: `getchar` se emite en linea en cada
/// sitio donde se llama --aqui no hay runtime que enlazar-- y los siete bytes que
/// sobran de un paquete tienen que sobrevivir de una llamada a la siguiente. En
/// la pila no sobrevivirian.
const BUF_ENTRADA: &str = "__bmo_entrada";
const BUF_ENTRADA_BYTES: u32 = 16;

/// Cuantos bytes de linea acepta un `scanf`. Es el mismo tope que el renglon
/// del kernel y que la caja del compositor: pasarse en uno de los tres deja que
/// el texto se corte en silencio en otro.
const LINEA_MAX: u8 = 127;

impl Codegen {
    /// Reserva el buffer de entrada si todavia no esta. Idempotente: dos
    /// `getchar()` comparten el mismo, que es justo el punto.
    fn reservar_buffer_entrada(&mut self) -> String {
        if !self.global_offsets.contains_key(BUF_ENTRADA) {
            let pad = (8 - self.global_data.len() as u32 % 8) % 8;
            for _ in 0..pad {
                self.global_data.push(0);
            }
            let off = self.global_data.len() as u32;
            for _ in 0..BUF_ENTRADA_BYTES {
                self.global_data.push(0);
            }
            self.global_offsets
                .insert(BUF_ENTRADA.to_string(), (off, TypeSpec::Long));
        }
        BUF_ENTRADA.to_string()
    }

    /// `lea rdi, [rip + buffer_de_entrada]`.
    fn emit_lea_rdi_buffer(&mut self) {
        let name = self.reservar_buffer_entrada();
        self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
        self.global_fixups.push((self.code.len() - 4, name));
    }

    /// `getchar()` -- el siguiente byte, **bloqueando**.
    ///
    /// Devuelve el byte en `rax`. No devuelve `EOF` nunca: una consola de BMO
    /// no se acaba, se queda esperando. Un `while ((c = getchar()) != EOF)`
    /// giraria para siempre, y por eso el ejemplo de la biblioteca corta con
    /// `'\n'` y no con `EOF`.
    pub(super) fn emit_getchar(&mut self) {
        self.emit_lea_rdi_buffer();
        bmo_lower::console::read_char(&mut self.code);
    }

    /// `scanf(fmt, &destino)` -- una conversion.
    pub(super) fn emit_scanf(&mut self, args: &[Expr]) {
        let Some(Expr::StringLit(formato)) = args.first() else {
            self.errors.push(
                "scanf con formato calculado en tiempo de ejecucion no se compila: \
                 el formato debe ser un literal"
                    .to_string(),
            );
            return;
        };
        let formato = formato.clone();
        let conversiones: Vec<char> = {
            let b: Vec<char> = formato.chars().collect();
            let mut v = Vec::new();
            let mut i = 0;
            while i < b.len() {
                if b[i] == '%' && i + 1 < b.len() {
                    if b[i + 1] == '%' {
                        i += 2;
                        continue;
                    }
                    v.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                i += 1;
            }
            v
        };

        if conversiones.len() != 1 || args.len() != 2 {
            self.errors.push(format!(
                "scanf admite UNA conversion y un destino por llamada (este trae {} y {} \
                 argumento(s)); partelo en varias llamadas",
                conversiones.len(),
                args.len().saturating_sub(1)
            ));
            return;
        }
        let conv = conversiones[0];
        let destino = args[1].clone();

        match conv {
            'c' => {
                // Un caracter: ni linea ni espacios de por medio.
                self.emit_getchar();
                self.emit_guardar_en_destino(&destino, &TypeSpec::Char);
            }
            'd' | 'i' => {
                self.emit_leer_linea_a_pila();
                // `read_line` deja `r8` al FINAL de lo leido; el parser lo
                // quiere al principio. Se vuelve a apuntar en vez de guardar
                // una copia en un registro que el `syscall` de dentro pisaria.
                self.emit_lea_r8_pila();
                bmo_lower::fmt::parse_decimal_scaled(&mut self.code, 0);
                self.emit_cerrar_hueco_pila();
                self.emit_guardar_en_destino(&destino, &TypeSpec::Int);
            }
            's' => {
                // Directo al buffer del llamante: una cadena no pasa por la
                // pila para volver a copiarse.
                self.emit_expr(&destino);
                self.code.extend_from_slice(&[0x49, 0x89, 0xC0]); // mov r8, rax
                bmo_lower::console::read_line(&mut self.code, LINEA_MAX);
                // El cero final: `read_line` no lo pone porque devuelve la
                // longitud, y en C una cadena SIN cero no es una cadena.
                self.code.extend_from_slice(&[0x41, 0xC6, 0x00, 0x00]); // mov byte [r8], 0
            }
            otra => {
                self.errors.push(format!(
                    "scanf: '%{otra}' aun no se compila (se compilan %d %i %c %s)"
                ));
            }
        }
    }

    /// Abre `LINEA_MAX+1` bytes en la pila y lee una linea ahi.
    fn emit_leer_linea_a_pila(&mut self) {
        let free_slot = (LINEA_MAX as i32 + 1 + 15) / 16 * 16;
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
        self.code.extend_from_slice(&(free_slot as u32).to_le_bytes());
        self.emit_lea_r8_pila();
        bmo_lower::console::read_line(&mut self.code, LINEA_MAX);
    }

    /// `lea r8, [rsp]`.
    fn emit_lea_r8_pila(&mut self) {
        self.code.extend_from_slice(&[0x4C, 0x8D, 0x04, 0x24]);
    }

    fn emit_cerrar_hueco_pila(&mut self) {
        let free_slot = (LINEA_MAX as i32 + 1 + 15) / 16 * 16;
        self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
        self.code.extend_from_slice(&(free_slot as u32).to_le_bytes());
    }

    /// Guarda `rax` donde apunta `destino`, del tamano de `tipo`.
    ///
    /// `destino` es una expresion que da una DIRECCION -- el `&x` del llamante.
    /// Se aparca el valor porque evaluar la direccion usa `rax` tambien.
    fn emit_guardar_en_destino(&mut self, destino: &Expr, tipo: &TypeSpec) {
        self.code.push(0x50); // push rax (el valor leido)
        self.emit_expr(destino); // rax = direccion
        self.code.push(0x5A); // pop rdx (el valor)
        match tipo {
            TypeSpec::Char | TypeSpec::UnsignedChar => {
                self.code.extend_from_slice(&[0x88, 0x10]) // mov [rax], dl
            }
            TypeSpec::Short | TypeSpec::UnsignedShort => {
                self.code.extend_from_slice(&[0x66, 0x89, 0x10])
            }
            TypeSpec::Int | TypeSpec::UnsignedInt => {
                self.code.extend_from_slice(&[0x89, 0x10]) // mov [rax], edx
            }
            _ => self.code.extend_from_slice(&[0x48, 0x89, 0x10]),
        }
    }
}
