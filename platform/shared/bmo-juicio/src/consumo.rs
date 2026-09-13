//! **EL CONSUMO, POR DIFERENCIA Y POR LECTOR** -- dos lecturas de contadores
//! que solo crecen, y lo que va entre ellas.
//!
//! ## Por que existe (2026-09-12)
//!
//! Hasta hoy el kernel contestaba *"milivatios desde la ultima consulta"* y
//! *"hercios desde la ultima consulta"*, y para eso guardaba **UNA** lectura
//! anterior para todo el sistema. Dos lectores --el panel del escritorio y
//! `pulso.inti`-- se robaban el intervalo: cada uno veia el rato desde la
//! pregunta del OTRO. El numero seguia siendo cierto y era de otra ventana.
//!
//! Era un parche con forma de dato. La pieza es la de siempre con contadores:
//!
//! ```text
//!    el kernel   da CONTADORES QUE SOLO CRECEN: energia en microjulios,
//!                MPERF y APERF
//!    cada lector guarda SU lectura anterior y resta
//! ```
//!
//! Y la resta vive aqui porque aqui se prueba. Es aritmetica sobre cinco
//! numeros; dentro de un `.bex` no corre un test.
//!
//! ## Las tres respuestas, y por que no son dos
//!
//! ```text
//!    Consumo    hay intervalo y cuadra
//!    Pronto     menos de 10 ms: el contador de energia se refresca a su ritmo
//!               y un cociente tan corto da picos que no pasaron. Se ESPERA,
//!               y la lectura anterior NO se tira
//!    Reinicio   algo bajo: un contador que solo sube no puede bajar. La
//!               lectura nueva pasa a ser la referencia, y no se inventa nada
//! ```

/// Lo que se lee de una vez. El TSC lo pone el lector (`rdtsc`, sin puerta).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Lectura {
    /// Microjulios del paquete desde el arranque. `INFO_CPU_UJ_PAQUETE`.
    pub uj_paquete: u64,
    /// Microjulios del nucleo que contesta. `INFO_CPU_UJ_NUCLEO`.
    pub uj_nucleo: u64,
    /// `INFO_CPU_MPERF`: sube al ritmo del reloj de referencia.
    pub mperf: u64,
    /// `INFO_CPU_APERF`: sube al ritmo real del nucleo.
    pub aperf: u64,
    /// El TSC en el momento de leer.
    pub tsc: u64,
}

/// Lo que paso entre dos lecturas. Un cero es "no se sabe", nunca "no gasta".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Consumo {
    pub mw_paquete: u64,
    pub mw_nucleo: u64,
    /// A que fue el nucleo MIENTRAS ESTABA DESPIERTO. `0` si la ventana de
    /// MPERF es demasiado corta para decir nada.
    pub hz_nucleo: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entre {
    Consumo(Consumo),
    Pronto,
    Reinicio,
}

/// Por debajo de esto el cociente de energia es ruido.
pub const MS_MINIMO: u64 = 10;

/// Por debajo de estos ciclos de MPERF, un evento del planificador mueve la
/// frecuencia un 50%. Se contesta 0 -- no se sabe -- en vez de un numero nervioso.
pub const VENTANA_MPERF: u64 = 100_000;

/// **Lo que paso entre `antes` y `ahora`.**
pub fn entre(antes: &Lectura, ahora: &Lectura, tsc_hz: u64) -> Entre {
    let bajo = ahora.uj_paquete < antes.uj_paquete
        || ahora.uj_nucleo < antes.uj_nucleo
        || ahora.mperf < antes.mperf
        || ahora.aperf < antes.aperf;
    if tsc_hz == 0 || ahora.tsc <= antes.tsc || bajo {
        return Entre::Reinicio;
    }
    // `u128`: dt por mil ya no cabe comodo en 64 bits tras unos dias de TSC.
    let ms = ((ahora.tsc - antes.tsc) as u128 * 1000 / tsc_hz as u128) as u64;
    if ms < MS_MINIMO {
        return Entre::Pronto;
    }
    // Microjulios por milisegundo son milivatios, sin convertir nada.
    let mw_paquete = (ahora.uj_paquete - antes.uj_paquete) / ms;
    let mw_nucleo = (ahora.uj_nucleo - antes.uj_nucleo) / ms;
    let dm = ahora.mperf - antes.mperf;
    let da = ahora.aperf - antes.aperf;
    let hz_nucleo = if dm < VENTANA_MPERF {
        0
    } else {
        (tsc_hz as u128 * da as u128 / dm as u128) as u64
    };
    Entre::Consumo(Consumo { mw_paquete, mw_nucleo, hz_nucleo })
}

/// **Un lector**: su lectura anterior y lo ultimo que salio.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Muestreo {
    antes: Option<Lectura>,
    /// Lo ultimo que se pudo calcular. `None` hasta tener dos lecturas.
    pub ultimo: Option<Consumo>,
}

impl Muestreo {
    pub const fn nuevo() -> Self {
        Muestreo { antes: None, ultimo: None }
    }

    pub fn muestra(&mut self, l: Lectura, tsc_hz: u64) {
        let Some(a) = self.antes else {
            self.antes = Some(l);
            return;
        };
        match entre(&a, &l, tsc_hz) {
            Entre::Consumo(c) => {
                self.ultimo = Some(c);
                self.antes = Some(l);
            }
            // ** Pronto NO mueve la referencia: si lo hiciera, un lector que
            // pregunta muy seguido no llegaria nunca a juntar diez milisegundos.
            Entre::Pronto => {}
            Entre::Reinicio => self.antes = Some(l),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const TSC: u64 = 3_700_000_000;

    fn l(uj: u64, m: u64, a: u64, tsc: u64) -> Lectura {
        Lectura { uj_paquete: uj, uj_nucleo: uj / 8, mperf: m, aperf: a, tsc }
    }

    /// Un cuarto de segundo del Ryzen del 13-09: 58 W y 4,52 GHz.
    #[test]
    fn un_cuarto_de_segundo_del_ryzen() {
        let antes = l(0, 0, 0, 1_000);
        let ahora = l(14_500_000, 925_000_000, 1_130_000_000, 1_000 + TSC / 4);
        match entre(&antes, &ahora, TSC) {
            Entre::Consumo(c) => {
                assert_eq!(c.mw_paquete, 58_000);
                assert_eq!(c.mw_nucleo, 7_250);
                assert_eq!(c.hz_nucleo, 4_520_000_000);
            }
            otra => panic!("{:?}", otra),
        }
    }

    /// ** DOS LECTORES NO SE ROBAN NADA: el parche que esto sustituye.
    #[test]
    fn dos_lectores_ven_cada_uno_su_intervalo() {
        let mut panel = Muestreo::nuevo();
        let mut sonda = Muestreo::nuevo();
        panel.muestra(l(0, 0, 0, 1_000), TSC);
        sonda.muestra(l(1_000_000, 0, 0, 1_000 + TSC / 20), TSC);
        // El panel vuelve a mirar: su ventana es un cuarto de segundo, aunque la
        // sonda haya leido en medio.
        panel.muestra(l(14_500_000, 925_000_000, 1_130_000_000, 1_000 + TSC / 4), TSC);
        assert_eq!(panel.ultimo.unwrap().mw_paquete, 58_000);
    }

    /// Menos de 10 ms: se espera, y la referencia NO se mueve.
    #[test]
    fn pronto_no_mueve_la_referencia() {
        let mut m = Muestreo::nuevo();
        m.muestra(l(0, 0, 0, 1_000), TSC);
        m.muestra(l(10, 0, 0, 1_000 + TSC / 1000), TSC);
        assert_eq!(m.ultimo, None);
        m.muestra(l(14_500_000, 0, 0, 1_000 + TSC / 4), TSC);
        assert_eq!(m.ultimo.unwrap().mw_paquete, 58_000, "la ventana cuenta desde la primera");
    }

    /// Un contador que baja es un reinicio, no una energia negativa.
    #[test]
    fn un_contador_que_baja_es_un_reinicio() {
        assert_eq!(entre(&l(500, 0, 0, 1), &l(100, 0, 0, TSC), TSC), Entre::Reinicio);
    }

    /// Sin MPERF (un CPU sin la pareja): la frecuencia es 0, "no se sabe".
    #[test]
    fn sin_mperf_la_frecuencia_es_no_se_sabe() {
        match entre(&l(0, 0, 0, 1), &l(1_000_000, 0, 0, 1 + TSC), TSC) {
            Entre::Consumo(c) => assert_eq!(c.hz_nucleo, 0),
            otra => panic!("{:?}", otra),
        }
    }

    /// Sin reloj de referencia no hay intervalo.
    #[test]
    fn sin_reloj_no_hay_consumo() {
        assert_eq!(entre(&l(0, 0, 0, 1), &l(1, 1, 1, 2), 0), Entre::Reinicio);
    }
}
