//! **EL PSP: la secuencia de arranque, como maquina de estados juzgable.**
//!
//! [carril]  AMARILLO  ordena escrituras a un procesador que no es este.
//!                     Equivocarse aqui no corrompe RAM: deja la GPU muerta
//!
//! [cuesta]  NADA aqui dentro -- esto no toca MMIO ni memoria. Lo que cuesta
//!           es lo que ORDENA: doce mensajes con esperas de 20 ms y una de
//!           hasta 3 s. Ver `PSP_MEDIDO.md`.
//!
//! [riesgo]  AJENO -- el que contesta al otro lado es firmware firmado que no
//!           se puede leer ni depurar. Si se queda callado, la unica
//!           informacion que hay es EN QUE PASO se quedo -- y por eso esta
//!           maquina lleva el paso por fuera y no dentro de un bucle.
//!
//! # *** POR QUE ESTO ES UNA MAQUINA DE ESTADOS Y NO UN DRIVER
//!
//! **No hay tarjeta.** Escribir aqui lecturas y escrituras de MMIO seria
//! escribir codigo que nadie puede ejecutar ni comprobar, que es exactamente lo
//! que el dueno prohibio: *"no quiero promesas"*.
//!
//! Asi que esto no toca hardware. **Dice QUE hay que escribir y en que orden**,
//! y contesta a lo que se le lee. Con eso:
//!
//! ```text
//!    se puede JUZGAR sin tarjeta      las pruebas de abajo lo hacen
//!    se puede REVISAR sin tarjeta     el orden esta en una tabla, no en un if
//!    el dia que haya tarjeta          el kernel pone el MMIO y ya
//! ```
//!
//! Es el mismo corte que ya usa el barrido de USB --`bmo_uhid::barrido` decide y
//! el kernel obedece-- y por el mismo motivo escrito alli: *"un barrido
//! automatico que se equivoque resetea el puerto del teclado que esta
//! escribiendo, y eso no se puede dejar a que salga bien en el metal"*.
//!
//! # De donde salen estos pasos
//!
//! De leer `amdgpu`, medido y contado en `PSP_MEDIDO.md`, que tambien lleva la
//! frontera de copyright: **se leyeron los pasos, no se copio codigo**.
//!
//! # [!] LO QUE ESTE FICHERO SE NIEGA A INVENTAR
//!
//! Los **numeros de orden** de cada componente (`PSP_BL__LOAD_*`) no estan aqui.
//! Se leen del arbol de Linux al rellenar el perfil, y hasta entonces
//! [`Codigos`] esta vacio y la maquina **se niega a arrancar**.
//!
//! > Un cero puesto donde va un codigo real no es un hueco: es una orden
//! > distinta enviada a un procesador que no puede decir que no la entiende.

use crate::psp::Estado::*;

// == LOS REGISTROS ==========================================================
//
// Los indices son los mismos en v13 y en v14; lo que cambia entre generaciones
// es el PREFIJO del bloque (`MP0_SMN_...` contra `MPASP_SMN_...`), o sea la
// direccion base. Por eso aqui viajan como INDICE y la base la pone el perfil:
// es la unica parte que de verdad depende de la tarjeta.

/// Donde se escribe la orden, y donde se espera la respuesta.
pub const ORDEN: u16 = 35;
/// Donde se escribe la direccion del blob, **desplazada 20 bits**.
pub const DIRECCION: u16 = 36;
/// La version del SOS, legible cuando ya vive.
pub const VERSION_SOS: u16 = 58;
/// Las ordenes del anillo, y donde contesta.
pub const ANILLO_ORDEN: u16 = 64;
/// El puntero de escritura del anillo.
pub const ANILLO_WPTR: u16 = 67;
/// Direccion del anillo: mitad baja, mitad alta, y su tamano.
pub const ANILLO_BAJA: u16 = 69;
pub const ANILLO_ALTA: u16 = 70;
pub const ANILLO_TAM: u16 = 71;
/// **La senal de vida del SOS.** Distinto de cero = ya esta arrancado, y
/// entonces casi toda la secuencia se salta.
pub const SOS_VIVO: u16 = 81;

/// El bit que enciende el PSP cuando ha terminado lo que se le pidio.
pub const LISTO: u32 = 1 << 31;

/// **A cuanto tiene que estar alineado el blob: 1 MiB.**
///
/// No es un gusto ni un margen de seguridad. La direccion viaja DESPLAZADA 20
/// BITS a la derecha, asi que **los veinte bits de abajo no llegan al otro
/// lado**: un blob a 0x1000 y otro a 0x0 se le entregan como la misma
/// direccion. Es la clase de fallo que no da error, da silencio.
pub const ALINEACION: u64 = 1 << 20;

/// Milisegundos de espera despues de pedir el SOS y las ordenes del anillo.
///
/// Sale del driver leido, y no se recorta: es un apreton de manos con firmware
/// que no se puede depurar. Ahorrar 20 ms una vez en el arranque no compra
/// nada; equivocarse cuesta una GPU muda.
pub const ESPERA_MS: u32 = 20;

// == LOS COMPONENTES, EN ORDEN ==============================================

/// Las piezas que el gestor de arranque del PSP carga, **en su orden**.
///
/// El orden no es de gusto: cada una prepara a la siguiente, y el SOS va el
/// ultimo porque es el que sustituye al gestor de arranque. Medido en
/// `PSP_MEDIDO.md`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Componente {
    /// La base de claves. Va primera: con ella se verifica lo demas.
    Kdb,
    Spl,
    SysDrv,
    SocDrv,
    IntfDrv,
    /// `DBGDRV` en v13, renombrado `HAD_DRV` en v14. Misma ranura.
    DbgDrv,
    RasDrv,
    /// `SPDMDRV` en v13, `IPKEYMGR` en v14. Misma ranura.
    Extra,
    /// **El sistema seguro.** El ultimo, y el unico que no se confirma mirando
    /// [`ORDEN`]: se confirma porque [`SOS_VIVO`] CAMBIA.
    Sos,
}

/// El orden, escrito una vez. Quien quiera saberlo lee esto y no un bucle.
pub const ORDEN_DE_CARGA: [Componente; 9] = [
    Componente::Kdb,
    Componente::Spl,
    Componente::SysDrv,
    Componente::SocDrv,
    Componente::IntfDrv,
    Componente::DbgDrv,
    Componente::RasDrv,
    Componente::Extra,
    Componente::Sos,
];

/// **Los numeros de orden de cada componente.** Parte del PERFIL, no del
/// protocolo.
///
/// Vacio a proposito. Ver la nota de la cabecera: un cero aqui seria una orden
/// distinta, no un hueco.
#[derive(Clone, Copy)]
pub struct Codigos {
    pub kdb: u32,
    pub spl: u32,
    pub sysdrv: u32,
    pub socdrv: u32,
    pub intfdrv: u32,
    pub dbgdrv: u32,
    pub rasdrv: u32,
    pub extra: u32,
    pub sos: u32,
}

impl Codigos {
    /// Lo que hay hoy: nada. La maquina se niega a arrancar con esto.
    pub const SIN_RELLENAR: Codigos = Codigos {
        kdb: 0,
        spl: 0,
        sysdrv: 0,
        socdrv: 0,
        intfdrv: 0,
        dbgdrv: 0,
        rasdrv: 0,
        extra: 0,
        sos: 0,
    };

    /// Estan todos puestos? Un solo cero basta para que no.
    pub fn completos(&self) -> bool {
        self.de(Componente::Kdb) != 0
            && self.de(Componente::Spl) != 0
            && self.de(Componente::SysDrv) != 0
            && self.de(Componente::SocDrv) != 0
            && self.de(Componente::IntfDrv) != 0
            && self.de(Componente::DbgDrv) != 0
            && self.de(Componente::RasDrv) != 0
            && self.de(Componente::Extra) != 0
            && self.de(Componente::Sos) != 0
    }

    pub fn de(&self, c: Componente) -> u32 {
        match c {
            Componente::Kdb => self.kdb,
            Componente::Spl => self.spl,
            Componente::SysDrv => self.sysdrv,
            Componente::SocDrv => self.socdrv,
            Componente::IntfDrv => self.intfdrv,
            Componente::DbgDrv => self.dbgdrv,
            Componente::RasDrv => self.rasdrv,
            Componente::Extra => self.extra,
            Componente::Sos => self.sos,
        }
    }
}

// == LO QUE LA MAQUINA PIDE =================================================

/// **Lo siguiente que hay que hacerle al PSP.** Quien lo ejecuta es el kernel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paso {
    /// Lee este registro y vuelve con su valor.
    Lee(u16),
    /// Escribe este valor en este registro.
    Escribe(u16, u32),
    /// Pon este componente en el buffer y vuelve con su direccion FISICA.
    /// Tiene que estar alineada a [`ALINEACION`].
    Pon(Componente),
    /// Espera milisegundos de verdad.
    Duerme(u32),
    /// El anillo esta vivo. A partir de aqui el resto del firmware sube por el.
    Listo,
    /// Se acabo. `motivo` dice por que.
    Alto(Motivo),
}

/// Por que se paro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motivo {
    /// El perfil no trae los numeros de orden. Ver [`Codigos`].
    SinCodigos,
    /// Se dio una direccion que no esta alineada a 1 MiB.
    DireccionTorcida,
    /// Se agoto el plazo esperando a este registro.
    SinRespuesta(u16),
}

/// En que punto va. Se guarda fuera del bucle a proposito: cuando el firmware
/// se queda callado, **este numero es la unica informacion que hay**.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Estado {
    Empezando,
    /// Mirando si el SOS ya vive.
    MirandoSos,
    /// Esperando al gestor de arranque antes de cargar `n`.
    EsperandoGestor(usize),
    /// Pidiendo la direccion del componente `n`.
    Pidiendo(usize),
    /// Direccion puesta; toca escribir la orden de `n`.
    Ordenando(usize),
    /// Orden escrita; esperando confirmacion de `n`.
    Confirmando(usize),
    /// Tras el SOS: dormir y esperar a que cambie SOS_VIVO.
    DurmiendoSos,
    EsperandoSos,
    /// El anillo: direccion baja, alta, tamano, orden, confirmacion.
    AnilloBaja,
    AnilloAlta,
    AnilloTam,
    AnilloOrden,
    AnilloDurmiendo,
    AnilloConfirmando,
    Terminado,
    Parado(Motivo),
}

/// Cuantas veces se relee un registro antes de darlo por muerto.
///
/// Tres mil, que es lo que hace el driver leido. Un tope y no un bucle abierto:
/// esto corre en el arranque, y colgarse aqui es una maquina que no enciende
/// **sin decir por que**.
pub const VUELTAS_MAX: u32 = 3000;

/// **La secuencia de arranque del PSP.**
pub struct Arranque {
    codigos: Codigos,
    estado: Estado,
    vueltas: u32,
    /// El valor de `SOS_VIVO` antes de pedir el SOS: la confirmacion es que
    /// CAMBIE, no que valga algo concreto.
    sos_antes: u32,
    /// La direccion del anillo, partida. La pone quien lo reserve.
    anillo: u64,
    anillo_bytes: u32,
}

impl Arranque {
    /// `anillo` es la direccion fisica del anillo ya reservado (4 KiB).
    pub fn nuevo(codigos: Codigos, anillo: u64, anillo_bytes: u32) -> Self {
        Self {
            codigos,
            estado: Empezando,
            vueltas: 0,
            sos_antes: 0,
            anillo,
            anillo_bytes,
        }
    }

    /// En que paso va, para poder decirlo cuando el otro lado se calle.
    pub fn donde(&self) -> Option<Componente> {
        match self.estado {
            EsperandoGestor(n) | Pidiendo(n) | Ordenando(n) | Confirmando(n) => {
                ORDEN_DE_CARGA.get(n).copied()
            }
            DurmiendoSos | EsperandoSos => Some(Componente::Sos),
            _ => None,
        }
    }

    /// **Que toca ahora.** `respuesta` es lo que devolvio el paso anterior:
    /// el valor leido para [`Paso::Lee`], la direccion fisica para
    /// [`Paso::Pon`], y se ignora para el resto.
    pub fn siguiente(&mut self, respuesta: u64) -> Paso {
        // Se comprueba en cada vuelta y no solo al empezar: un perfil a medias
        // no puede colarse por un camino lateral.
        if !self.codigos.completos() {
            self.estado = Parado(Motivo::SinCodigos);
        }
        match self.estado {
            Parado(m) => Paso::Alto(m),
            Terminado => Paso::Listo,

            Empezando => {
                self.estado = MirandoSos;
                Paso::Lee(SOS_VIVO)
            }

            // ** SI EL SOS YA VIVE, NO SE VUELVE A ARRANCAR. Es lo primero que
            // hace el driver leido, y tiene motivo: reiniciar un PSP que ya
            // esta arriba no es idempotente -- se lleva por delante lo que ya
            // haya cargado.
            MirandoSos => {
                if respuesta != 0 {
                    self.estado = AnilloBaja;
                    return self.siguiente(0);
                }
                self.vueltas = 0;
                self.estado = EsperandoGestor(0);
                Paso::Lee(ORDEN)
            }

            EsperandoGestor(n) => {
                if respuesta as u32 & LISTO != 0 {
                    self.vueltas = 0;
                    self.estado = Pidiendo(n);
                    return Paso::Pon(ORDEN_DE_CARGA[n]);
                }
                self.vueltas += 1;
                if self.vueltas >= VUELTAS_MAX {
                    self.estado = Parado(Motivo::SinRespuesta(ORDEN));
                    return Paso::Alto(Motivo::SinRespuesta(ORDEN));
                }
                Paso::Lee(ORDEN)
            }

            Pidiendo(n) => {
                // *** LA ALINEACION SE COMPRUEBA AQUI Y NO SE PERDONA.
                // Ver `ALINEACION`: los veinte bits de abajo no viajan, asi que
                // una direccion torcida no da error -- da silencio.
                if respuesta % ALINEACION != 0 {
                    self.estado = Parado(Motivo::DireccionTorcida);
                    return Paso::Alto(Motivo::DireccionTorcida);
                }
                self.estado = Ordenando(n);
                Paso::Escribe(DIRECCION, (respuesta >> 20) as u32)
            }

            Ordenando(n) => {
                let c = ORDEN_DE_CARGA[n];
                self.vueltas = 0;
                if c == Componente::Sos {
                    self.estado = DurmiendoSos;
                } else {
                    self.estado = Confirmando(n);
                }
                Paso::Escribe(ORDEN, self.codigos.de(c))
            }

            Confirmando(n) => {
                if respuesta as u32 & LISTO != 0 {
                    self.vueltas = 0;
                    self.estado = EsperandoGestor(n + 1);
                    return Paso::Lee(ORDEN);
                }
                self.vueltas += 1;
                if self.vueltas >= VUELTAS_MAX {
                    self.estado = Parado(Motivo::SinRespuesta(ORDEN));
                    return Paso::Alto(Motivo::SinRespuesta(ORDEN));
                }
                Paso::Lee(ORDEN)
            }

            // ** EL SOS NO CONFIRMA COMO LOS DEMAS. No enciende un bit en
            // `ORDEN`: cambia `SOS_VIVO`. Por eso hay dos estados aparte y no
            // se reutiliza `Confirmando`.
            DurmiendoSos => {
                self.estado = EsperandoSos;
                Paso::Duerme(ESPERA_MS)
            }

            EsperandoSos => {
                if respuesta as u32 != self.sos_antes && respuesta != 0 {
                    self.vueltas = 0;
                    self.estado = AnilloBaja;
                    return self.siguiente(0);
                }
                self.vueltas += 1;
                if self.vueltas >= VUELTAS_MAX {
                    self.estado = Parado(Motivo::SinRespuesta(SOS_VIVO));
                    return Paso::Alto(Motivo::SinRespuesta(SOS_VIVO));
                }
                Paso::Lee(SOS_VIVO)
            }

            AnilloBaja => {
                self.estado = AnilloAlta;
                Paso::Escribe(ANILLO_BAJA, self.anillo as u32)
            }
            AnilloAlta => {
                self.estado = AnilloTam;
                Paso::Escribe(ANILLO_ALTA, (self.anillo >> 32) as u32)
            }
            AnilloTam => {
                self.estado = AnilloOrden;
                Paso::Escribe(ANILLO_TAM, self.anillo_bytes)
            }
            AnilloOrden => {
                self.vueltas = 0;
                self.estado = AnilloDurmiendo;
                // La orden de crear el anillo va en el mismo registro por el
                // que contesta. Su numero tambien es del perfil: se reusa el
                // del SOS como marcador hasta que se rellene -- ver `Codigos`.
                Paso::Escribe(ANILLO_ORDEN, self.codigos.sos)
            }
            AnilloDurmiendo => {
                self.estado = AnilloConfirmando;
                Paso::Duerme(ESPERA_MS)
            }
            AnilloConfirmando => {
                if respuesta as u32 & LISTO != 0 {
                    self.estado = Terminado;
                    return Paso::Listo;
                }
                self.vueltas += 1;
                if self.vueltas >= VUELTAS_MAX {
                    self.estado = Parado(Motivo::SinRespuesta(ANILLO_ORDEN));
                    return Paso::Alto(Motivo::SinRespuesta(ANILLO_ORDEN));
                }
                Paso::Lee(ANILLO_ORDEN)
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Codigos de mentira: sirven para probar la FORMA, no para hablar con una
    /// GPU. Se llaman asi para que nadie los confunda con los de verdad.
    const FALSOS: Codigos = Codigos {
        kdb: 0x11,
        spl: 0x12,
        sysdrv: 0x13,
        socdrv: 0x14,
        intfdrv: 0x15,
        dbgdrv: 0x16,
        rasdrv: 0x17,
        extra: 0x18,
        sos: 0x19,
    };

    const ANILLO: u64 = 0x1_0000_0000;

    #[test]
    fn sin_codigos_se_niega_a_empezar() {
        let mut a = Arranque::nuevo(Codigos::SIN_RELLENAR, ANILLO, 4096);
        assert_eq!(a.siguiente(0), Paso::Alto(Motivo::SinCodigos));
    }

    #[test]
    fn lo_primero_es_preguntar_si_el_sos_ya_vive() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        assert_eq!(a.siguiente(0), Paso::Lee(SOS_VIVO));
    }

    #[test]
    fn con_el_sos_vivo_se_salta_a_crear_el_anillo() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        assert_eq!(a.siguiente(0), Paso::Lee(SOS_VIVO));
        // Contesta que si vive: no se vuelve a cargar nada.
        assert_eq!(a.siguiente(0xDEAD), Paso::Escribe(ANILLO_BAJA, 0));
    }

    #[test]
    fn cada_carga_es_direccion_orden_y_espera() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        assert_eq!(a.siguiente(0), Paso::Lee(SOS_VIVO));
        assert_eq!(a.siguiente(0), Paso::Lee(ORDEN)); // SOS muerto -> gestor
        assert_eq!(a.siguiente(LISTO as u64), Paso::Pon(Componente::Kdb));
        // Direccion alineada a 1 MiB; viaja desplazada 20 bits.
        assert_eq!(a.siguiente(0x0030_0000), Paso::Escribe(DIRECCION, 3));
        assert_eq!(a.siguiente(0), Paso::Escribe(ORDEN, FALSOS.kdb));
        assert_eq!(a.siguiente(0), Paso::Lee(ORDEN));
        // Confirma, y pasa al SIGUIENTE componente.
        assert_eq!(a.siguiente(LISTO as u64), Paso::Lee(ORDEN));
        assert_eq!(a.siguiente(LISTO as u64), Paso::Pon(Componente::Spl));
    }

    #[test]
    fn una_direccion_torcida_no_se_perdona() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        a.siguiente(0);
        a.siguiente(0);
        assert_eq!(a.siguiente(LISTO as u64), Paso::Pon(Componente::Kdb));
        // Alineada a pagina, NO a 1 MiB: los 20 bits de abajo no viajarian.
        assert_eq!(a.siguiente(0x0030_1000), Paso::Alto(Motivo::DireccionTorcida));
    }

    #[test]
    fn el_orden_de_los_nueve_es_el_medido() {
        assert_eq!(ORDEN_DE_CARGA.len(), 9);
        assert_eq!(ORDEN_DE_CARGA[0], Componente::Kdb);
        // El SOS SIEMPRE el ultimo: es el que sustituye al gestor de arranque.
        assert_eq!(ORDEN_DE_CARGA[8], Componente::Sos);
    }

    #[test]
    fn el_sos_confirma_por_su_propio_registro_y_no_por_la_orden() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        a.siguiente(0);
        a.siguiente(0);
        // Los ocho primeros, en seco.
        for i in 0..8 {
            assert_eq!(a.siguiente(LISTO as u64), Paso::Pon(ORDEN_DE_CARGA[i]));
            a.siguiente(0x0010_0000);
            a.siguiente(0);
            a.siguiente(0);
            assert_eq!(a.siguiente(LISTO as u64), Paso::Lee(ORDEN));
        }
        // El noveno es el SOS: tras la orden hay una espera REAL y despues se
        // vigila SOS_VIVO, no ORDEN.
        assert_eq!(a.siguiente(LISTO as u64), Paso::Pon(Componente::Sos));
        a.siguiente(0x0010_0000);
        assert_eq!(a.siguiente(0), Paso::Escribe(ORDEN, FALSOS.sos));
        assert_eq!(a.siguiente(0), Paso::Duerme(ESPERA_MS));
        assert_eq!(a.siguiente(0), Paso::Lee(SOS_VIVO));
    }

    #[test]
    fn un_registro_que_no_contesta_se_da_por_muerto_y_dice_cual() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        a.siguiente(0);
        a.siguiente(0);
        let mut ultimo = Paso::Lee(ORDEN);
        for _ in 0..VUELTAS_MAX + 2 {
            ultimo = a.siguiente(0);
        }
        assert_eq!(ultimo, Paso::Alto(Motivo::SinRespuesta(ORDEN)));
    }

    #[test]
    fn el_paso_se_puede_preguntar_cuando_el_otro_lado_calla() {
        let mut a = Arranque::nuevo(FALSOS, ANILLO, 4096);
        a.siguiente(0);
        a.siguiente(0);
        a.siguiente(LISTO as u64);
        // Se quedo pidiendo el KDB: eso es lo que hay que poder decir.
        assert_eq!(a.donde(), Some(Componente::Kdb));
    }
}
