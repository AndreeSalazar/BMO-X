//! **La edicion de COBOL en x86-64**: una `PICTURE` de edicion convertida en
//! instrucciones.
//!
//! [isa] x86-64. Leer la plantilla --y `formatear`, su gemela en Rust-- es del
//! frontend (`bmo_cobol_front::edicion`); esto la EMITE. Partido el 2026-09-18:
//! el codigo es el mismo, byte a byte, y ahora cuelga de un rasgo porque la
//! `Plantilla` es de otro crate. La regla del signo (`par_de_signo`) NO se copio:
//! sigue en un solo sitio, alli, para que `formatear` y el emisor no puedan
//! discrepar.

use bmo_cobol_front::edicion::{par_de_signo, simbolo_flotante, Plantilla, Sim};

/// Lo que el emisor sabe hacer con una [`Plantilla`].
pub trait EmitirPlantilla {
    /// Edita `rax` segun la plantilla y lo ESCRIBE por la consola.
    fn emitir(&self, code: &mut Vec<u8>) -> Result<(), String>;
    /// Edita `rax` en un hueco de la pila; ver la implementacion.
    fn emitir_en_buffer(&self, code: &mut Vec<u8>) -> Result<i8, String>;
    #[doc(hidden)]
    fn emitir_simbolo(&self, code: &mut Vec<u8>, s: Sim);
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

impl EmitirPlantilla for Plantilla {
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
    fn emitir(&self, code: &mut Vec<u8>) -> Result<(), String> {
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
    fn emitir_en_buffer(&self, code: &mut Vec<u8>) -> Result<i8, String> {
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
}
