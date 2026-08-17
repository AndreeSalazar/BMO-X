//! **EL PADRE** -- cada campo NOMBRA una palabra y la compone. No sabe que
//! tiene hermanos.
//!
//! [eje]     CORRECCION
//! [exige]   R-DISCO2 (unidad y sesgo), R-DISCO6 (el medio se pregunta),
//!           R-FW2 (lo declarado se comprueba contra lo que hace el aparato)
//!
//! # La simetria que declara este fichero (L6c)
//!
//! **Cada tipo de aqui es exactamente una palabra del IDENTIFY, su sesgo y su
//! guarda de validez.** Ninguno mira a otro; ninguno sabe cuantos hay. El campo
//! que aparezca se escribe igual y al lado, con las mismas tres piezas:
//!
//! ```text
//!    DE QUE PALABRA SALE   el numero, citado del estandar
//!    QUE SESGO LLEVA       -1, exponente, o ninguno
//!    COMO SE SABE QUE VALE  la guarda, si la palabra tiene una
//! ```
//!
//! Que falte uno es visible porque los que hay tienen los tres apartados.
//!
//! # ** Los dos sesgos, y por que esta casa ya los conoce
//!
//! `R-DISCO2` dice que *"todo campo de conteo del hardware se cita con su
//! unidad y su sesgo: -1, exponente, milisegundos"*. Esa regla se escribio
//! despues de pagar el `bInterval` del teclado USB, que es un EXPONENTE y se
//! escribio como si fuera un numero: **un teclado sondeado cada 35 minutos, y
//! Configure Endpoint devolvio EXITO**.
//!
//! Aqui hay dos campos de esa misma familia, y por eso el fichero existe:
//!
//!   - **Palabra 75**, cola: bits 4:0 son *"maximo MENOS UNO"*. Un disco con 32
//!     ranuras dice **31**. Leerlo crudo deja una cola de 31 en vez de 32 -- un
//!     error que no rompe nada y que nadie descubre nunca.
//!   - **Palabra 106**, geometria: bits 3:0 son un **exponente**. Un `2` no son
//!     dos sectores logicos por fisico: son **cuatro**.
//!
//! # ** Y la guarda que la spec repite: bit 15 a cero Y bit 14 a uno
//!
//! Las palabras 106 y 209 solo son validas si `bit15 == 0 && bit14 == 1`. Es el
//! idioma que ATA uso para anadir campos a una estructura que ya estaba
//! desplegada: un disco viejo deja la palabra a `0000h` o a `FFFFh`, y las dos
//! fallan la guarda. **Sin comprobarla, un disco de 2003 declara una geometria
//! inventada** -- y como es un numero pequeno y plausible, se cree.

use crate::abuelo::Identify;

// ---------------------------------------------------------------------------
// EL MEDIO -- palabra 217
// ---------------------------------------------------------------------------

/// Gira o no gira. **La pregunta que BMO-X no habia hecho nunca** (R-DISCO6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Medio {
    /// `0000h` -- el disco no lo dice. **No significa "es un HDD"**: significa
    /// que hay que averiguarlo de otra forma. Ver `NOTA HISTORICA` abajo.
    NoContesta,
    /// `0001h` -- medio no rotacional. Un SSD.
    NoRota,
    /// `0401h`..`FFFEh` -- las revoluciones por minuto.
    Rota { rpm: u16 },
    /// `0002h`..`0400h` y `FFFFh` -- reservados. El disco dice algo que la spec
    /// no define, y eso **no se interpreta**: se dice.
    Reservado { crudo: u16 },
}

/// # NOTA HISTORICA: por que existe la palabra, y por que Windows no se fio
///
/// Antes de 2008 todo el software asumia que el medio giraba: el ascensor del
/// planificador, la desfragmentacion y la lectura anticipada existen **por el
/// coste de mover un brazo**. Llegaron los SSD y no habia forma de preguntarlo,
/// asi que ATA8-ACS anadio esta palabra.
///
/// ** Pero Windows 7 no la creyo sola.** Excluia de la desfragmentacion los
/// discos que declaran rotacion `1` **y ademas** los que su prueba real de
/// lectura aleatoria (WinSAT) puntuaba como SSD, porque los SSD tempranos
/// contestaban `0000h` o valores falsos.
///
/// Eso es `R-FW2` de esta casa, escrita once anos despues y con otras palabras:
/// *"lo que el firmware declara se comprueba contra lo que el aparato hace. Si
/// no coinciden, gana el aparato."* Por eso `Medio` tiene `NoContesta` como
/// estado propio y no lo colapsa a "rotacional": **la ausencia de respuesta es
/// una respuesta distinta de las dos**, y el que decida arriba tiene que verla.
impl Medio {
    /// Palabra 217. Sin sesgo; con tabla de rangos.
    pub fn de(id: &Identify) -> Medio {
        match id.palabra(217) {
            0x0000 => Medio::NoContesta,
            0x0001 => Medio::NoRota,
            v @ 0x0401..=0xFFFE => Medio::Rota { rpm: v },
            v => Medio::Reservado { crudo: v },
        }
    }

    /// Se sabe, con certeza, que este medio NO paga busqueda de cabezal?
    ///
    /// **`NoContesta` contesta `false`**, y es deliberado: quien pregunte esto
    /// va a activar un camino, y activar un camino sobre una duda es lo que
    /// R-DISCO6 prohibe.
    pub fn es_estado_solido(self) -> bool {
        matches!(self, Medio::NoRota)
    }
}

// ---------------------------------------------------------------------------
// LA COLA -- palabras 75 y 76 bit 8
// ---------------------------------------------------------------------------

/// Cuantos comandos admite el disco a la vez.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cola {
    /// Profundidad real, **con el sesgo ya deshecho**. 1..=32.
    pub profundidad: u8,
    /// Palabra 76 bit 8. Sin esto, la profundidad no se puede usar.
    pub ncq: bool,
}

impl Cola {
    /// Palabra 75 bits 4:0 = **maximo MENOS UNO**. Palabra 76 bit 8 = NCQ.
    pub fn de(id: &Identify) -> Cola {
        let profundidad = (id.palabra(75) & 0x1F) as u8 + 1;
        Cola { profundidad, ncq: id.palabra(76) & (1 << 8) != 0 }
    }
}

// ---------------------------------------------------------------------------
// EL ENLACE -- palabras 76 (soportado) y 77 (negociado)
// ---------------------------------------------------------------------------

/// La velocidad del cable. **Son DOS preguntas y la spec les da dos palabras.**
///
/// La 76 dice lo que el disco *sabe hacer*; la 77 bits 3:1 dice **lo que
/// negocio de verdad**. Un disco Gen3 en un puerto Gen2 declara 3 y corre a 2,
/// asi que quedarse con la 76 da un techo que no existe. Es la misma forma que
/// `R-FW2`: lo declarado y lo real son campos distintos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enlace {
    /// Bit 0 = Gen1 (1,5 Gb/s), bit 1 = Gen2 (3,0), bit 2 = Gen3 (6,0).
    pub soportadas: u8,
    /// 1, 2 o 3. `0` = el disco no lo dice.
    pub negociada: u8,
}

impl Enlace {
    pub fn de(id: &Identify) -> Enlace {
        let w76 = id.palabra(76);
        // ** La 76 no es valida si vale 0000h o FFFFh: eso significa que el
        // aparato no habla SATA (o no contesta), no que no soporte nada.
        let soportadas = if w76 == 0 || w76 == 0xFFFF {
            0
        } else {
            ((w76 >> 1) & 0b111) as u8
        };
        Enlace { soportadas, negociada: ((id.palabra(77) >> 1) & 0b111) as u8 }
    }

    /// La generacion mas alta que el disco declara saber hacer. 0 si ninguna.
    pub fn mejor_soportada(self) -> u8 {
        match self.soportadas {
            s if s & 0b100 != 0 => 3,
            s if s & 0b010 != 0 => 2,
            s if s & 0b001 != 0 => 1,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// LA GEOMETRIA -- palabras 106 y 209
// ---------------------------------------------------------------------------

/// El tamano del sector fisico y donde cae el LBA 0 dentro de el.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Geometria {
    /// **Exponente**: hay `2^exponente` sectores logicos en uno fisico.
    /// 0 = un disco clasico de 512/512.
    pub exponente: u8,
    /// La palabra 106 paso su guarda.
    pub valida: bool,
    /// Palabra 209 bits 13:0: cuantos sectores logicos esta desplazado el LBA 0
    /// respecto al principio del primer sector fisico.
    pub desplazamiento: u16,
    /// La palabra 209 paso su guarda.
    pub desplazamiento_valido: bool,
}

/// # NOTA HISTORICA: de donde sale el desplazamiento, y a quien se llevo por
/// delante
///
/// Los fabricantes querian sectores fisicos de 4096 B --menos ECC, ~10% mas de
/// capacidad-- pero el mundo entero asumia 512. La solucion fue **emular 512
/// logicos sobre 4096 fisicos** (*512e*, "Advanced Format", ~2010).
///
/// ** El desastre vino de una herencia mas vieja: MS-DOS empezaba la primera
/// particion en el **LBA 63**, por la geometria CHS de los ochenta. 63 no es
/// multiplo de 8, asi que **cada escritura de 4 KB caia a caballo de dos
/// sectores fisicos** y el disco tenia que leer-modificar-escribir los dos. El
/// mismo disco, con el mismo software, iba a la mitad segun donde empezara su
/// particion.
///
/// La palabra 209 existe para poder detectarlo: dice cuanto esta desplazado el
/// LBA 0. Un disco asi contesta `0x4001` -- bit 14 puesto (la guarda) y
/// desplazamiento 1.
///
/// ** Y por que le importa a ESTRATOS: su log crece hacia adelante y en bloques
/// de 4096. Si el frente del log cae desalineado respecto al sector fisico,
/// **cada avance paga dos sectores en vez de uno**, para siempre y en silencio.
impl Geometria {
    pub fn de(id: &Identify) -> Geometria {
        let w106 = id.palabra(106);
        // La guarda que la spec repite: bit 15 a cero Y bit 14 a uno.
        let valida = (w106 & 0x8000) == 0 && (w106 & 0x4000) != 0;
        // Solo hay varios logicos por fisico si el bit 13 lo dice.
        let varios = valida && (w106 & (1 << 13)) != 0;
        let exponente = if varios { (w106 & 0x0F) as u8 } else { 0 };

        let w209 = id.palabra(209);
        // La 209 tiene su propia guarda, Y ademas solo significa algo si la 106
        // dijo que hay varios logicos por fisico.
        let d_valido = varios && (w209 & 0x8000) == 0 && (w209 & 0x4000) != 0;
        let desplazamiento = if d_valido { w209 & 0x3FFF } else { 0 };

        Geometria { exponente, valida, desplazamiento, desplazamiento_valido: d_valido }
    }

    /// Sectores logicos por sector fisico: `2^exponente`.
    pub fn logicos_por_fisico(self) -> u32 {
        1u32 << self.exponente
    }
}

// ---------------------------------------------------------------------------
// TRIM -- palabra 169 bit 0
// ---------------------------------------------------------------------------

/// Soporta el disco que le digan que un bloque ya no importa?
///
/// Es un solo bit y aun asi tiene tipo propio, porque la respuesta **negativa
/// tiene consecuencias que hay que decir en voz alta** (R-DISCO10): sin TRIM, el
/// recolector de ESTRATOS libera bloques para el sistema de ficheros y el disco
/// los sigue creyendo vivos y copiandolos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Trim {
    pub soportado: bool,
}

impl Trim {
    pub fn de(id: &Identify) -> Trim {
        Trim { soportado: id.palabra(169) & 1 != 0 }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sector(pares: &[(usize, u16)]) -> [u8; 512] {
        let mut s = [0u8; 512];
        for &(n, v) in pares {
            let b = v.to_le_bytes();
            s[n * 2] = b[0];
            s[n * 2 + 1] = b[1];
        }
        s
    }
    fn id(pares: &[(usize, u16)]) -> [u8; 512] {
        sector(pares)
    }

    // -- EL MEDIO -----------------------------------------------------------

    #[test]
    fn medio_los_cuatro_rangos_de_la_spec() {
        let casos: [(u16, Medio); 6] = [
            (0x0000, Medio::NoContesta),
            (0x0001, Medio::NoRota),
            (0x0002, Medio::Reservado { crudo: 0x0002 }),
            (0x0400, Medio::Reservado { crudo: 0x0400 }),
            (0x1C20, Medio::Rota { rpm: 7200 }),
            (0xFFFF, Medio::Reservado { crudo: 0xFFFF }),
        ];
        for (crudo, esperado) in casos {
            let s = id(&[(217, crudo)]);
            let i = crate::abuelo::Identify::nuevo(&s).unwrap();
            assert_eq!(Medio::de(&i), esperado, "palabra 217 = {crudo:#06x}");
        }
    }

    /// 7200 rpm es `1C20h` en el estandar. Si esto falla, la tabla esta mal
    /// leida.
    #[test]
    fn medio_7200_rpm_es_1c20() {
        let s = id(&[(217, 0x1C20)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert_eq!(Medio::de(&i), Medio::Rota { rpm: 7200 });
    }

    /// ** La que importa: "no contesta" NO es "es un HDD", y sobre todo no
    /// autoriza el camino de SSD.
    #[test]
    fn medio_no_contesta_no_autoriza_el_camino_de_ssd() {
        let s = id(&[(217, 0x0000)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert!(!Medio::de(&i).es_estado_solido());
        // Y un valor reservado tampoco.
        let s = id(&[(217, 0x0300)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert!(!Medio::de(&i).es_estado_solido());
    }

    // -- LA COLA: el sesgo de menos uno -------------------------------------

    /// La prueba que existe por el `bInterval` del teclado.
    #[test]
    fn cola_deshace_el_sesgo_de_menos_uno() {
        // 31 en la palabra son 32 ranuras.
        let s = id(&[(75, 31), (76, 1 << 8)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert_eq!(Cola::de(&i).profundidad, 32);
    }

    /// Y el otro extremo: 0 en la palabra es UNA ranura, no cero. Un disco
    /// siempre admite al menos un comando -- por eso el campo lleva sesgo.
    #[test]
    fn cola_cero_en_la_palabra_es_una_ranura() {
        let s = id(&[(75, 0)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert_eq!(Cola::de(&i).profundidad, 1);
    }

    #[test]
    fn cola_el_ncq_es_otra_pregunta_que_la_profundidad() {
        let s = id(&[(75, 31), (76, 0x0002)]); // Gen1, sin bit 8
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        let c = Cola::de(&i);
        assert_eq!(c.profundidad, 32);
        assert!(!c.ncq, "declara 32 ranuras y NO soporta NCQ: es posible");
    }

    // -- EL ENLACE: soportado vs negociado ----------------------------------

    #[test]
    fn enlace_soportado_y_negociado_son_campos_distintos() {
        // Soporta Gen1+2+3 (bits 1,2,3 de la 76) y negocio Gen2.
        let s = id(&[(76, 0b1110), (77, 2 << 1)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        let e = Enlace::de(&i);
        assert_eq!(e.mejor_soportada(), 3);
        assert_eq!(e.negociada, 2, "el cable va a Gen2 aunque el disco sepa Gen3");
    }

    #[test]
    fn enlace_ffff_no_es_un_disco_que_lo_soporte_todo() {
        let s = id(&[(76, 0xFFFF)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert_eq!(Enlace::de(&i).mejor_soportada(), 0);
    }

    // -- LA GEOMETRIA: la guarda y el exponente -----------------------------

    /// Un disco viejo deja la palabra a cero. Sin la guarda, se leeria como
    /// "512/512 valido" -- que casualmente acierta, y por eso el fallo
    /// sobrevive hasta que aparece un disco 4Kn.
    #[test]
    fn geometria_sin_guarda_no_es_valida() {
        for w in [0x0000u16, 0xFFFF] {
            let s = id(&[(106, w)]);
            let i = crate::abuelo::Identify::nuevo(&s).unwrap();
            assert!(!Geometria::de(&i).valida, "palabra 106 = {w:#06x}");
        }
    }

    #[test]
    fn geometria_el_exponente_no_es_una_cuenta() {
        // valida (bit14) + varios por fisico (bit13) + exponente 3
        let s = id(&[(106, 0x4000 | (1 << 13) | 3)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        let g = Geometria::de(&i);
        assert!(g.valida);
        assert_eq!(g.exponente, 3);
        assert_eq!(g.logicos_por_fisico(), 8, "2^3, no 3");
    }

    /// El caso de 512e con 4096 fisicos: exponente 3 y desplazamiento 1, que es
    /// el `0x4001` de la nota historica.
    #[test]
    fn geometria_el_desplazamiento_del_lba_63() {
        let s = id(&[(106, 0x4000 | (1 << 13) | 3), (209, 0x4001)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        let g = Geometria::de(&i);
        assert!(g.desplazamiento_valido);
        assert_eq!(g.desplazamiento, 1);
    }

    /// La 209 no significa nada si la 106 no dijo que hay varios por fisico.
    #[test]
    fn geometria_el_desplazamiento_depende_de_la_106() {
        let s = id(&[(106, 0x4000), (209, 0x4001)]); // valida pero sin bit 13
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert!(!Geometria::de(&i).desplazamiento_valido);
    }

    // -- TRIM ---------------------------------------------------------------

    #[test]
    fn trim_es_el_bit_0_de_la_169() {
        let s = id(&[(169, 1)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert!(Trim::de(&i).soportado);
        let s = id(&[(169, 0xFFFE)]);
        let i = crate::abuelo::Identify::nuevo(&s).unwrap();
        assert!(!Trim::de(&i).soportado, "los otros 15 bits no son TRIM");
    }
}
