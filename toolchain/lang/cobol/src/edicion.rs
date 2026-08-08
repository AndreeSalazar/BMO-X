//! **PICTURE de edicion** -- el motor de mascaras de COBOL.
//!
//! `pic.rs` responde "cuantos digitos y cuanta escala". Esto responde la otra
//! mitad: **como se ESCRIBE ese numero para un humano**. Son cosas distintas y
//! por eso son dos modulos --
//!
//! ```text
//!   MOVE 1234567 TO TOTAL   con  PIC $$$,$$9.99   ->  "$12,345.67"
//!   MOVE -12000  TO SALDO   con  PIC Z,ZZ9.99CR   ->  "   120.00CR"
//!   MOVE 45      TO CHEQUE  con  PIC **,**9.99    ->  "*****0.45"
//! ```
//!
//! ## Por que esto es LA funcion bancaria
//!
//! Un informe de banco no es otra cosa que campos editados. El importe se
//! guarda como un entero exacto en centavos --esa es la aritmetica de
//! `codegen.rs`, el alma de Grace Hopper-- y aqui se convierte en la linea que
//! sale por la impresora: con su moneda, sus separadores de millar, sus ceros
//! suprimidos y su `CR` cuando el saldo esta en rojo.
//!
//! Sin este modulo, BMO COBOL sabe calcular y no sabe presentar. Con el, el
//! ciclo esta cerrado.
//!
//! ## El truco de la supresion
//!
//! Los ceros a la izquierda se sustituyen por espacio (`Z`) o asterisco (`*`),
//! y **los separadores que caen en esa zona se sustituyen tambien** -- por eso
//! `Z,ZZ9` con el valor 7 da `"    7"` y no `"   ,7"`. La supresion termina en
//! el primer digito significativo o en el punto decimal, lo que llegue antes.
//!
//! Los simbolos FLOTANTES (`$$$`, `---`, `+++`) son supresion con un remate:
//! el simbolo se coloca pegado al primer digito significativo. Se implementa
//! recordando donde acabo la supresion y escribiendolo ahi al final; intentar
//! colocarlo sobre la marcha obliga a mirar hacia adelante en cada posicion.
//!
//! ## Truncar, no redondear
//!
//! COBOL trunca salvo que se pida `ROUNDED`, y aqui no hay `ROUNDED` todavia.
//! Un `PIC 9V9` recibiendo `1.99` da `1.9`. Es lo que dice el estandar y es lo
//! que espera quien cuadra un balance a mano.

/// Un simbolo de la plantilla, ya expandido (sin la cuenta `(n)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sim {
    /// `9` -- digito que se escribe siempre, aunque sea cero.
    Digito,
    /// `Z` -- digito con supresion a espacio.
    CeroEspacio,
    /// `*` -- digito con supresion a asterisco (proteccion de cheque).
    CeroAsterisco,
    /// Posicion flotante de `$`, `+` o `-`. Consume digito y puede suprimir.
    Flotante(char),
    /// `$`, `+` o `-` en posicion fija (una sola, al principio o al final).
    Fijo(char),
    /// Caracter de insercion: `,` `.` `B` `0` `/`. No consume digito.
    Insercion(char),
    /// `CR` o `DB` al final. Solo se escribe si el valor es negativo.
    Credito(bool),
}

/// Una `PICTURE` de edicion ya analizada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plantilla {
    sim: Vec<Sim>,
    /// Posiciones que consumen digito.
    digitos: usize,
    /// Digitos a la derecha del punto decimal.
    pub escala: u32,
    /// Que caracter rellena la zona suprimida.
    relleno: char,
    /// Hay al menos una posicion flotante.
    hay_flotante: bool,
}

impl Plantilla {
    /// Ancho en caracteres del campo editado.
    pub fn ancho(&self) -> usize {
        self.sim
            .iter()
            .map(|s| if matches!(s, Sim::Credito(_)) { 2 } else { 1 })
            .sum()
    }

    /// Posiciones que consumen digito. Es el tamano del numero que cabe, y no
    /// coincide con el ancho: `$$$,$$9.99` mide 10 caracteres y guarda 7
    /// digitos.
    pub fn digitos(&self) -> usize {
        self.digitos
    }

    /// Esta PIC lleva algun simbolo de edicion? Si no, es una PIC de calculo
    /// y la debe analizar `pic::parse_pic`, no esto.
    pub fn es_editada(pic: &str) -> bool {
        let s = pic.to_uppercase();
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'Z' | b'*' | b'$' | b'+' | b'-' | b',' | b'.' | b'B' | b'/' => return true,
                b'C' if i + 1 < b.len() && b[i + 1] == b'R' => return true,
                b'D' if i + 1 < b.len() && b[i + 1] == b'B' => return true,
                // El `0` solo es insercion si NO esta dentro de un `(n)`.
                b'0' if !dentro_de_parentesis(b, i) => return true,
                _ => {}
            }
            i += 1;
        }
        false
    }

    pub fn parse(pic: &str) -> Result<Self, String> {
        let s = pic.trim().to_uppercase();
        let b = s.as_bytes();

        let mut crudo: Vec<char> = Vec::new();
        let mut i = 0usize;
        while i < b.len() {
            let c = b[i] as char;
            i += 1;
            // `CR` y `DB` son dos letras y una sola posicion logica.
            if (c == 'C' && i < b.len() && b[i] == b'R') || (c == 'D' && i < b.len() && b[i] == b'B')
            {
                i += 1;
                crudo.push(if c == 'C' { '\u{1}' } else { '\u{2}' });
                continue;
            }
            let mut cuenta = 1usize;
            if i < b.len() && b[i] == b'(' {
                let ini = i + 1;
                let mut j = ini;
                while j < b.len() && b[j] != b')' {
                    j += 1;
                }
                if j >= b.len() {
                    return Err(format!("PIC '{pic}': falta ')'"));
                }
                cuenta = s[ini..j]
                    .trim()
                    .parse()
                    .map_err(|_| format!("PIC '{pic}': repeticion invalida"))?;
                i = j + 1;
            }
            for _ in 0..cuenta {
                crudo.push(c);
            }
        }

        // Segunda pasada: los repetidos de `$`, `+` y `-` son FLOTANTES; uno
        // solo es fijo. La primera posicion de un grupo flotante no consume
        // digito -- es el sitio del simbolo.
        let mut sim: Vec<Sim> = Vec::new();
        let mut escala = 0u32;
        let mut tras_punto = false;
        let mut relleno = ' ';
        let mut hay_flotante = false;

        let mut k = 0usize;
        while k < crudo.len() {
            let c = crudo[k];
            match c {
                '9' => {
                    sim.push(Sim::Digito);
                    if tras_punto {
                        escala += 1;
                    }
                    k += 1;
                }
                'Z' => {
                    sim.push(Sim::CeroEspacio);
                    if tras_punto {
                        escala += 1;
                    }
                    k += 1;
                }
                '*' => {
                    relleno = '*';
                    sim.push(Sim::CeroAsterisco);
                    if tras_punto {
                        escala += 1;
                    }
                    k += 1;
                }
                '$' | '+' | '-' => {
                    // Cuantos seguidos? (los separadores no cortan el grupo)
                    let mut n = 0usize;
                    let mut j = k;
                    while j < crudo.len() && (crudo[j] == c || es_insercion(crudo[j])) {
                        if crudo[j] == c {
                            n += 1;
                        }
                        j += 1;
                    }
                    if n <= 1 {
                        sim.push(Sim::Fijo(c));
                        k += 1;
                    } else {
                        hay_flotante = true;
                        // El PRIMERO no consume digito: es el hueco del
                        // simbolo. Los demas si, con supresion.
                        let mut primero = true;
                        while k < j {
                            let cc = crudo[k];
                            if cc == c {
                                if primero {
                                    primero = false;
                                    sim.push(Sim::Insercion('\u{0}')); // hueco
                                } else {
                                    sim.push(Sim::Flotante(c));
                                    if tras_punto {
                                        escala += 1;
                                    }
                                }
                            } else {
                                if cc == '.' {
                                    tras_punto = true;
                                }
                                sim.push(Sim::Insercion(cc));
                            }
                            k += 1;
                        }
                    }
                }
                '.' => {
                    tras_punto = true;
                    sim.push(Sim::Insercion('.'));
                    k += 1;
                }
                ',' | 'B' | '0' | '/' => {
                    sim.push(Sim::Insercion(if c == 'B' { ' ' } else { c }));
                    k += 1;
                }
                'V' => {
                    // Punto IMPLICITO: no ocupa caracter, solo marca la escala.
                    tras_punto = true;
                    k += 1;
                }
                'S' => {
                    // El signo de la PIC de calculo no se escribe.
                    k += 1;
                }
                '\u{1}' => {
                    sim.push(Sim::Credito(true));
                    k += 1;
                }
                '\u{2}' => {
                    sim.push(Sim::Credito(false));
                    k += 1;
                }
                otro => return Err(format!("PIC '{pic}': simbolo no soportado '{otro}'")),
            }
        }

        let digitos = sim
            .iter()
            .filter(|s| matches!(s, Sim::Digito | Sim::CeroEspacio | Sim::CeroAsterisco | Sim::Flotante(_)))
            .count();
        if digitos == 0 {
            return Err(format!("PIC '{pic}': no tiene posiciones de digito"));
        }

        Ok(Plantilla { sim, digitos, escala, relleno, hay_flotante })
    }

    /// Formatea `valor` (entero exacto) que viene con `escala_origen`
    /// decimales. Devuelve la cadena editada, del ancho de la plantilla.
    pub fn formatear(&self, valor: i128, escala_origen: u32) -> String {
        let negativo = valor < 0;
        let mut mag = valor.unsigned_abs();

        // Reescalar al numero de decimales que pide la plantilla. TRUNCA, que
        // es lo que hace COBOL sin `ROUNDED`.
        if self.escala > escala_origen {
            for _ in 0..(self.escala - escala_origen) {
                mag = mag.saturating_mul(10);
            }
        } else {
            for _ in 0..(escala_origen - self.escala) {
                mag /= 10;
            }
        }

        // Cadena de digitos, rellenada a la izquierda y recortada por arriba:
        // COBOL descarta los digitos de orden alto que no caben.
        let s = mag.to_string();
        let mut d: Vec<u8> = Vec::with_capacity(self.digitos);
        if s.len() >= self.digitos {
            d.extend_from_slice(&s.as_bytes()[s.len() - self.digitos..]);
        } else {
            d.resize(self.digitos - s.len(), b'0');
            d.extend_from_slice(s.as_bytes());
        }

        // Hay algun digito significativo antes del punto? Si no, la supresion
        // se lo come todo y el cero final es un campo en blanco -- que es lo
        // correcto en un listado: una linea vacia se distingue de un cero.
        let mut salida: Vec<char> = Vec::with_capacity(self.ancho());
        let mut suprimiendo = true;
        let mut idx = 0usize;
        // Donde iria el simbolo flotante: la ultima posicion suprimida.
        let mut hueco_flotante: Option<usize> = None;
        let mut simbolo_flotante = '$';

        for s in &self.sim {
            match *s {
                Sim::Digito => {
                    suprimiendo = false;
                    salida.push(d[idx] as char);
                    idx += 1;
                }
                Sim::CeroEspacio | Sim::CeroAsterisco | Sim::Flotante(_) => {
                    if let Sim::Flotante(c) = *s {
                        simbolo_flotante = c;
                    }
                    if suprimiendo && d[idx] == b'0' {
                        // Sigue suprimido. En una posicion flotante, este es el
                        // sitio candidato para el simbolo.
                        salida.push(if matches!(s, Sim::CeroAsterisco) { '*' } else { self.relleno });
                        if matches!(s, Sim::Flotante(_)) {
                            hueco_flotante = Some(salida.len() - 1);
                        }
                    } else {
                        suprimiendo = false;
                        salida.push(d[idx] as char);
                    }
                    idx += 1;
                }
                Sim::Insercion('\u{0}') => {
                    // El hueco del primer simbolo de un grupo flotante.
                    salida.push(self.relleno);
                    hueco_flotante = Some(salida.len() - 1);
                }
                Sim::Insercion('.') => {
                    // El punto SIEMPRE se escribe y corta la supresion: a
                    // partir de aqui los ceros son significativos (0.05).
                    suprimiendo = false;
                    salida.push('.');
                }
                Sim::Insercion(c) => {
                    // Un separador dentro de la zona suprimida se sustituye
                    // por el relleno: `Z,ZZ9` con 7 da "    7", no "   ,7".
                    salida.push(if suprimiendo { self.relleno } else { c });
                    // * Y ADEMAS es sitio candidato para el simbolo flotante.
                    //
                    // El estandar dice que los separadores que caen DENTRO del
                    // grupo flotante son parte del grupo, asi que el `$` puede
                    // aterrizar en la posicion de la coma. Sin esto,
                    // `$$$,$$9.99` con 105.00 daba `  $ 105.00` --el simbolo una
                    // casilla antes y un hueco en medio-- en vez de
                    // `   $105.00`. Se veia en el importe, no en el total: pasa
                    // solo cuando la supresion muere justo despues de la coma.
                    if suprimiendo {
                        hueco_flotante = Some(salida.len() - 1);
                    }
                }
                Sim::Fijo(c) => {
                    salida.push(match c {
                        '$' => '$',
                        '+' => {
                            if negativo {
                                '-'
                            } else {
                                '+'
                            }
                        }
                        _ => {
                            if negativo {
                                '-'
                            } else {
                                ' '
                            }
                        }
                    });
                }
                Sim::Credito(es_cr) => {
                    if negativo {
                        salida.push(if es_cr { 'C' } else { 'D' });
                        salida.push(if es_cr { 'R' } else { 'B' });
                    } else {
                        salida.push(' ');
                        salida.push(' ');
                    }
                }
            }
        }

        // El simbolo flotante va pegado al primer digito significativo.
        if self.hay_flotante {
            if let Some(p) = hueco_flotante {
                let c = match simbolo_flotante {
                    '$' => '$',
                    '+' => {
                        if negativo {
                            '-'
                        } else {
                            '+'
                        }
                    }
                    _ => {
                        if negativo {
                            '-'
                        } else {
                            ' '
                        }
                    }
                };
                salida[p] = c;
            }
        }

        salida.into_iter().collect()
    }
}

/// El simbolo flotante que toca escribir en el hueco, o `None` si la plantilla
/// no tiene ninguno. Se toma el ULTIMO, que es lo que hace `formatear` al
/// sobrescribir `simbolo_flotante` en cada posicion flotante que recorre.
fn simbolo_flotante(sim: &[Sim]) -> Option<char> {
    sim.iter()
        .rev()
        .find_map(|s| if let Sim::Flotante(c) = s { Some(*c) } else { None })
}

/// Los dos caracteres que puede escribir un simbolo de signo: `(positivo,
/// negativo)`. Es la unica regla de signo de COBOL y esta en un solo sitio
/// para que `formatear` y el emisor no puedan discrepar.
fn par_de_signo(c: char) -> (u8, u8) {
    match c {
        '$' => (b'$', b'$'),
        '+' => (b'+', b'-'),
        _ => (b' ', b'-'),
    }
}

// -- El emisor -----------------------------------------------------------
//
// Hasta aqui `formatear` es Rust que corre en el compilador: sirve para los
// tests y para un valor que se conozca al compilar. Un informe de banco no es
// eso. `MOVE SALDO TO LINEA-EXTRACTO` tiene que editar el numero que haya en
// `SALDO` cuando el programa CORRA, y el compilador no sabe cuanto vale
// despues de tres `ADD`.
//
// La salida de aqui no es un interprete de plantillas: es el recorrido de ESTA
// plantilla convertido en instrucciones. La plantilla se consume en tiempo de
// compilacion y no queda ni un byte de ella en el `.bex` -- lo que queda es el
// codigo que hace exactamente lo que ella decia. Es la misma idea que
// `write_const`, que mete el texto como inmediatos en vez de en una seccion de
// datos.

/// Registros vivos durante el recorrido. Ninguno es argumento de la puerta de
/// consola y no hay `syscall` en medio, asi que sobreviven sin salvarse.
mod reg {
    use bmo_lower::x86;
    /// Puntero de escritura en el buffer de salida. Avanza.
    pub const SALIDA: u8 = x86::R8;
    /// Direccion del hueco del simbolo flotante, o 0 si no hay.
    pub const HUECO: u8 = x86::R9;
    /// Puntero de lectura de los digitos. Avanza.
    pub const DIGITOS: u8 = x86::R10;
    /// Seguimos en la zona de ceros suprimidos? 1 = si.
    pub const SUPRIMIENDO: u8 = x86::R11;
    /// Signo del valor: 0 = positivo, 1 = negativo.
    pub const NEGATIVO: u8 = x86::RSI;
}

impl Plantilla {
    /// Emite el codigo que edita `rax` segun esta plantilla y lo ESCRIBE por
    /// la consola.
    ///
    /// Contrato de entrada: `rax` trae el valor como entero con signo, ya en
    /// la escala de la plantilla (`self.escala`). Eso lo garantiza el
    /// almacenamiento: un dato con PIC editada guarda su escala como
    /// cualquier otro, asi que `MOVE` y la aritmetica ya dejan el entero
    /// correcto sin saber nada de edicion.
    ///
    /// Al terminar la pila queda como estaba. Ensucia todos los registros
    /// caller-saved; ninguno vale nada despues de un `DISPLAY`.
    pub fn emitir(&self, code: &mut Vec<u8>) -> Result<(), String> {
        use bmo_lower::x86::{self, RSP};

        let total = self.emitir_en_buffer(code)?;
        bmo_lower::console::write_buffer(code);
        x86::add_r64_imm8(code, RSP, total);
        Ok(())
    }

    /// Igual, pero deja la linea editada en un buffer de PILA en vez de
    /// imprimirla.
    ///
    /// - Salida: `r8` = primer caracter, `r9` = ancho declarado de la mascara.
    /// - Devuelve los bytes de pila que **el llamante DEBE devolver** con
    ///   `add rsp, n`.
    ///
    /// Es el mismo reparto que hizo `bmo_lower::fmt`: editar es una cosa y
    /// publicar es otra. Existe porque la linea de un extracto no siempre va a
    /// la consola -- `WRITE` la manda al disco por otra puerta, y sin esto
    /// habria que duplicar el recorrido de la plantilla para cambiar solo su
    /// ultimo paso.
    ///
    /// Ojo con el orden: esto ensucia `r10` (es `reg::DIGITOS`), que es
    /// justamente donde `archivo::escribir_buffer` quiere el handle. El handle
    /// se carga DESPUES de editar, nunca antes.
    pub fn emitir_en_buffer(&self, code: &mut Vec<u8>) -> Result<i8, String> {
        use bmo_lower::x86::{self, Jump, RAX, RCX, RDX, RSP};

        let ancho = self.ancho();
        let dig = self.digitos;
        // Dos zonas alineadas a 8: la linea editada y los digitos sueltos.
        let zona_salida = (ancho + 7) & !7;
        let total = zona_salida + ((dig + 7) & !7);
        // El hueco se abre con `sub rsp, imm8`. Pasado ese limite haria falta
        // la forma imm32 y todos los `lea` con disp32: se dice en vez de
        // emitir una plantilla que escribe fuera de su sitio.
        if total > 127 {
            return Err(format!(
                "PIC editada demasiado ancha ({ancho} caracteres, {dig} digitos): \
                 no cabe en el hueco de pila de un desplazamiento de 8 bits"
            ));
        }

        x86::sub_r64_imm8(code, RSP, total as i8);
        x86::lea_r64_rsp_disp8(code, reg::SALIDA, 0);
        // Los digitos se llenan de atras hacia adelante --dividir entre 10 da
        // el ultimo primero--, asi que el puntero empieza UNA posicion pasado
        // el final y retrocede antes de cada escritura. Al acabar queda justo
        // en el primero, que es donde el recorrido lo necesita.
        x86::lea_r64_rsp_disp8(code, reg::DIGITOS, (zona_salida + dig) as i8);

        // -- Signo y magnitud --
        //
        // El signo se aparta ANTES de trocear el numero. Dividir un negativo
        // entre 10 da restos negativos, y `resto + '0'` con resto -7 no es un
        // digito: es el byte 0x29.
        x86::zero_r32(code, reg::NEGATIVO);
        x86::test_r64_r64(code, RAX, RAX);
        let es_positivo = x86::emit_jump(code, Jump::IfNotSign);
        x86::mov_r32_imm32(code, reg::NEGATIVO, 1);
        x86::neg_r64(code, RAX);
        x86::patch_jump(code, es_positivo);

        // -- Los digitos --
        //
        // Desenrollado: la cuenta la sabe el compilador. Lo que sobre por
        // arriba se queda en `rax` y se ignora, que es justo lo que hace
        // COBOL -- un importe que no cabe en su PIC pierde las cifras altas.
        for _ in 0..dig {
            x86::zero_r32(code, RDX);
            x86::mov_r32_imm32(code, RCX, 10);
            x86::div_r64(code, RCX);
            x86::add_r64_imm8(code, RDX, b'0' as i8);
            x86::dec_r64(code, reg::DIGITOS);
            x86::mov_byte_at_reg_from_low(code, reg::DIGITOS, RDX);
        }

        // -- El recorrido --
        x86::mov_r32_imm32(code, reg::SUPRIMIENDO, 1);
        x86::zero_r32(code, reg::HUECO);
        for s in &self.sim {
            self.emitir_simbolo(code, *s);
        }

        // -- El remate del simbolo flotante --
        //
        // Va al final y no sobre la marcha: el sitio del `$` es la ULTIMA
        // posicion suprimida, y eso no se sabe hasta haber pasado por ella.
        if let Some(sim) = simbolo_flotante(&self.sim) {
            let (pos, neg) = par_de_signo(sim);
            x86::test_r64_r64(code, reg::HUECO, reg::HUECO);
            let sin_hueco = x86::emit_jump(code, Jump::IfZero);
            x86::mov_byte_at_reg_imm8(code, reg::HUECO, pos);
            x86::test_r64_r64(code, reg::NEGATIVO, reg::NEGATIVO);
            let era_positivo = x86::emit_jump(code, Jump::IfZero);
            x86::mov_byte_at_reg_imm8(code, reg::HUECO, neg);
            x86::patch_jump(code, era_positivo);
            x86::patch_jump(code, sin_hueco);
        }

        // -- El buffer, listo para publicar --
        //
        // Quien escribe quiere el puntero al PRINCIPIO, y el del recorrido
        // acabo al final. Se vuelve a calcular en vez de restarle el ancho:
        // el `lea` es la misma verdad que al empezar, y una resta seria una
        // segunda copia del ancho que alguien tendria que mantener a mano.
        x86::lea_r64_rsp_disp8(code, x86::R8, 0);
        x86::mov_r32_imm32(code, x86::R9, ancho as u32);
        Ok(total as i8)
    }

    /// Una posicion de la plantilla. Cada rama es la traduccion literal de su
    /// gemela en `formatear`; si una cambia, la otra miente.
    fn emitir_simbolo(&self, code: &mut Vec<u8>, s: Sim) {
        use bmo_lower::x86::{self, Jump, RDX};

        match s {
            // `9`: se escribe siempre, y corta la supresion.
            Sim::Digito => {
                x86::zero_r32(code, reg::SUPRIMIENDO);
                x86::movzx_r32_byte_at_reg(code, RDX, reg::DIGITOS);
                x86::inc_r64(code, reg::DIGITOS);
                x86::mov_byte_at_reg_from_low(code, reg::SALIDA, RDX);
                x86::inc_r64(code, reg::SALIDA);
            }
            // `Z`, `*` y las posiciones flotantes: digito CON supresion.
            Sim::CeroEspacio | Sim::CeroAsterisco | Sim::Flotante(_) => {
                let relleno = if matches!(s, Sim::CeroAsterisco) { b'*' } else { self.relleno as u8 };
                x86::movzx_r32_byte_at_reg(code, RDX, reg::DIGITOS);
                x86::inc_r64(code, reg::DIGITOS);
                // Sigue suprimido? Solo si veniamos suprimiendo Y es un cero.
                x86::test_r64_r64(code, reg::SUPRIMIENDO, reg::SUPRIMIENDO);
                let escribe_digito = x86::emit_jump(code, Jump::IfZero);
                x86::cmp_r64_imm8(code, RDX, b'0' as i8);
                let no_es_cero = x86::emit_jump(code, Jump::IfNotZero);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, relleno);
                if matches!(s, Sim::Flotante(_)) {
                    // Sitio candidato para el simbolo: el ultimo gana.
                    x86::mov_r64_r64(code, reg::HUECO, reg::SALIDA);
                }
                let hecho = x86::emit_jump(code, Jump::Always);
                x86::patch_jump(code, escribe_digito);
                x86::patch_jump(code, no_es_cero);
                x86::zero_r32(code, reg::SUPRIMIENDO);
                x86::mov_byte_at_reg_from_low(code, reg::SALIDA, RDX);
                x86::patch_jump(code, hecho);
                x86::inc_r64(code, reg::SALIDA);
            }
            // El hueco del primer simbolo de un grupo flotante: no consume
            // digito, solo reserva el sitio.
            Sim::Insercion('\u{0}') => {
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, self.relleno as u8);
                x86::mov_r64_r64(code, reg::HUECO, reg::SALIDA);
                x86::inc_r64(code, reg::SALIDA);
            }
            // El punto siempre se escribe y corta la supresion: a partir de
            // ahi los ceros son significativos (`0.05`).
            Sim::Insercion('.') => {
                x86::zero_r32(code, reg::SUPRIMIENDO);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, b'.');
                x86::inc_r64(code, reg::SALIDA);
            }
            // Un separador dentro de la zona suprimida se va con ella -- y su
            // posicion queda como candidata para el simbolo flotante, porque
            // los separadores de dentro del grupo son parte del grupo. Ver la
            // nota de su gemela en `formatear`.
            Sim::Insercion(c) => {
                x86::test_r64_r64(code, reg::SUPRIMIENDO, reg::SUPRIMIENDO);
                let normal = x86::emit_jump(code, Jump::IfZero);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, self.relleno as u8);
                x86::mov_r64_r64(code, reg::HUECO, reg::SALIDA);
                let hecho = x86::emit_jump(code, Jump::Always);
                x86::patch_jump(code, normal);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, c as u8);
                x86::patch_jump(code, hecho);
                x86::inc_r64(code, reg::SALIDA);
            }
            // Signo en posicion fija: solo mira el signo, nunca la supresion.
            Sim::Fijo(c) => {
                let (pos, neg) = par_de_signo(c);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, pos);
                x86::test_r64_r64(code, reg::NEGATIVO, reg::NEGATIVO);
                let era_positivo = x86::emit_jump(code, Jump::IfZero);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, neg);
                x86::patch_jump(code, era_positivo);
                x86::inc_r64(code, reg::SALIDA);
            }
            // `CR`/`DB`: dos caracteres, y en positivo son dos espacios para
            // que la columna del listado no se descuadre.
            Sim::Credito(es_cr) => {
                let (a, b) = if es_cr { (b'C', b'R') } else { (b'D', b'B') };
                x86::test_r64_r64(code, reg::NEGATIVO, reg::NEGATIVO);
                let era_positivo = x86::emit_jump(code, Jump::IfZero);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, a);
                x86::inc_r64(code, reg::SALIDA);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, b);
                let hecho = x86::emit_jump(code, Jump::Always);
                x86::patch_jump(code, era_positivo);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, b' ');
                x86::inc_r64(code, reg::SALIDA);
                x86::mov_byte_at_reg_imm8(code, reg::SALIDA, b' ');
                x86::patch_jump(code, hecho);
                x86::inc_r64(code, reg::SALIDA);
            }
        }
    }
}

fn es_insercion(c: char) -> bool {
    matches!(c, ',' | '.' | 'B' | '0' | '/')
}

/// El byte en `i` esta dentro de un `(n)`? Un `0` ahi es parte de una cuenta
/// de repeticion --`9(10)`-- y no un caracter de insercion.
fn dentro_de_parentesis(b: &[u8], i: usize) -> bool {
    let mut j = i;
    while j > 0 {
        j -= 1;
        match b[j] {
            b'(' => return true,
            b')' => return false,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las plantillas que se ejercitan enteras. Cubren cada simbolo que el
    /// emisor sabe emitir: `9`, `Z`, `*`, moneda flotante, signo flotante,
    /// signo fijo, `CR`, `DB`, punto, coma, barra y blanco.
    const BANCO: &[&str] = &[
        "$$$,$$9.99",
        "**,**9.99",
        "Z,ZZ9.99CR",
        "Z,ZZ9.99DB",
        "9(4).99DB",
        "ZZZ.99",
        "+999",
        "-999",
        "---9",
        "99/99/99",
        "99B99B99",
        "ZZ9.99",
        "9.9",
        "Z,ZZ9",
    ];

    /// Los valores que rompen cosas: el cero (campo en blanco o `0.00`?), el
    /// uno (supresion hasta el final), los negativos (signo y `CR`), y uno que
    /// no cabe (COBOL tira las cifras altas, no falla).
    const VALORES: &[i128] = &[
        0, 1, 5, 7, 45, 199, 1234, 12_000, 281_026, 1_234_567, 99_999_999_999,
        -1, -5, -45, -1234, -12_000, -1_234_567,
    ];

    /// Ejecuta el codigo emitido y devuelve lo que el kernel habria pintado.
    ///
    /// El valor entra por `rax` porque ese es el contrato de `emitir`: en un
    /// programa de verdad lo deja ahi `load_var`.
    fn editar_en_maquina(p: &Plantilla, valor: i128) -> String {
        use bmo_lower::emu::{run, Machine};
        let mut code = Vec::new();
        bmo_lower::x86::mov_r64_imm64(&mut code, bmo_lower::x86::RAX, valor as i64 as u64);
        p.emitir(&mut code).expect("la plantilla cabe");
        run(Machine::new(code), 500_000).console
    }

    /// * La prueba que hace real la edicion: lo que EJECUTA el x86 emitido
    /// tiene que ser caracter por caracter lo que devuelve `formatear`.
    ///
    /// `formatear` es Rust corriendo en el compilador y esta probado abajo
    /// contra casos escritos a mano. Esto ata el emisor a el, asi que los dos
    /// caminos no pueden separarse en silencio: si alguien toca una rama de
    /// `formatear` y se olvida de su gemela en `emitir_simbolo`, esto se cae.
    ///
    /// Y no compara bytes contra bytes escritos a mano --eso solo dice que el
    /// emisor no ha cambiado, no que este bien--: ejecuta.
    #[test]
    fn lo_emitido_da_lo_mismo_que_lo_calculado() {
        for pic in BANCO {
            let p = Plantilla::parse(pic).unwrap();
            for &v in VALORES {
                let esperado = p.formatear(v, p.escala);
                let obtenido = editar_en_maquina(&p, v);
                assert_eq!(
                    obtenido, esperado,
                    "PIC {pic} con valor {v}: el codigo emitido no coincide con el motor"
                );
            }
        }
    }

    /// El ancho es la promesa de un listado: si una fila mide un caracter de
    /// mas, la columna de al lado se descuadra hasta el final del informe.
    /// Aqui se comprueba sobre lo EJECUTADO, no sobre lo calculado.
    #[test]
    fn lo_emitido_mide_siempre_el_ancho_declarado() {
        for pic in BANCO {
            let p = Plantilla::parse(pic).unwrap();
            for &v in VALORES {
                assert_eq!(
                    editar_en_maquina(&p, v).chars().count(),
                    p.ancho(),
                    "PIC {pic} con valor {v}"
                );
            }
        }
    }

    /// Una plantilla que no cabe en el hueco de pila se RECHAZA con su
    /// motivo. Emitirla de todos modos daria un programa que escribe fuera de
    /// su buffer -- y eso no se ve hasta que corrompe otra cosa.
    #[test]
    fn una_plantilla_gigante_se_rechaza_en_vez_de_desbordar() {
        let p = Plantilla::parse("Z(70).99").unwrap();
        let mut code = Vec::new();
        assert!(p.emitir(&mut code).is_err());
    }

    /// El caso del recibo: moneda flotante, millares y centavos.
    #[test]
    fn moneda_flotante() {
        let p = Plantilla::parse("$$$,$$9.99").unwrap();
        assert_eq!(p.escala, 2);
        // La plantilla mide 10 caracteres y tiene 7 posiciones de digito: el
        // primer `$` es el hueco del simbolo, los otros tres cuentan.
        assert_eq!(p.ancho(), 10);
        // 1234567 centavos = 12.345,67 -- entran justos.
        assert_eq!(p.formatear(1_234_567, 2), "$12,345.67");
        // Uno pequeno: el `$` se pega al digito y los millares desaparecen.
        assert_eq!(p.formatear(45, 2), "     $0.45");
    }

    /// Supresion de ceros: los separadores de la zona suprimida se van con
    /// ellos. Sin esto saldria "   ,7" en vez de "    7".
    #[test]
    fn supresion_se_come_los_separadores() {
        let p = Plantilla::parse("Z,ZZ9").unwrap();
        assert_eq!(p.formatear(7, 0), "    7");
        assert_eq!(p.formatear(1234, 0), "1,234");
    }

    /// Proteccion de cheque: los huecos van con asterisco para que nadie
    /// escriba encima. Es la razon de que `*` exista.
    #[test]
    fn proteccion_de_cheque() {
        let p = Plantilla::parse("**,**9.99").unwrap();
        assert_eq!(p.formatear(45, 2), "*****0.45");
        assert_eq!(p.formatear(1_234_567, 2), "12,345.67");
    }

    /// `CR` solo aparece en numeros rojos; en positivo son dos espacios, para
    /// que la columna del listado no se descuadre.
    #[test]
    fn credito_solo_si_negativo() {
        let p = Plantilla::parse("Z,ZZ9.99CR").unwrap();
        assert_eq!(p.formatear(-12_000, 2), "  120.00CR");
        assert_eq!(p.formatear(12_000, 2), "  120.00  ");
        assert_eq!(p.ancho(), 10);
    }

    #[test]
    fn debito() {
        let p = Plantilla::parse("9(4).99DB").unwrap();
        assert_eq!(p.formatear(-150, 2), "0001.50DB");
        assert_eq!(p.formatear(150, 2), "0001.50  ");
    }

    /// El punto corta la supresion: 0.05 tiene que ensenar su cero.
    #[test]
    fn el_punto_corta_la_supresion() {
        let p = Plantilla::parse("ZZZ.99").unwrap();
        assert_eq!(p.formatear(5, 2), "   .05");
        assert_eq!(p.formatear(0, 2), "   .00");
    }

    /// Signo fijo: `+` escribe el signo siempre, `-` solo en negativo.
    #[test]
    fn signos_fijos() {
        let mas = Plantilla::parse("+999").unwrap();
        assert_eq!(mas.formatear(12, 0), "+012");
        assert_eq!(mas.formatear(-12, 0), "-012");
        let menos = Plantilla::parse("-999").unwrap();
        assert_eq!(menos.formatear(12, 0), " 012");
        assert_eq!(menos.formatear(-12, 0), "-012");
    }

    /// Signo flotante: se pega al primer digito significativo.
    #[test]
    fn signo_flotante() {
        let p = Plantilla::parse("---9").unwrap();
        assert_eq!(p.formatear(-7, 0), "  -7");
        assert_eq!(p.formatear(-1234, 0), "-234"); // trunca por arriba
        assert_eq!(p.formatear(7, 0), "   7");
    }

    /// COBOL TRUNCA sin `ROUNDED`. 1.99 en `9V9` es 1.9, no 2.0.
    #[test]
    fn trunca_no_redondea() {
        let p = Plantilla::parse("9.9").unwrap();
        assert_eq!(p.formatear(199, 2), "1.9");
    }

    /// Reescalar hacia arriba: el origen trae menos decimales que la mascara.
    #[test]
    fn reescala_hacia_arriba() {
        let p = Plantilla::parse("ZZ9.99").unwrap();
        assert_eq!(p.formatear(12, 0), " 12.00");
    }

    /// Insercion de blancos y barras: fechas y agrupaciones.
    #[test]
    fn blancos_y_barras() {
        let p = Plantilla::parse("99/99/99").unwrap();
        assert_eq!(p.formatear(281_026, 0), "28/10/26");
        let b = Plantilla::parse("99B99B99").unwrap();
        assert_eq!(b.formatear(281_026, 0), "28 10 26");
    }

    /// `es_editada` distingue una PIC de calculo de una de presentacion --
    /// y no se traga el `0` de `9(10)`, que es una cuenta, no una insercion.
    #[test]
    fn detecta_pic_editada() {
        assert!(Plantilla::es_editada("$$$,$$9.99"));
        assert!(Plantilla::es_editada("ZZ9"));
        assert!(Plantilla::es_editada("9(4).99"));
        assert!(!Plantilla::es_editada("9(5)V99"));
        assert!(!Plantilla::es_editada("S9(7)V99"));
        assert!(!Plantilla::es_editada("9(10)"));
        assert!(!Plantilla::es_editada("X(10)"));
    }

    /// El ancho declarado tiene que ser el ancho real: un listado se descuadra
    /// entero si una fila mide un caracter de mas.
    #[test]
    fn el_ancho_cuadra_siempre() {
        for pic in ["$$$,$$9.99", "**,**9.99", "Z,ZZ9.99CR", "+999", "99/99/99"] {
            let p = Plantilla::parse(pic).unwrap();
            assert_eq!(p.formatear(123_456, 2).chars().count(), p.ancho(), "{pic}");
            assert_eq!(p.formatear(-1, 2).chars().count(), p.ancho(), "{pic}");
            assert_eq!(p.formatear(0, 2).chars().count(), p.ancho(), "{pic}");
        }
    }
}
