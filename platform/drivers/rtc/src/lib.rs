//! **El reloj de la placa** -- la parte que DECIDE, sin tocar un puerto.
//!
//! ## Por que hace falta
//!
//! Hasta hoy BMO-X no sabia que dia era. CABINA sella cada evento con `t3096C`
//! --un contador del arranque-- y eso sirve para ordenar entre si lo que paso
//! **en esta sesion**, y para nada mas. Dos arranques no se pueden comparar, un
//! log no se puede cruzar con nada de fuera, y un fichero guardado no puede
//! decir cuando se guardo.
//!
//! El dato existe y esta a dos puertos de distancia: el CMOS de la placa lleva
//! la hora con su pila desde antes de que arrancaramos.
//!
//! ## El reparto, que es el de siempre aqui
//!
//! Este crate **no toca hardware**: recibe los bytes crudos y decide que
//! significan. Quien los lee es `ring0/dev/reloj.rs`, con `in`/`out` sobre
//! `0x70`/`0x71`. La ventaja no es de estilo -- es que **la parte que se
//! equivoca se prueba en el anfitrion**: el BCD, las doce horas, el siglo y el
//! ano de dos digitos son cuatro trampas y ninguna necesita un CPU.
//!
//! ## Las cuatro trampas del CMOS, y son todas del mismo tipo
//!
//! El reloj no dice en que formato habla: **hay que preguntarselo al registro B
//! y creerle**. Y cada firmware lo deja como quiere.
//!
//! 1. **BCD o binario** (registro B, bit 2). En BCD, `0x59` son 59 segundos, no
//!    89. Es lo mas comun y es lo que hace la MSI de esta maquina.
//! 2. **12 o 24 horas** (registro B, bit 1). En 12 horas, el **bit 7 de la hora
//!    es PM** -- y hay que quitarlo ANTES de convertir el BCD, no despues.
//! 3. **Las 12 de la noche son la hora 12, no la 0.** `12 AM` -> 0 y `12 PM` ->
//!    12. Sin ese caso, a mediodia y a medianoche el reloj se va doce horas.
//! 4. **El siglo.** El registro de ano tiene DOS digitos. El registro de siglo
//!    (`0x32`) existe en casi todas las placas pero **no esta garantizado**, asi
//!    que si no dice nada creible se supone 20xx.

#![no_std]

/// Los bytes tal como salen del CMOS, sin interpretar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Crudo {
    pub segundo: u8,
    pub minuto: u8,
    pub hora: u8,
    pub dia: u8,
    pub mes: u8,
    /// Dos digitos.
    pub anio: u8,
    /// Registro `0x32`. `0` = no dijo nada.
    pub siglo: u8,
    /// Registro de estado B: bit 1 = 24 horas, bit 2 = binario.
    pub estado_b: u8,
}

/// Una fecha y hora ya entendida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fecha {
    pub anio: u16,
    pub mes: u8,
    pub dia: u8,
    pub hora: u8,
    pub minuto: u8,
    pub segundo: u8,
}

/// Bit 1 del registro B: el reloj habla en 24 horas.
pub const B_24H: u8 = 1 << 1;
/// Bit 2 del registro B: los valores son binarios y no BCD.
pub const B_BINARIO: u8 = 1 << 2;

fn de_bcd(v: u8) -> u8 {
    (v & 0x0F) + ((v >> 4) * 10)
}

/// Convierte los bytes del CMOS en una fecha, o `None` si no son creibles.
///
/// [!] **Se valida, y no es paranoia**: un CMOS sin pila devuelve `0xFF` en
/// todo, y un `mes = 165` recorrido como indice de una tabla de nombres es
/// justo la clase de dato de fuera que no se puede suponer. Aqui se contesta
/// que no y quien llama dice "sin reloj" -- que es cierto.
pub fn decodificar(c: Crudo) -> Option<Fecha> {
    let bcd = c.estado_b & B_BINARIO == 0;
    let h24 = c.estado_b & B_24H != 0;

    // * El bit de PM se quita ANTES de convertir. Con BCD, `0x92` es "12 PM":
    // convertirlo primero daria 92, y quitarle el bit despues, 28.
    let pm = !h24 && (c.hora & 0x80) != 0;
    let hora_cruda = c.hora & 0x7F;

    let conv = |v: u8| if bcd { de_bcd(v) } else { v };

    let segundo = conv(c.segundo);
    let minuto = conv(c.minuto);
    let mut hora = conv(hora_cruda);
    let dia = conv(c.dia);
    let mes = conv(c.mes);
    let anio2 = conv(c.anio) as u16;

    if !h24 {
        // Las 12 son el caso raro: `12 AM` es la hora 0 y `12 PM` es la 12.
        if hora == 12 {
            hora = 0;
        }
        if pm {
            hora += 12;
        }
    }

    let siglo = if c.siglo != 0 { conv(c.siglo) as u16 } else { 0 };
    let anio = if (19..=99).contains(&siglo) {
        siglo * 100 + anio2
    } else if anio2 >= 70 {
        1900 + anio2
    } else {
        2000 + anio2
    };

    let f = Fecha { anio, mes, dia, hora, minuto, segundo };
    if creible(&f) {
        Some(f)
    } else {
        None
    }
}

/// Es una fecha que puede existir? No comprueba el calendario entero --no hace
/// falta saber si 2026 fue bisiesto para descartar `0xFF`-- pero si los rangos.
pub fn creible(f: &Fecha) -> bool {
    (1970..=2199).contains(&f.anio)
        && (1..=12).contains(&f.mes)
        && (1..=31).contains(&f.dia)
        && f.hora < 24
        && f.minuto < 60
        && f.segundo < 60
}

/// `AAAA-MM-DD HH:MM:SS` en un buffer, sin asignar. Devuelve cuantos bytes.
///
/// ISO y no `DD/MM`: es el unico orden que se ordena solo al mirarlo, y un log
/// se lee ordenando.
pub fn escribir(f: &Fecha, out: &mut [u8]) -> usize {
    if out.len() < 19 {
        return 0;
    }
    let d2 = |v: u8, o: &mut [u8]| {
        o[0] = b'0' + (v / 10) % 10;
        o[1] = b'0' + v % 10;
    };
    out[0] = b'0' + ((f.anio / 1000) % 10) as u8;
    out[1] = b'0' + ((f.anio / 100) % 10) as u8;
    out[2] = b'0' + ((f.anio / 10) % 10) as u8;
    out[3] = b'0' + (f.anio % 10) as u8;
    out[4] = b'-';
    d2(f.mes, &mut out[5..7]);
    out[7] = b'-';
    d2(f.dia, &mut out[8..10]);
    out[10] = b' ';
    d2(f.hora, &mut out[11..13]);
    out[13] = b':';
    d2(f.minuto, &mut out[14..16]);
    out[16] = b':';
    d2(f.segundo, &mut out[17..19]);
    19
}

/// Empaqueta la fecha en un `u64` para que quepa en **un** campo de `OP_INFO`.
///
/// ** Un solo campo y no seis: la puerta contesta un numero por llamada, y seis
/// llamadas para una fecha se pueden leer **a caballo de un cambio de minuto**
/// -- daria `10:59` con los segundos del `11:00`. Empaquetado, la fecha es
/// atomica por construccion y no hace falta ningun cerrojo.
pub fn empaquetar(f: &Fecha) -> u64 {
    ((f.anio as u64) << 40)
        | ((f.mes as u64) << 32)
        | ((f.dia as u64) << 24)
        | ((f.hora as u64) << 16)
        | ((f.minuto as u64) << 8)
        | (f.segundo as u64)
}

/// Lo contrario de [`empaquetar`]. `None` si el numero no describe una fecha --
/// **cero incluido**, que es lo que contesta el kernel cuando no hay reloj.
pub fn desempaquetar(v: u64) -> Option<Fecha> {
    let f = Fecha {
        anio: ((v >> 40) & 0xFFFF) as u16,
        mes: ((v >> 32) & 0xFF) as u8,
        dia: ((v >> 24) & 0xFF) as u8,
        hora: ((v >> 16) & 0xFF) as u8,
        minuto: ((v >> 8) & 0xFF) as u8,
        segundo: (v & 0xFF) as u8,
    };
    if creible(&f) {
        Some(f)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El caso de esta maquina: BCD y 24 horas, que es lo que deja casi todo
    /// firmware moderno.
    #[test]
    fn bcd_y_24_horas() {
        let c = Crudo {
            segundo: 0x45, minuto: 0x34, hora: 0x21,
            dia: 0x09, mes: 0x08, anio: 0x26, siglo: 0x20,
            estado_b: B_24H,
        };
        let f = decodificar(c).expect("es creible");
        assert_eq!(f, Fecha { anio: 2026, mes: 8, dia: 9, hora: 21, minuto: 34, segundo: 45 });
    }

    /// ** EL BIT DE PM SE QUITA ANTES DE CONVERTIR EL BCD. `0x92` es "12 PM":
    /// convertir primero daria 92, y quitar el bit despues, 28. Ninguno de los
    /// dos es una hora.
    #[test]
    fn el_bit_de_pm_se_quita_antes_del_bcd() {
        let c = Crudo {
            segundo: 0, minuto: 0, hora: 0x80 | 0x12, // 12 PM en BCD
            dia: 0x01, mes: 0x01, anio: 0x26, siglo: 0x20,
            estado_b: 0, // 12 horas, BCD
        };
        assert_eq!(decodificar(c).unwrap().hora, 12);
    }

    /// ** Y las 12 AM son la hora CERO. Sin este caso, medianoche y mediodia se
    /// van doce horas -- en direcciones contrarias, ademas.
    #[test]
    fn las_doce_de_la_noche_son_la_hora_cero() {
        let medianoche = Crudo {
            hora: 0x12, estado_b: 0, mes: 0x01, dia: 0x01, anio: 0x26, siglo: 0x20,
            ..Default::default()
        };
        assert_eq!(decodificar(medianoche).unwrap().hora, 0);

        let una_am = Crudo { hora: 0x01, ..medianoche };
        assert_eq!(decodificar(una_am).unwrap().hora, 1);

        let una_pm = Crudo { hora: 0x80 | 0x01, ..medianoche };
        assert_eq!(decodificar(una_pm).unwrap().hora, 13);
    }

    /// El modo binario existe y hay firmware que lo usa. La misma hora, otros
    /// bytes.
    #[test]
    fn el_modo_binario_no_pasa_por_el_bcd() {
        let c = Crudo {
            segundo: 45, minuto: 34, hora: 21,
            dia: 9, mes: 8, anio: 26, siglo: 20,
            estado_b: B_24H | B_BINARIO,
        };
        let f = decodificar(c).unwrap();
        assert_eq!((f.anio, f.hora, f.segundo), (2026, 21, 45));
    }

    /// Sin registro de siglo se supone 20xx -- salvo que el ano de dos digitos
    /// sea >= 70, que es la ventana que usa todo el mundo desde el efecto 2000.
    #[test]
    fn sin_siglo_se_supone_el_veintiuno() {
        let base = Crudo { estado_b: B_24H, dia: 0x01, mes: 0x01, ..Default::default() };
        assert_eq!(decodificar(Crudo { anio: 0x26, ..base }).unwrap().anio, 2026);
        assert_eq!(decodificar(Crudo { anio: 0x85, ..base }).unwrap().anio, 1985);
    }

    /// **** UN CMOS SIN PILA DEVUELVE `0xFF` EN TODO, y eso no es una fecha.
    ///
    /// Se contesta `None` y quien llama dice "sin reloj". Aceptarlo daria un
    /// `mes = 165`, y un mes de 165 recorrido como indice de una tabla de
    /// nombres es exactamente como un dato de fuera se convierte en una lectura
    /// fuera de rango.
    #[test]
    fn un_reloj_sin_pila_no_inventa_una_fecha() {
        let muerto = Crudo {
            segundo: 0xFF, minuto: 0xFF, hora: 0xFF,
            dia: 0xFF, mes: 0xFF, anio: 0xFF, siglo: 0xFF,
            estado_b: B_24H,
        };
        assert!(decodificar(muerto).is_none());
        let ceros = Crudo { estado_b: B_24H, ..Default::default() };
        assert!(decodificar(ceros).is_none(), "mes 0 y dia 0 no existen");
    }

    /// ** La fecha viaja EMPAQUETADA en un solo `u64` porque la puerta contesta
    /// un numero por llamada. Seis llamadas se pueden leer a caballo de un
    /// cambio de minuto y dar `10:59` con los segundos del `11:00`.
    #[test]
    fn ida_y_vuelta_empaquetada() {
        let f = Fecha { anio: 2026, mes: 8, dia: 9, hora: 21, minuto: 34, segundo: 45 };
        assert_eq!(desempaquetar(empaquetar(&f)), Some(f));
    }

    /// Y el cero --lo que contesta el kernel cuando no hay reloj-- no se
    /// desempaqueta como una fecha valida.
    #[test]
    fn el_cero_no_es_una_fecha() {
        assert_eq!(desempaquetar(0), None);
    }

    #[test]
    fn se_escribe_en_iso() {
        let f = Fecha { anio: 2026, mes: 8, dia: 9, hora: 21, minuto: 4, segundo: 5 };
        let mut b = [0u8; 24];
        let n = escribir(&f, &mut b);
        assert_eq!(&b[..n], b"2026-08-09 21:04:05");
    }

    /// Un buffer corto no escribe media fecha: contesta 0. Media marca de
    /// tiempo es peor que ninguna, porque parece una.
    #[test]
    fn un_buffer_corto_no_escribe_media_fecha() {
        let f = Fecha { anio: 2026, mes: 8, dia: 9, hora: 21, minuto: 4, segundo: 5 };
        let mut b = [0u8; 10];
        assert_eq!(escribir(&f, &mut b), 0);
        assert_eq!(b, [0u8; 10]);
    }
}
