//! **EL JUEZ DE LAS MEDIDAS** -- invariantes y veredictos, sin tocar el metal.
//!
//! # Por que existe, y por que NO esta escrito en el programa que mide
//!
//! El 2026-08-16 `c/coste.bex` imprimio `dispatch [SE PASA] 1116 > techo 320`
//! mientras otra funcion del MISMO programa daba 309 para la misma cantidad.
//! Costo **un flasheo entero** descubrirlo, y despues se dieron dos
//! explicaciones falsas antes de la buena:
//!
//!   1. *"es un fallo del compilador de C al anidar llamadas"* -- **falso**,
//!      tres sondas lo reproducen y las tres pasan.
//!   2. *"1116 > 895 es imposible, una parte no excede al todo"* -- **falso**:
//!      uno es una MEDIA y el otro un MINIMO, y una media inflada por
//!      expropiaciones supera un minimo sin problema.
//!
//! Lo que era: el metro cuenta **todas** las puertas entre las dos lecturas, y
//! `printf` cruza la puerta -- una escritura de consola dibuja glifos y hace
//! scroll, ~2,2 M ciclos. La ventana estaba contaminada por el propio informe.
//!
//! Ninguna de las tres cosas es un problema de LENGUAJE. Son reglas sobre
//! numeros. Y una regla sobre numeros **se prueba en un `cargo test` de tres
//! segundos**, no en una tanda de flasheo.
//!
//! Por eso este crate vive en `platform/shared/` y no dentro del `.bex`: es la
//! misma razon que ya dejo escrita `services/gui/Cargo.toml` sobre la politica
//! de foco -- *"alli se puede PROBAR; este binario es `no_main` para un target
//! sin sistema operativo y no corre un test"*.
//!
//! # Que hace un juez DURO
//!
//! No condenar mas. **Negarse a opinar cuando el instrumento se contradice.**
//!
//! ```text
//!    [SE PASA]    peor que el techo -> REGRESION
//!    [EN PLAZO]   dentro del techo, lejos de la meta -> DEUDA
//!    [META]       llego
//!    [ROTO]       el instrumento no se sostiene -> NO HAY VEREDICTO
//! ```
//!
//! El cuarto es el que faltaba. Un juez que siempre contesta algo es un juez
//! que algun dia contesta cualquier cosa.
//!
//! # [!] Lo que este fichero NO puede comprobar
//!
//! Que la ventana de medida estuviera limpia. Si quien mide imprime entre las
//! dos lecturas, los numeros que llegan aqui son **coherentes y falsos**: no
//! violan ningun invariante, simplemente describen otra cosa. Eso se arregla
//! donde se mide ([`Medida::cerrada_sin_imprimir`] lo pide por escrito), y se
//! dice aqui para que nadie lea este crate como una garantia que no da.

#![cfg_attr(not(test), no_std)]

/// Los dos numeros que el kernel declara por fila, tal y como viajan
/// empaquetados: `meta << 32 | techo`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Presupuesto {
    /// Ultima medida CONFIRMADA en metal. Cruzarlo es una regresion.
    pub techo: u64,
    /// A donde tiene que llegar. No alcanzarla es deuda, no fallo.
    pub meta: u64,
}

impl Presupuesto {
    /// Desempaqueta el `u64` que llega por `OP_INFO`.
    ///
    /// [!] Se desempaquetan LOS DOS de la misma palabra a proposito: separarlos
    /// en dos campos permitiria leer uno y no el otro, que es justo el error
    /// que hace decir *"cumple"* a algo que no llego a la meta.
    pub const fn desempaquetar(valor: u64) -> Self {
        Self { techo: valor & 0xFFFF_FFFF, meta: valor >> 32 }
    }

    /// `true` si este kernel no declara nada para esta fila -- un binario
    /// viejo, o una fila recien anadida.
    pub const fn sin_declarar(&self) -> bool {
        self.techo == 0
    }
}

/// Lo que una tanda de `coste` observo, en crudo y sin dividir.
///
/// Se toma en CRUDO y no ya dividido porque las divisiones son parte de lo que
/// hay que comprobar: un `ciclos / puertas` con `puertas == 0` es la forma mas
/// facil de imprimir un numero inventado.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Medida {
    /// Minimo de la fila, por operacion. Es la respuesta.
    pub min: u64,
    /// Media de la fila, por operacion. Es el segundo dato: la diferencia con
    /// el minimo ES la expropiacion.
    pub media: u64,
    /// Puertas que el kernel conto en la ventana.
    pub puertas: u64,
    /// Ciclos que el kernel acumulo DENTRO de `dispatch` en esa ventana.
    pub ciclos_dispatch: u64,
    /// **Testigo de que la ventana se cerro sin imprimir en medio.**
    ///
    /// No es un dato medido: es una AFIRMACION de quien mide. El juez no puede
    /// comprobarlo --los numeros de una ventana sucia son coherentes-- asi que
    /// lo unico honesto es obligar a declararlo y dejar el rastro. Quien ponga
    /// `false` aqui vera sus veredictos rechazados, que es lo correcto.
    pub cerrada_sin_imprimir: bool,
}

/// Por que un juez se niega a opinar. Cada variante es un fallo REAL que
/// ocurrio, no un caso hipotetico.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Roto {
    /// La ventana no se declaro limpia. El caso del 16-08.
    VentanaSucia,
    /// Cero puertas en la ventana: no hay nada que promediar.
    SinPuertas,
    /// El minimo salio mayor que la media. Imposible por definicion.
    MinimoSobreMedia,
    /// La medida es cero. Un cero no es una medida barata, es una medida que
    /// no ocurrio.
    MedidaEnCero,
    /// La media supera al minimo mas de lo que cabe explicar. No invalida el
    /// minimo --que es la respuesta-- pero dice que la maquina estaba haciendo
    /// otra cosa y que la MEDIA no se puede leer.
    MediaDisparada,
}

/// Lo que el juez contesta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veredicto {
    /// El kernel no declara presupuesto para esta fila. No es un fallo.
    SinDeclarar,
    /// Peor que el techo. Alguien metio trabajo en el camino.
    SePasa { medido: u64, techo: u64 },
    /// Dentro del techo, lejos de la meta. **En plazo no es estar bien.**
    EnPlazo { medido: u64, techo: u64, meta: u64, faltan: u64 },
    /// Llego a la meta.
    Meta { medido: u64, meta: u64 },
    /// El instrumento no se sostiene. **No hay veredicto**, y eso es la
    /// respuesta.
    Roto(Roto),
}

/// Cuantas veces puede la media pasar del minimo antes de dejar de creersela.
///
/// El 16-08 el Ryzen dio 2,55x con dos tareas listas, y eso era el planificador
/// compartiendo el quantum -- normal y explicado. A 20x ya no hay reparto de
/// quantum que lo justifique: es que la maquina estaba haciendo otra cosa.
///
/// [!] El numero es un juicio, no una medida, y por eso esta aqui con su
/// nombre y no escondido en una comparacion.
pub const MEDIA_MAXIMA_SOBRE_MINIMO: u64 = 20;

impl Medida {
    /// Los invariantes que se comprueban ANTES de mirar ningun presupuesto.
    ///
    /// Devuelve `None` si la medida se sostiene.
    pub fn revisar(&self) -> Option<Roto> {
        if !self.cerrada_sin_imprimir {
            return Some(Roto::VentanaSucia);
        }
        if self.puertas == 0 {
            return Some(Roto::SinPuertas);
        }
        if self.min == 0 {
            return Some(Roto::MedidaEnCero);
        }
        if self.min > self.media {
            return Some(Roto::MinimoSobreMedia);
        }
        // `checked_mul` y no `*`: con un minimo grande esto desborda, y un
        // desbordamiento que envuelve convertiria "disparada" en "normal".
        match self.min.checked_mul(MEDIA_MAXIMA_SOBRE_MINIMO) {
            Some(limite) if self.media > limite => Some(Roto::MediaDisparada),
            _ => None,
        }
    }

    /// Ciclos medios dentro de `dispatch`, o `None` si no se puede dividir.
    pub fn dispatch_medio(&self) -> Option<u64> {
        if self.puertas == 0 {
            return None;
        }
        Some(self.ciclos_dispatch / self.puertas)
    }
}

/// Juzga un valor ya medido contra lo que el kernel declara.
///
/// [!] `medida` es la tanda ENTERA y `valor` la cantidad concreta que se juzga,
/// que puede no ser `medida.min` -- el coste de resolver un handle es una
/// diferencia entre dos filas. Van separados porque atarlos obligaria a un juez
/// por cada forma de cantidad, y los invariantes son los mismos para todas.
pub fn juzgar(medida: &Medida, valor: u64, presupuesto: Presupuesto) -> Veredicto {
    if let Some(roto) = medida.revisar() {
        return Veredicto::Roto(roto);
    }
    // ** UN CERO NO ES UNA MEDIDA BARATA: ES UNA MEDIDA QUE NO OCURRIO.
    //
    // [`Medida::revisar`] ya lo dice del `min` de la tanda. Faltaba decirlo del
    // valor CONCRETO que se juzga, y el 2026-08-16 eso paso de ser teorico a ser
    // urgente: al sacar el metro de `dispatch` con un `cfg`, sus ciclos valen 0
    // -- y sin esta guarda el juez contestaba **`[META] 0`**, o sea *"llego al
    // objetivo"* para una fila que nadie ha medido.
    //
    // Es el cero silencioso de siempre, en el sitio donde mas dano hace: no
    // falla, FELICITA.
    if valor == 0 {
        return Veredicto::Roto(Roto::MedidaEnCero);
    }
    if presupuesto.sin_declarar() {
        return Veredicto::SinDeclarar;
    }
    if valor > presupuesto.techo {
        return Veredicto::SePasa { medido: valor, techo: presupuesto.techo };
    }
    if valor > presupuesto.meta {
        return Veredicto::EnPlazo {
            medido: valor,
            techo: presupuesto.techo,
            meta: presupuesto.meta,
            faltan: valor - presupuesto.meta,
        };
    }
    Veredicto::Meta { medido: valor, meta: presupuesto.meta }
}

/// **La comparacion que el 16-08 se hizo mal**: una MEDIA contra un MINIMO.
///
/// `dispatch` se lee como media sobre todas las puertas de la ventana y el
/// total como minimo del bloque menos molestado. Restar el uno del otro para
/// sacar "el stub" da un numero con un sesgo conocido, y **la direccion del
/// sesgo hay que decirla**: si la media viene inflada, el stub sale de MENOS.
///
/// Devuelve `None` cuando la resta no tiene sentido en vez de envolver a un
/// numero cercano a `u64::MAX`, que es lo que hace `-` con `u64`.
pub fn stub_desde(min_total: u64, dispatch_medio: u64) -> Option<u64> {
    min_total.checked_sub(dispatch_medio)
}

/// Las tres respuestas posibles al repartir una puerta entre sus dos mitades.
///
/// ** LA TERCERA ES LA QUE FALTABA, y la trajo el metal del 2026-08-17: con el
/// metro retirado `dispatch` vale 0, y `792 - 0 = 792` se imprimia como
/// *"reparto: dentro de dispatch 0, en el stub 792"*. Eso no es un reparto: es
/// una resta contra una medida que no ocurrio, con la forma exacta de un
/// hallazgo. Es el mismo cero silencioso que ya se tapo en [`juzgar`], en el
/// otro sitio donde asomaba.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reparto {
    /// Nadie midio `dispatch`. No hay dos mitades que repartir.
    NoMedido,
    /// La media de `dispatch` supera el minimo total. **Puede pasar sin que
    /// nada este roto** -- una media inflada por expropiaciones contra un
    /// minimo-- y por eso no es un fallo, es una respuesta.
    MediaSobreMinimo,
    /// El reparto se sostiene: esto es lo que queda fuera de `dispatch`.
    Stub(u64),
}

/// Reparte una puerta entre su mitad Rust (`dispatch`) y el resto (el stub).
///
/// Ver [`Reparto`] para por que hay tres respuestas y no un numero.
pub fn reparto(min_total: u64, dispatch_medio: u64) -> Reparto {
    if dispatch_medio == 0 {
        return Reparto::NoMedido;
    }
    match min_total.checked_sub(dispatch_medio) {
        Some(stub) => Reparto::Stub(stub),
        None => Reparto::MediaSobreMinimo,
    }
}

/// -- ** LOS DOS RELOJES DE LA MAQUINA -------------------------------------
///
/// `rdtsc` cuenta **ticks del TSC**, y el TSC es INVARIANTE: va a la frecuencia
/// base pase lo que pase con el boost. El nucleo, mientras, corre a otra cosa.
/// El Ryzen del 2026-08-17 lo dijo con sus dos instrumentos a la vez:
///
/// ```text
///    reloj base    3700 MHz   el TSC          <- lo que cuenta rdtsc
///    reloj ahora   4529 MHz   MPERF/APERF     <- a lo que va el nucleo
/// ```
///
/// O sea que **un tick son 1,22 ciclos**, y llamar "ciclos" a lo que mide
/// `rdtsc` es un error del 22% -- el patron 2 de la casa, el campo que viene en
/// otra unidad. Ver R-CENSO0 en `docs/CENSO_DE_EJES.md`.
///
/// [!] **Los presupuestos de `presupuesto.rs` estan en TICKS**, porque en ticks
/// es como se midieron. Esta conversion es para LEER, no para juzgar: convertir
/// antes de comparar contra el techo moveria el trinquete cada vez que el CPU
/// cambia de frecuencia, que es justo lo que un trinquete no puede hacer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reloj {
    /// Frecuencia del TSC. `INFO_TSC_HZ`.
    pub tsc_hz: u64,
    /// Frecuencia real del nucleo AHORA, medida por MPERF/APERF.
    /// `INFO_CPU_HZ_REAL`. Cero = esta maquina no sabe medirla.
    pub nucleo_hz: u64,
}

impl Reloj {
    /// Ticks -> ciclos de nucleo. `None` si falta un reloj.
    ///
    /// **`None` y no una estimacion**: sin `MPERF/APERF` no se sabe a que va el
    /// nucleo, y rellenar con la frecuencia base daria un numero que parece una
    /// medida y no lo es. Un cero aqui es exactamente el fallo que este crate
    /// existe para no repetir.
    pub fn ciclos(&self, ticks: u64) -> Option<u64> {
        if self.tsc_hz == 0 || self.nucleo_hz == 0 {
            return None;
        }
        // `u128` y no `u64`: 2,2 M ticks (una puerta de consola) por 4,5 GHz ya
        // son 10^16 -- cabe, pero por poco, y el dia que alguien mida un
        // segundo entero no cabria. Una multiplicacion que envuelve aqui daria
        // un numero pequeno y creible.
        let n = (ticks as u128) * (self.nucleo_hz as u128) / (self.tsc_hz as u128);
        Some(n as u64)
    }

    /// Ticks -> nanosegundos. Esto **no** necesita el reloj del nucleo: el
    /// tiempo lo da el TSC, que es para lo que sirve ser invariante.
    pub fn nanos(&self, ticks: u64) -> Option<u64> {
        if self.tsc_hz == 0 {
            return None;
        }
        Some(((ticks as u128) * 1_000_000_000u128 / (self.tsc_hz as u128)) as u64)
    }

    /// Cuantos ciclos hay en un tick, **en centesimas** (122 = 1,22 ciclos).
    ///
    /// En centesimas porque aqui no hay coma flotante y porque el ratio importa
    /// con dos cifras: entre 1,00 y 1,25 hay un 25% de diferencia en cada
    /// numero de este documento.
    pub fn centesimas(&self) -> Option<u64> {
        self.ciclos(100)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una tanda sana, con los numeros reales del Ryzen del 16-08.
    fn ryzen() -> Medida {
        Medida {
            min: 895,
            media: 2286,
            puertas: 65_543,
            ciclos_dispatch: 895 * 0 + 309 * 65_543,
            cerrada_sin_imprimir: true,
        }
    }

    #[test]
    fn la_tanda_buena_del_ryzen_se_sostiene() {
        assert_eq!(ryzen().revisar(), None);
        assert_eq!(ryzen().dispatch_medio(), Some(309));
    }

    // ===== EL FALLO DEL 16-08, CONVERTIDO EN PRUEBA =====

    /// **El caso que costo un flasheo.** `dispatch` salio 1116 porque la
    /// ventana incluia tres `printf`, y cada puerta de consola son ~2,2 M
    /// ciclos. Los numeros son coherentes: no hay invariante numerico que los
    /// pille. Lo unico que los pilla es que quien mide DECLARE la ventana.
    #[test]
    fn una_ventana_sucia_no_recibe_veredicto() {
        let sucia = Medida { cerrada_sin_imprimir: false, ..ryzen() };
        assert_eq!(sucia.revisar(), Some(Roto::VentanaSucia));
        assert_eq!(
            juzgar(&sucia, 1116, Presupuesto { techo: 320, meta: 190 }),
            Veredicto::Roto(Roto::VentanaSucia),
            "con la ventana sucia NO se opina, ni para condenar ni para absolver"
        );
    }

    /// **Y el razonamiento que tambien fue falso**: se dijo que 1116 > 895 era
    /// imposible *"porque una parte no excede al todo"*. No lo es: uno es media
    /// y el otro minimo. El juez no debe rechazarlo por esa razon.
    #[test]
    fn un_dispatch_medio_mayor_que_el_minimo_total_no_es_imposible() {
        let m = Medida { cerrada_sin_imprimir: true, ..ryzen() };
        // Con la ventana limpia, 1116 contra un techo de 320 es una REGRESION
        // de verdad -- no un imposible, y no un instrumento roto.
        assert_eq!(
            juzgar(&m, 1116, Presupuesto { techo: 320, meta: 190 }),
            Veredicto::SePasa { medido: 1116, techo: 320 }
        );
        // Y la resta contra el minimo no revienta: contesta que no se puede.
        assert_eq!(stub_desde(895, 1116), None);
    }

    // ===== EL RESTO DE INVARIANTES =====

    #[test]
    fn sin_puertas_no_hay_medida() {
        let m = Medida { puertas: 0, ..ryzen() };
        assert_eq!(m.revisar(), Some(Roto::SinPuertas));
        assert_eq!(m.dispatch_medio(), None, "no se divide entre cero");
    }

    #[test]
    fn un_cero_no_es_una_medida_barata() {
        let m = Medida { min: 0, ..ryzen() };
        assert_eq!(m.revisar(), Some(Roto::MedidaEnCero));
    }

    #[test]
    fn el_minimo_no_puede_superar_a_la_media() {
        let m = Medida { min: 3000, media: 2286, ..ryzen() };
        assert_eq!(m.revisar(), Some(Roto::MinimoSobreMedia));
    }

    #[test]
    fn la_expropiacion_normal_del_ryzen_no_dispara_la_alarma() {
        // 2286 / 895 = 2,55x -- dos tareas listas compartiendo quantum.
        assert_eq!(ryzen().revisar(), None);
    }

    #[test]
    fn una_media_veinte_veces_el_minimo_ya_no_se_lee() {
        let m = Medida { media: 895 * 21, ..ryzen() };
        assert_eq!(m.revisar(), Some(Roto::MediaDisparada));
    }

    /// El limite se comprueba con `checked_mul`: un minimo enorme no debe
    /// envolver y convertir "disparada" en "normal".
    #[test]
    fn un_minimo_gigante_no_envuelve_la_comprobacion_de_la_media() {
        let m = Medida {
            min: u64::MAX / 2,
            media: u64::MAX,
            puertas: 1,
            ciclos_dispatch: 1,
            cerrada_sin_imprimir: true,
        };
        assert_eq!(m.revisar(), None, "sin desbordar y sin alarma falsa");
    }

    // ===== LOS TRES VEREDICTOS =====

    #[test]
    fn los_tres_veredictos_del_ryzen_del_16_08() {
        let m = ryzen();
        // La puerta: 895 contra techo 895 y meta 400.
        assert_eq!(
            juzgar(&m, 895, Presupuesto { techo: 895, meta: 400 }),
            Veredicto::EnPlazo { medido: 895, techo: 895, meta: 400, faltan: 495 }
        );
        // El handle: 327 contra techo 327 y meta 80.
        assert_eq!(
            juzgar(&m, 327, Presupuesto { techo: 327, meta: 80 }),
            Veredicto::EnPlazo { medido: 327, techo: 327, meta: 80, faltan: 247 }
        );
        // Y una que llega.
        assert_eq!(
            juzgar(&m, 380, Presupuesto { techo: 895, meta: 400 }),
            Veredicto::Meta { medido: 380, meta: 400 }
        );
    }

    /// El techo es un TRINQUETE: igual al techo cumple, uno mas se pasa.
    #[test]
    fn el_techo_es_inclusivo_y_uno_mas_es_regresion() {
        let m = ryzen();
        let p = Presupuesto { techo: 895, meta: 400 };
        assert!(matches!(juzgar(&m, 895, p), Veredicto::EnPlazo { .. }));
        assert_eq!(juzgar(&m, 896, p), Veredicto::SePasa { medido: 896, techo: 895 });
    }

    /// Y llegar EXACTAMENTE a la meta es llegar.
    #[test]
    fn la_meta_es_inclusiva() {
        let m = ryzen();
        assert_eq!(
            juzgar(&m, 400, Presupuesto { techo: 895, meta: 400 }),
            Veredicto::Meta { medido: 400, meta: 400 }
        );
    }

    /// ** EL CERO QUE FELICITABA. Con el metro fuera del build (`cfg`
    /// `metro_puerta`), `dispatch` vale 0 -- y 0 esta por debajo de cualquier
    /// meta, asi que el juez contestaba `[META]`. Un fallo que no falla:
    /// **felicita**.
    #[test]
    fn un_cero_no_llega_a_la_meta_sino_que_rompe_el_juicio() {
        let m = ryzen();
        let p = Presupuesto { techo: 105, meta: 60 };
        assert_eq!(
            juzgar(&m, 0, p),
            Veredicto::Roto(Roto::MedidaEnCero),
            "un cero es una medida que no ocurrio, no una que salio barata"
        );
        // Y uno solo por encima si se juzga: la guarda es del cero, no de los
        // numeros pequenos -- que son justamente los que se persiguen.
        assert_eq!(juzgar(&m, 1, p), Veredicto::Meta { medido: 1, meta: 60 });
    }

    /// El cero manda tambien sobre "no hay presupuesto": primero se comprueba
    /// que haya medida, y solo despues si hay contra que juzgarla. Al reves, una
    /// fila sin declarar taparia que el metro no estaba puesto.
    #[test]
    fn el_cero_gana_a_la_fila_sin_declarar() {
        let m = ryzen();
        assert_eq!(
            juzgar(&m, 0, Presupuesto::desempaquetar(0)),
            Veredicto::Roto(Roto::MedidaEnCero)
        );
    }

    #[test]
    fn un_kernel_sin_presupuesto_se_calla_en_vez_de_inventarse_un_juicio() {
        let m = ryzen();
        assert_eq!(
            juzgar(&m, 895, Presupuesto::desempaquetar(0)),
            Veredicto::SinDeclarar
        );
    }

    /// ** El instrumento roto MANDA sobre el presupuesto: primero se comprueba
    /// que la medida se sostenga, y solo despues se juzga. Al reves, un
    /// instrumento roto podria imprimir [META] y darse por bueno.
    #[test]
    fn lo_roto_gana_al_presupuesto() {
        let sucia = Medida { cerrada_sin_imprimir: false, ..ryzen() };
        assert_eq!(
            juzgar(&sucia, 1, Presupuesto { techo: 895, meta: 400 }),
            Veredicto::Roto(Roto::VentanaSucia),
            "un 1 clavaria la META, y sin embargo no se opina"
        );
    }

    // ===== EL EMPAQUETADO, QUE CRUZA A RING 3 =====

    #[test]
    fn el_empaquetado_es_meta_arriba_y_techo_abajo() {
        let p = Presupuesto::desempaquetar((400u64 << 32) | 895);
        assert_eq!(p.techo, 895);
        assert_eq!(p.meta, 400);
    }

    #[test]
    fn un_techo_de_32_bits_completo_no_se_come_la_meta() {
        let p = Presupuesto::desempaquetar((7u64 << 32) | 0xFFFF_FFFF);
        assert_eq!(p.techo, 0xFFFF_FFFF);
        assert_eq!(p.meta, 7);
    }

    // ===== EL REPARTO, Y EL CERO QUE PARECIA UN HALLAZGO =====

    /// **El caso del 2026-08-17.** Con el metro retirado `dispatch` vale 0 y la
    /// resta daba el total entero, impreso como *"en el stub 792"*: una medida
    /// que nadie tomo, con la forma de la respuesta que se estaba buscando.
    #[test]
    fn sin_metro_no_hay_reparto_que_imprimir() {
        assert_eq!(reparto(792, 0), Reparto::NoMedido);
    }

    #[test]
    fn el_reparto_bueno_del_ryzen_se_parte() {
        assert_eq!(reparto(895, 309), Reparto::Stub(586));
    }

    /// Una media puede pasarse de un minimo sin que nada este roto. Se dice, no
    /// se envuelve a un numero cercano a `u64::MAX`.
    #[test]
    fn una_media_sobre_el_minimo_se_dice_por_su_nombre() {
        assert_eq!(reparto(895, 1116), Reparto::MediaSobreMinimo);
    }

    // ===== LOS DOS RELOJES =====

    /// Los numeros que trajo el Ryzen el 2026-08-17.
    fn ryzen_reloj() -> Reloj {
        Reloj { tsc_hz: 3_700_000_000, nucleo_hz: 4_529_000_000 }
    }

    #[test]
    fn una_puerta_en_ticks_no_es_una_puerta_en_ciclos() {
        let r = ryzen_reloj();
        // 792 ticks x 4529/3700 = 969,4 -> 969
        assert_eq!(r.ciclos(792), Some(969));
        assert_eq!(r.centesimas(), Some(122), "1 tick = 1,22 ciclos");
        // Y el tiempo sale del TSC, no del nucleo: 792 / 3,7 GHz = 214 ns
        assert_eq!(r.nanos(792), Some(214));
    }

    /// ** Sin `MPERF/APERF` no se contesta. Rellenar con la frecuencia base
    /// daria `ciclos == ticks`, o sea un numero que parece medido y dice que el
    /// nucleo va a la base -- que es justo lo que no se sabe.
    #[test]
    fn sin_reloj_de_nucleo_no_se_inventa_una_conversion() {
        let r = Reloj { tsc_hz: 3_700_000_000, nucleo_hz: 0 };
        assert_eq!(r.ciclos(792), None);
        // El tiempo SI se puede dar: para eso el TSC es invariante.
        assert_eq!(r.nanos(792), Some(214));
    }

    /// Una puerta de consola son ~2,2 M ticks. Por 4,5 GHz eso son 10^16: cabe
    /// en `u64` por poco, y en `u128` sin pensarlo. La prueba existe para que
    /// nadie lo devuelva a `u64` por parecer mas barato.
    #[test]
    fn lo_gordo_no_envuelve() {
        let r = ryzen_reloj();
        assert_eq!(r.ciclos(2_200_000), Some(2_692_918));
        // ** Y el caso extremo se comprueba por su PROPIEDAD, no contra una
        // constante escrita a mano: con 1,22 ciclos por tick el resultado tiene
        // que SUBIR. Una multiplicacion que envolviera daria un numero mas
        // pequeno que la entrada, y eso lo caza esta linea sin que nadie tenga
        // que fiarse de mi aritmetica -- que en la primera version de esta
        // prueba estaba mal por un 0,004%, y el `cargo test` lo dijo.
        let gordo = u64::MAX / 2;
        assert!(r.ciclos(gordo).unwrap() > gordo);
    }
}
