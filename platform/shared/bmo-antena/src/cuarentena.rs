//! **EL PRESTAMO Y SU CASTIGO** -- BMO-X presta, y si la antena se pasa de lista,
//! corta y la hace esperar hasta que se aclare.
//!
//! [carril]  AMARILLO  decide cuanto tiempo se le niega la puerta a otra maquina
//! [cuesta]  DATO      una cuenta mal llevada perdona a quien no debia o destierra
//!                     a la antena buena
//! [riesgo]  AJENO     las faltas las comete la antena; aqui solo se apuntan
//!
//! # Seccion 9 de `docs/plan/PLAN_CLOUD_LOCAL.md` (2026-09-14)
//!
//! Eddi: *"que mi BMO-X le preste su parte, pero si intenta pasarse de listo, mi
//! BMO-X le corta y se reinicia hasta aclarar"*.
//!
//! ```text
//!    falta 1   corta, y 1 s sin puerta          una conversacion LIMPIA borra
//!    falta 2   corta, y 2 s                     una falta: la memoria se enfria
//!    falta 3   corta, y 4 s
//!    falta 4   corta, y 8 s   (x2 cada vez, techo 5 min)
//!    falta 5   DESTERRADA: solo vuelve si el dueno lo dice en BMO-X
//! ```
//!
//! ** "Reiniciar" es la SESION, nunca la maquina. Si una falta pudiera reiniciar
//! BMO-X, la antena tendria un boton para apagarte el PC: bastaria con portarse
//! mal a proposito.
//!
//! ** Y el prestamo tiene CUPO: tiempo de CPU y bytes por trabajo, fijados por
//! BMO-X antes de empezar. Pasarse del cupo es una falta como otra cualquiera.
//!
//! [!] Esto es la LEY, pura y con banco. Quien la aplica (la app que habla con la
//! antena) espera a TCP (G5 de `docs/plan/PLAN_RED_TX.md`).

/// La espera tras la primera falta.
pub const ESPERA_BASE_MS: u64 = 1_000;
/// El techo de la espera: cinco minutos.
pub const ESPERA_MAX_MS: u64 = 5 * 60 * 1_000;
/// A la quinta, desterrada.
pub const FALTAS_MAX: u8 = 5;

/// **Que cuenta como pasarse de lista.** Una por motivo, para decirlo en CABINA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falta {
    /// Una linea que no es del protocolo, o en el momento equivocado.
    Protocolo,
    /// Mas bytes, o mas tiempo de CPU, que el cupo del prestamo.
    Cupo,
    /// Pidio un servicio que BMO-X no presta.
    ServicioAjeno,
    /// Pide demasiado deprisa: otro trabajo con uno en marcha.
    Ritmo,
    /// Un trabajo que llega sin firma, o con una que no cuadra.
    Firma,
}

impl Falta {
    pub fn texto(self) -> &'static str {
        match self {
            Falta::Protocolo => "hablo fuera del protocolo",
            Falta::Cupo => "se paso del cupo prestado",
            Falta::ServicioAjeno => "pidio algo que BMO-X no presta",
            Falta::Ritmo => "pidio otro trabajo con uno en marcha",
            Falta::Firma => "trajo un trabajo sin firma valida",
        }
    }
}

/// **Si la antena puede hablar ahora.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Puerta {
    Abierta,
    /// Cerrada todavia `ms` milisegundos.
    Esperar { ms: u64 },
    /// Solo el dueno la abre (`perdonar`).
    Desterrada,
}

/// **La memoria de BMO-X sobre UNA antena.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuarentena {
    faltas: u8,
    hasta_ms: u64,
}

impl Cuarentena {
    pub const fn nueva() -> Self {
        Self { faltas: 0, hasta_ms: 0 }
    }

    pub fn faltas(&self) -> u8 {
        self.faltas
    }

    /// Apunta una falta en `ahora_ms`: la conexion se corta ya (lo hace quien
    /// llama) y esto dice cuanto tiempo queda cerrada la puerta.
    pub fn falta(&mut self, ahora_ms: u64, _f: Falta) -> Puerta {
        if self.faltas >= FALTAS_MAX {
            return Puerta::Desterrada;
        }
        self.faltas += 1;
        if self.faltas >= FALTAS_MAX {
            return Puerta::Desterrada;
        }
        let espera = (ESPERA_BASE_MS << (self.faltas - 1)).min(ESPERA_MAX_MS);
        self.hasta_ms = ahora_ms.saturating_add(espera);
        Puerta::Esperar { ms: espera }
    }

    /// Si puede volver a saludar en `ahora_ms`.
    pub fn puerta(&self, ahora_ms: u64) -> Puerta {
        if self.faltas >= FALTAS_MAX {
            Puerta::Desterrada
        } else if ahora_ms < self.hasta_ms {
            Puerta::Esperar { ms: self.hasta_ms - ahora_ms }
        } else {
            Puerta::Abierta
        }
    }

    /// Una conversacion entera sin faltas: se borra UNA. Asi se "aclara": no de
    /// golpe, sino portandose bien. A una desterrada no le vale.
    pub fn limpia(&mut self) {
        if self.faltas < FALTAS_MAX {
            self.faltas = self.faltas.saturating_sub(1);
        }
    }

    /// El dueno, en BMO-X, la perdona. La unica salida del destierro.
    pub fn perdonar(&mut self) {
        *self = Self::nueva();
    }
}

/// **Lo que BMO-X presta a un trabajo**, fijado ANTES de empezar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cupo {
    pub cpu_ms_max: u64,
    pub bytes_max: u64,
    cpu_ms: u64,
    bytes: u64,
}

impl Cupo {
    pub const fn nuevo(cpu_ms_max: u64, bytes_max: u64) -> Self {
        Self { cpu_ms_max, bytes_max, cpu_ms: 0, bytes: 0 }
    }

    /// Cobra lo gastado. Pasarse, aunque sea por uno, es `Falta::Cupo`.
    pub fn cobrar(&mut self, cpu_ms: u64, bytes: u64) -> Result<(), Falta> {
        self.cpu_ms = self.cpu_ms.saturating_add(cpu_ms);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.cpu_ms > self.cpu_ms_max || self.bytes > self.bytes_max {
            Err(Falta::Cupo)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_espera_se_dobla_y_a_la_quinta_destierra() {
        let mut c = Cuarentena::nueva();
        assert_eq!(c.falta(0, Falta::Protocolo), Puerta::Esperar { ms: 1_000 });
        assert_eq!(c.falta(0, Falta::Cupo), Puerta::Esperar { ms: 2_000 });
        assert_eq!(c.falta(0, Falta::Ritmo), Puerta::Esperar { ms: 4_000 });
        assert_eq!(c.falta(0, Falta::Firma), Puerta::Esperar { ms: 8_000 });
        assert_eq!(c.falta(0, Falta::ServicioAjeno), Puerta::Desterrada);
        assert_eq!(c.puerta(u64::MAX), Puerta::Desterrada, "el tiempo no perdona el destierro");
        assert_eq!(c.falta(0, Falta::Protocolo), Puerta::Desterrada);
        assert_eq!(c.faltas(), FALTAS_MAX, "no se pasa del maximo");
    }

    #[test]
    fn la_puerta_se_abre_al_cumplir_la_espera() {
        let mut c = Cuarentena::nueva();
        c.falta(10_000, Falta::Protocolo);
        assert_eq!(c.puerta(10_400), Puerta::Esperar { ms: 600 });
        assert_eq!(c.puerta(11_000), Puerta::Abierta);
    }

    #[test]
    fn portarse_bien_aclara_de_una_en_una() {
        let mut c = Cuarentena::nueva();
        c.falta(0, Falta::Protocolo);
        c.falta(0, Falta::Protocolo);
        c.limpia();
        assert_eq!(c.faltas(), 1);
        c.limpia();
        c.limpia();
        assert_eq!(c.faltas(), 0, "no baja de cero");
    }

    #[test]
    fn a_la_desterrada_solo_la_perdona_el_dueno() {
        let mut c = Cuarentena::nueva();
        for _ in 0..FALTAS_MAX {
            c.falta(0, Falta::Cupo);
        }
        c.limpia();
        assert_eq!(c.puerta(u64::MAX), Puerta::Desterrada);
        c.perdonar();
        assert_eq!(c.puerta(0), Puerta::Abierta);
        assert_eq!(c.faltas(), 0);
    }

    #[test]
    fn el_techo_de_la_espera_no_se_desborda() {
        let mut c = Cuarentena { faltas: 3, hasta_ms: 0 };
        assert_eq!(c.falta(u64::MAX - 1, Falta::Ritmo), Puerta::Esperar { ms: 8_000 });
        assert_eq!(c.puerta(u64::MAX), Puerta::Abierta, "la suma se satura, no da la vuelta");
    }

    #[test]
    fn pasarse_del_cupo_por_uno_es_falta() {
        let mut p = Cupo::nuevo(100, 4096);
        assert_eq!(p.cobrar(60, 2048), Ok(()));
        assert_eq!(p.cobrar(40, 2048), Ok(()), "justo en el borde, vale");
        assert_eq!(p.cobrar(0, 1), Err(Falta::Cupo));
        let mut q = Cupo::nuevo(100, 4096);
        assert_eq!(q.cobrar(101, 0), Err(Falta::Cupo));
    }
}
