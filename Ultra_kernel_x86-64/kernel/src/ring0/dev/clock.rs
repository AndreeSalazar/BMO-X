//! **El reloj de la placa**, leido del CMOS. Aqui se tocan los puertos y nada
//!
//! [carril]  VERDE     lee el CMOS y contesta la hora
//! mas: lo que significan los bytes lo decide `bmo-rtc`, que se prueba entero
//! en el anfitrion.
//!
//! ## El protocolo, que tiene una trampa de verdad
//!
//! El CMOS **se actualiza solo**, una vez por segundo, y mientras lo hace sus
//! registros no son coherentes: se puede leer el minuto viejo con el segundo
//! nuevo. El bit 7 del registro de estado A avisa (`UIP`, *update in progress*).
//!
//! Esperar a que `UIP` baje **no basta**, y esa es la trampa: la actualizacion
//! puede empezar justo entre dos de nuestras lecturas. El unico metodo que no
//! se equivoca es el clasico -- **leer dos veces seguidas y creer solo si las
//! dos dan lo mismo**. Es la misma idea que un contador de generacion, con las
//! herramientas de 1984.
//!
//! ## Se lee UNA vez y se guarda el desfase
//!
//! Un `in` al CMOS es lento --son puertos ISA, cientos de nanosegundos-- y la
//! hora no hace falta con precision de microsegundo. Se lee en el arranque, se
//! apunta contra los ticks del sistema, y a partir de ahi la hora **se calcula**
//! sumando el tiempo transcurrido. El CMOS no se vuelve a molestar.

use bmo_rtc::{Crudo, Fecha};

const PORT_INDEX: u16 = 0x70;
const PORT_DATA: u16 = 0x71;

const REG_SEGUNDO: u8 = 0x00;
const REG_MINUTO: u8 = 0x02;
const REG_HORA: u8 = 0x04;
const REG_DIA: u8 = 0x07;
const REG_MES: u8 = 0x08;
const REG_ANIO: u8 = 0x09;
/// No esta garantizado por ninguna norma: lo dice el FADT de ACPI, y casi
/// siempre es este. Si trae basura, `bmo-rtc` lo descarta y supone 20xx.
const REG_SIGLO: u8 = 0x32;
const REG_STATUS_A: u8 = 0x0A;
const REG_STATUS_B: u8 = 0x0B;

const UIP: u8 = 1 << 7;

/// La fecha del arranque, empaquetada, y el TSC que habia entonces.
/// `0` = no se pudo leer el reloj.
///
/// Se ancla al TSC y no a los ticks del temporizador porque su frecuencia
/// **esta medida** (`tsc_freq`), y la del temporizador no tiene constante con
/// nombre en ningun sitio. Un reloj que avanza a una velocidad supuesta es peor
/// que no tenerlo: se va derivando y nadie sabe cuanto.
static mut ARRANQUE: u64 = 0;
static mut TSC_BASE: u64 = 0;

unsafe fn read_reg(reg: u8) -> u8 {
    // El bit 7 del indice enmascara la NMI. Se deja como estaba --se escribe
    // solo el registro-- porque tocarlo aqui seria cambiar una politica del
    // sistema para leer la hora.
    core::arch::asm!("out dx, al", in("dx") PORT_INDEX, in("al") reg, options(nomem, nostack));
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") PORT_DATA, options(nomem, nostack));
    v
}

unsafe fn actualizandose() -> bool {
    read_reg(REG_STATUS_A) & UIP != 0
}

unsafe fn one_reading() -> Crudo {
    Crudo {
        segundo: read_reg(REG_SEGUNDO),
        minuto: read_reg(REG_MINUTO),
        hora: read_reg(REG_HORA),
        dia: read_reg(REG_DIA),
        mes: read_reg(REG_MES),
        anio: read_reg(REG_ANIO),
        siglo: read_reg(REG_SIGLO),
        estado_b: read_reg(REG_STATUS_B),
    }
}

/// Lee el CMOS de verdad. `None` si no da una fecha creible.
///
/// [!] Las dos lecturas seguidas no son por si acaso: son el metodo. Y el tope
/// de intentos tampoco -- un CMOS averiado que deje `UIP` puesto colgaria el
/// arranque, y una maquina que no arranca por no saber la hora es un intercambio
/// ridiculo.
fn read_now() -> Option<Fecha> {
    unsafe {
        for _ in 0..100 {
            let mut espera = 0u32;
            while actualizandose() && espera < 1_000_000 {
                espera += 1;
            }
            let a = one_reading();
            let b = one_reading();
            if a == b {
                return bmo_rtc::decodificar(a);
            }
        }
        None
    }
}

/// Lee el reloj una vez y lo ancla a los ticks. Se llama en el arranque.
///
/// Si no hay reloj creible se deja en cero y `ahora()` contesta cero: **no se
/// inventa una fecha**. Un log fechado en 1970 miente con mas convicion que uno
/// sin fechar.
pub fn init() {
    let Some(f) = read_now() else {
        crate::ring0::cabina::warn("reloj", "el CMOS no dio una fecha creible", 0);
        return;
    };
    unsafe {
        ARRANQUE = bmo_rtc::empaquetar(&f);
        TSC_BASE = crate::ring0::task::scheduler::rdtsc();
    }
    crate::ring0::cabina::info("reloj", "hora de la placa leida", f.anio as u64);
}

/// La fecha y hora de AHORA, empaquetada. `0` = no hay reloj.
///
/// Se calcula: la del arranque mas los segundos que han pasado segun el timer.
/// Volver a preguntarle al CMOS costaria ocho `in` a puertos ISA cada vez que
/// alguien pinta una ventana.
pub fn ahora() -> u64 {
    let (base, tsc0) = unsafe { (ARRANQUE, TSC_BASE) };
    if base == 0 {
        return 0;
    }
    let Some(f) = bmo_rtc::desempaquetar(base) else {
        return 0;
    };
    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        // Sin frecuencia medida no se puede convertir, y se contesta la hora
        // del arranque tal cual en vez de una inventada.
        return base;
    }
    let delta = crate::ring0::task::scheduler::rdtsc().saturating_sub(tsc0);
    bmo_rtc::empaquetar(&sumar_segundos(f, delta / hz))
}

/// Suma segundos a una fecha. Con calendario de verdad -- meses de distinto
/// largo y bisiestos incluidos.
///
/// ** Se escribe entero y no se aproxima: una maquina encendida dos dias con un
/// "todos los meses tienen 30" empieza a fechar los logs en un dia que no
/// existe, y eso se descubre cuando ya hay cien ficheros mal fechados.
fn sumar_segundos(mut f: Fecha, mut s: u64) -> Fecha {
    let bisiesto = |a: u16| (a % 4 == 0 && a % 100 != 0) || a % 400 == 0;
    let dias_mes = |a: u16, m: u8| -> u8 {
        match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => if bisiesto(a) { 29 } else { 28 },
            _ => 30,
        }
    };

    let resto = (s % 60) as u8;
    s /= 60;
    f.segundo += resto;
    if f.segundo >= 60 {
        f.segundo -= 60;
        s += 1;
    }
    let resto = (s % 60) as u8;
    s /= 60;
    f.minuto += resto;
    if f.minuto >= 60 {
        f.minuto -= 60;
        s += 1;
    }
    let resto = (s % 24) as u8;
    let mut dias = s / 24;
    f.hora += resto;
    if f.hora >= 24 {
        f.hora -= 24;
        dias += 1;
    }
    // Los dias, uno a uno. Un tope generoso porque esto solo corre mientras la
    // maquina esta encendida: nadie va a acumular mil anios de uptime, y si los
    // acumulara, pararse es mejor que girar.
    let mut vueltas = 0u32;
    while dias > 0 && vueltas < 400_000 {
        let en_mes = dias_mes(f.anio, f.mes) as u64;
        if (f.dia as u64) + dias <= en_mes {
            f.dia += dias as u8;
            dias = 0;
        } else {
            dias -= en_mes - f.dia as u64 + 1;
            f.dia = 1;
            f.mes += 1;
            if f.mes > 12 {
                f.mes = 1;
                f.anio += 1;
            }
        }
        vueltas += 1;
    }
    f
}
