//! **EL GUION** -- la animacion del arranque, como aritmetica del tiempo.
//!
//! ## Lo que se pidio, en sus palabras
//!
//! *"el gato en neon que se prende al arrancar... que se sienta que tiene
//! animacion y como la camara avanza... y luego el gato toma el control con sus
//! ojos, que todo se vuelve negro, y el terminal aparece natural con
//! animacion"*.
//!
//! Son cuatro actos y estan escritos abajo con sus milisegundos.
//!
//! ## ** LA DECISION QUE SOSTIENE TODO: aqui no se duerme
//!
//! Lo natural seria escribir la animacion como una secuencia: pinta, espera,
//! pinta, espera. Y entonces **la animacion solo se puede juzgar arrancando la
//! maquina**, que en esta casa significa un reinicio por cada ajuste de medio
//! segundo.
//!
//! Aqui es al reves: [`fotograma`] es una **funcion pura del tiempo**. Le das un
//! milisegundo y te dice exactamente que hay en pantalla en ese instante -- el
//! alfa del gato, el brillo de los ojos, cuanto ha avanzado la camara. Nadie
//! duerme, nadie pinta.
//!
//! Con eso el guion entero se prueba en el anfitrion: se pueden recorrer los
//! 2.400 milisegundos de uno en uno y comprobar que **nada da un salto**, que
//! cada acto entrega al siguiente, y que al final la pantalla esta negra del
//! todo. Ninguna de esas tres cosas se ve bien a ojo, y las tres se rompen solas
//! en cuanto alguien mueve una duracion.
//!
//! El kernel se queda con lo suyo: mira el reloj, pregunta, pinta.

/// Cuanto dura la animacion entera, en milisegundos.
///
/// [!] **Esto es TIEMPO DE ARRANQUE, todo el.** El kernel esta operativo a los
/// 47 ms; lo que se ponga aqui es lo que la maquina tarda de mas en darte el
/// terminal. Estaba en 1.600 con el logo quieto y sube a 2.400 con la animacion
/// entera.
///
/// Va en una constante y con el precio escrito para que subirlo sea una decision
/// y no un descuido -- que es la misma nota que ya llevaba `GATO_MS`.
pub const DURACION_MS: u32 = 2400;

// Los cuatro actos, por su instante de FINAL. Se escriben acumulados y no por
// duracion porque lo que hay que poder leer de un vistazo es **cuando pasa
// cada cosa**, no cuanto duran los trozos.
pub const FIN_CIUDAD: u32 = 700;
pub const FIN_GATO: u32 = 1500;
pub const FIN_OJOS: u32 = 2000;
// El cuarto acaba en DURACION_MS.

// ?????? LA ESPERA ???????????????????????????????????????????????????????????
//
// ** EL GUION NO SE ACABA: ESPERA. Y esto sale de un video, como todo lo demas.
//
// El del 2026-08-15 enseno el arranque real y la animacion no estaba
// "funcionando regular": estaba **congelada**. Tres segundos y medio de ciudad
// quieta, el gato sin salir ni una vez, y de golpe el destello y el log.
//
// Dos causas, y las dos son de reloj:
//
//   1. El arranque llega al 100% ANTES del milisegundo 700. El kernel esta
//      operativo a los 47 ms, asi que cuando `intro_paso(100)` ocurre el guion
//      todavia esta en el primer acto -- el gato se enciende del 700 al 1500 y
//      ese tramo no llegaba a tocarse nunca.
//   2. Y al reves: entre `intro_paso(40)` y `intro_paso(70)` esta el bloque
//      entero de USB, xHCI, AHCI, red y ficheros. Segundos. Y nadie pinta.
//
// O sea que el guion iba demasiado despacio para el arranque **y** demasiado
// deprisa para el trabajo, a la vez. Un guion de duracion fija no puede
// acompanar a un trabajo de duracion variable: o sobra o falta.
//
// La salida es que el guion tenga un estado que **se repite mientras haya
// trabajo**. La animacion dura lo que dure el arranque, que es lo que se pedia
// desde el principio: *"mientras distrae al usuario"*.

/// Cada cuanto se repite el vaiven de la camara en la espera, en ms.
///
/// Veinticuatro segundos, y es **largo a proposito**. La camara recorre
/// [`VAIVEN_AMP`] px en media vuelta, o sea unos 33 px/s contra los 120 px/s de
/// la entrada: en un arranque normal --dos o tres segundos-- lo unico que se ve
/// es que la camara sigue andando, y el vaiven no se llega a notar. Solo un
/// arranque muy largo ve el camino de vuelta, y a esa velocidad se lee como una
/// grua lenta y no como un bucle.
pub const VAIVEN_MS: u32 = 24_000;

/// Cuanto se mueve la camara en la espera, en pixeles a cada lado.
///
/// ** ESTE NUMERO NO ES DE GUSTO: LO FIJA EL MUNDO QUE HAY GENERADO.
///
/// `Ciudad::nueva` genera torres hasta `ancho + ancho/2`, o sea que la camara
/// tiene **medio ancho de pantalla** de margen antes de quedarse sin ciudad y
/// dejar el borde derecho pelado. El caso mas apretado de los que arrancan aqui
/// es 1280 de ancho: 640 px de margen.
///
/// La camara llega a la espera con `FIN_GATO * 120 / 1000 = 180` px andados, y
/// desde ahi sube hasta `180 + 2 * VAIVEN_AMP`. Con 200 eso son 580 px, que
/// caben en los 640 con sitio de sobra. Subirlo pela el borde en el panel mas
/// estrecho, y **eso no se ve hasta que alguien arranca en 1280**.
///
/// Hay una prueba que lo sujeta: `la_espera_no_se_sale_del_mundo_generado`.
pub const VAIVEN_AMP: i32 = 200;

/// En que parte del guion estamos. Sirve para que quien pinta sepa **que ni
/// siquiera tiene que intentar dibujar**, que es mas barato que dibujarlo con
/// alfa cero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Acto {
    /// La ciudad sola, la camara ya andando. Todavia no hay gato.
    Ciudad,
    /// El gato aparece y **se enciende**: primero el trazo, luego los ojos.
    Gato,
    /// **El gato mirando, y la ciudad viva, mientras el kernel trabaja.**
    ///
    /// Se repite indefinidamente. Es el unico acto sin final propio: lo termina
    /// el arranque cuando ya no le queda nada que tapar.
    Espera,
    /// Los ojos toman el control: su cian crece hasta comerse la pantalla.
    Ojos,
    /// Todo negro, y el terminal entrando.
    Terminal,
}

/// Lo que hay en pantalla en un instante. Todo son numeros; nada son pixeles.
#[derive(Clone, Copy, Debug)]
pub struct Fotograma {
    pub acto: Acto,
    /// Pixeles que lleva avanzados la camara. Se lo pasa a `Camara`.
    pub avance: i32,
    /// Cuanto de la ciudad esta encendida, 0..100. Va subiendo: la maquina se
    /// enciende y la ciudad con ella.
    pub ciudad_pct: u32,
    /// Opacidad del trazo del gato, 0..255.
    pub gato_alfa: u32,
    /// Brillo de los ojos, 0..255. Sube DESPUES del trazo -- primero esta el
    /// gato, y luego abre los ojos.
    pub ojos_alfa: u32,
    /// Cuanto ha inundado la pantalla el cian de los ojos, 0..255.
    pub destello: u32,
    /// Cuanto negro hay encima de todo, 0..255. Al final es 255.
    pub negro: u32,
    /// Cuanto ha entrado el terminal, 0..100.
    pub terminal_pct: u32,
    /// **Cuantos pixeles flota el gato**, arriba o abajo del centro.
    ///
    /// Es lo que lo separa del fondo de verdad. Dos planos quietos uno sobre
    /// otro se leen como un collage por mucho que tengan brillos distintos; en
    /// cuanto uno se mueve a su ritmo, **el ojo los separa solo**. Es la misma
    /// pista que el paralaje, aplicada al primer plano.
    pub gato_flote: i32,
    /// El latido del neon de los ojos, 0..255, encima de `ojos_alfa`.
    ///
    /// Un neon perfectamente estable no parece neon: parece un LED. Lo que hace
    /// que se lea como tubo de gas es que respire.
    pub ojos_pulso: u32,
}

/// Una onda triangular de `-amp` a `+amp` con periodo `periodo_ms`.
///
/// Triangular y no senoidal porque aqui no hay coma flotante ni tabla de senos,
/// y a esta amplitud --unos pocos pixeles-- **no se distingue una de otra**. Una
/// tabla de senos por tres pixeles de flote seria pagar por una precision que
/// nadie puede ver.
fn onda(ms: u32, periodo_ms: u32, amp: i32) -> i32 {
    if periodo_ms == 0 {
        return 0;
    }
    let t = (ms % periodo_ms) as i32;
    let medio = (periodo_ms / 2) as i32;
    let subida = if t < medio { t } else { 2 * medio - t };
    // De 0..medio a -amp..+amp.
    (subida * 2 * amp) / medio.max(1) - amp
}

/// Interpolacion lineal entera de `a` a `b` segun `t` va de 0 a `t_max`.
///
/// [!] **Los dos sentidos.** La primera version hacia `a + (b - a) * t / t_max`
/// y solo servia para subir: con `b < a` la resta se desborda en `u32` --y en
/// release no avisa, da un numero gigante--. Lo destapo la prueba de los saltos
/// en el acto cuarto, donde el destello **baja** de 255 a 0. Es exactamente la
/// clase de fallo que un arranque animado esconde: se ve como un fogonazo de un
/// fotograma, y a ojo pasa por un parpadeo del monitor.
fn rampa(a: u32, b: u32, t: u32, t_max: u32) -> u32 {
    if t_max == 0 {
        return b;
    }
    let t = t.min(t_max);
    if b >= a {
        a + (b - a) * t / t_max
    } else {
        a - (a - b) * t / t_max
    }
}

/// **Que hay en pantalla en el milisegundo `ms` MIENTRAS EL KERNEL TRABAJA.**
///
/// Tres actos: la ciudad entrando, el gato encendiendose y **la espera, que se
/// repite sin final**. Pasado [`FIN_GATO`] esto ya no se acaba, y es a
/// proposito: quien decide que la intro termina no es el reloj sino el
/// arranque, y para eso esta [`cierre`].
///
/// [!] Antes esta funcion llevaba los cuatro actos y se acababa en
/// [`DURACION_MS`], devolviendo para siempre el ultimo fotograma. O sea que a
/// partir del segundo 2,4 la pantalla era **una foto**. En el video del arranque
/// real eso fueron tres segundos y medio de ciudad congelada, y el gato sin
/// salir ni una vez. Ver la seccion LA ESPERA de arriba.
pub fn fotograma(ms: u32) -> Fotograma {
    if ms >= FIN_GATO {
        return espera(ms);
    }
    // La camara no para en ningun acto: avanza durante toda la animacion, y por
    // eso se calcula fuera de la cascada. Que siga andando mientras el gato se
    // enciende es lo que hace que no parezcan dos videos pegados.
    let avance = (ms as i32) * 120 / 1000; // 120 px por segundo

    if ms < FIN_CIUDAD {
        return Fotograma {
            acto: Acto::Ciudad,
            avance,
            // La ciudad entra encendiendose sola: es lo primero que se ve, y
            // verla llenarse dice "esto esta arrancando" sin una palabra.
            ciudad_pct: rampa(0, 100, ms, FIN_CIUDAD),
            gato_alfa: 0,
            ojos_alfa: 0,
            destello: 0,
            negro: 0,
            terminal_pct: 0,
            gato_flote: 0,
            ojos_pulso: 0,
        };
    }

    // Y el ultimo tramo antes de la espera: `ms` esta entre FIN_CIUDAD y
    // FIN_GATO, que es lo unico que queda -- de ahi que no lleve `if` y de ahi
    // que no haga falta un `unreachable`, que en un kernel `no_std` es una
    // llamada al panic para un caso que la aritmetica ya descarto.
    let d = ms - FIN_CIUDAD;
    let dur = FIN_GATO - FIN_CIUDAD;
    // ** El trazo primero y los ojos DESPUES, con solape. Si los dos suben a
    // la vez, el gato aparece ya despierto y no hay nada que "se prenda".
    // Los ojos empiezan pasada la mitad.
    let mitad = dur / 2;
    Fotograma {
        acto: Acto::Gato,
        avance,
        ciudad_pct: 100,
        gato_alfa: rampa(0, 255, d, mitad),
        ojos_alfa: if d < mitad { 0 } else { rampa(0, 255, d - mitad, dur - mitad) },
        destello: 0,
        negro: 0,
        terminal_pct: 0,
        // El flote empieza EN CUANTO hay gato, no despues: si entrara
        // quieto y luego arrancara, se veria el momento de arrancar.
        gato_flote: onda(ms, 2600, 4),
        ojos_pulso: 0,
    }
}

/// **El fotograma de la ESPERA**: el gato mirando y la ciudad viva.
///
/// Se repite indefinidamente y no tiene final. Todo lo que se mueve aqui es
/// periodico, asi que da igual cuanto dure el arranque: no hay ningun instante
/// en el que la imagen se quede quieta ni ningun valor que crezca sin tope.
///
/// # Que se mueve, y por que solo eso
///
/// * **La camara**, en vaiven lento -- ver [`VAIVEN_MS`] y [`VAIVEN_AMP`]. Es lo
///   unico que no podia seguir como estaba: un avance lineal se sale del mundo
///   generado y deja el borde derecho pelado.
/// * **El flote del gato y el latido de su neon**, que ya eran ondas sobre el
///   reloj crudo y por tanto ya se repetian solos. No habia que tocarlos: lo que
///   les faltaba era que alguien pintase.
///
/// Y `ciudad_pct` NO se toca: lo manda el progreso real del arranque, que es
/// justo la informacion que la espera existe para ensenar. Una torre que tarda
/// en encenderse es un subsistema que tarda en arrancar.
fn espera(ms: u32) -> Fotograma {
    let d = ms - FIN_GATO;
    // El avance con el que llega la camara. Se calcula y no se copia: si manana
    // cambia la velocidad de entrada, el empalme sigue sin salto.
    let base = (FIN_GATO as i32) * 120 / 1000;
    // `onda` vale -amp en d=0, asi que sumarle la amplitud deja el vaiven
    // **empezando exactamente en `base`**: la camara no da un tiron al entrar en
    // la espera. Y nunca baja de ahi, o retrocederia sobre ciudad ya vista.
    let avance = base + VAIVEN_AMP + onda(d, VAIVEN_MS, VAIVEN_AMP);
    Fotograma {
        acto: Acto::Espera,
        avance,
        ciudad_pct: 100,
        gato_alfa: 255,
        ojos_alfa: 255,
        destello: 0,
        negro: 0,
        terminal_pct: 0,
        gato_flote: onda(ms, 2600, 4),
        ojos_pulso: onda(ms, 900, 40).unsigned_abs(),
    }
}

/// Cuanto dura el cierre, en ms. Son los dos ultimos actos.
pub const CIERRE_MS: u32 = DURACION_MS - FIN_GATO;

/// **El final: los ojos toman el control y todo se va a negro.**
///
/// `d` cuenta desde CERO, en el instante en que el arranque decide que ya no
/// tiene nada que tapar. Antes esto vivia dentro de [`fotograma`] y habia que
/// llamarlo con `FIN_GATO + d`, o sea sabiendose de memoria por donde iba el
/// guion; y como el arranque de verdad nunca llegaba a ese milisegundo, el
/// cierre **saltaba por encima del acto del gato** y el logo solo se veia
/// durante los ultimos 900 ms.
///
/// Ahora son dos funciones porque son dos preguntas: `fotograma` contesta *que
/// se ensena mientras trabajo* y esta contesta *como me despido*.
pub fn cierre(d: u32) -> Fotograma {
    let base = espera(FIN_GATO + d);
    let dur_ojos = FIN_OJOS - FIN_GATO;
    if d < dur_ojos {
        // El destello crece y el negro empieza a entrar por detras. Se solapan a
        // proposito: el cian no se apaga, **se lo traga el negro**, que es lo
        // que se pidio -- el gato toma el control y todo se vuelve negro.
        return Fotograma {
            acto: Acto::Ojos,
            destello: rampa(0, 255, d, dur_ojos),
            negro: rampa(0, 200, d, dur_ojos),
            ..base
        };
    }
    let d2 = d.saturating_sub(dur_ojos);
    let dur = CIERRE_MS - dur_ojos;
    Fotograma {
        acto: Acto::Terminal,
        // El destello se retira mientras el negro acaba de cerrar.
        destello: rampa(255, 0, d2, dur / 2),
        negro: rampa(200, 255, d2, dur / 2),
        terminal_pct: rampa(0, 100, d2, dur),
        ..base
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un arranque largo de verdad, para recorrer la espera unas cuantas
    /// vueltas. Minuto y medio: mas de lo que tarda ningun arranque, que es lo
    /// que hay que probar.
    const LARGO_MS: u32 = 90_000;

    /// ** LA PRUEBA QUE JUSTIFICA EL MODULO: recorrer la animacion entera
    /// milisegundo a milisegundo y comprobar que **ningun valor da un salto**.
    ///
    /// Un salto es lo que se ve como un parpadeo o un tiron, y es justo lo que
    /// se cuela al mover una duracion sin mirar las de al lado. A ojo hace falta
    /// reiniciar la maquina y tener suerte de estar mirando.
    ///
    /// [!] Ahora recorre **noventa segundos** y no dos y medio, porque el guion
    /// ya no se acaba: la espera se repite mientras el kernel trabaje. Si el
    /// empalme con la espera diera un tiron, aqui sale.
    #[test]
    fn ningun_valor_da_un_salto() {
        // 40 de 255 es un octavo: por debajo de eso el ojo lo lee como un
        // cambio continuo a treinta fotogramas por segundo.
        const TOPE: i64 = 40;
        let mut ant = fotograma(0);
        for ms in 1..=LARGO_MS {
            let f = fotograma(ms);
            let salto = |a: u32, b: u32| (a as i64 - b as i64).abs();
            assert!(salto(f.gato_alfa, ant.gato_alfa) <= TOPE, "el gato salta en {}", ms);
            assert!(salto(f.ojos_alfa, ant.ojos_alfa) <= TOPE, "los ojos saltan en {}", ms);
            assert!((f.avance - ant.avance).abs() <= 2, "la camara salta en {}", ms);
            ant = f;
        }
    }

    /// Y el cierre, igual: del primer milisegundo al ultimo sin tirones.
    #[test]
    fn el_cierre_no_da_saltos() {
        const TOPE: i64 = 40;
        let mut ant = cierre(0);
        for d in 1..=CIERRE_MS {
            let f = cierre(d);
            let salto = |a: u32, b: u32| (a as i64 - b as i64).abs();
            assert!(salto(f.destello, ant.destello) <= TOPE, "el destello salta en {}", d);
            assert!(salto(f.negro, ant.negro) <= TOPE, "el negro salta en {}", d);
            ant = f;
        }
    }

    /// ** Y EL EMPALME ENTRE LOS DOS, que es la costura nueva y por tanto la
    /// sospechosa.
    ///
    /// `cierre(0)` tiene que salir de donde estaba la espera. Si no, el arranque
    /// se ve como un corte: la pantalla da un tiron justo cuando el kernel
    /// termina, que es el peor momento para que parezca que algo fallo.
    #[test]
    fn el_cierre_empalma_con_la_espera() {
        let ultimo = fotograma(FIN_GATO);
        let primero = cierre(0);
        assert_eq!(primero.avance, ultimo.avance, "la camara da un tiron al cerrar");
        assert_eq!(primero.gato_alfa, ultimo.gato_alfa);
        assert_eq!(primero.destello, 0, "el cierre empieza con el destello ya encendido");
        assert_eq!(primero.negro, 0, "el cierre empieza con la pantalla ya apagada");
    }

    /// Los actos ocurren y en su orden: los tres del trabajo en `fotograma`, los
    /// dos del adios en `cierre`.
    #[test]
    fn los_actos_salen_y_en_orden() {
        let idx = |a: Acto| match a {
            Acto::Ciudad => 0,
            Acto::Gato => 1,
            Acto::Espera => 2,
            Acto::Ojos => 3,
            Acto::Terminal => 4,
        };
        let mut vistos = [false; 5];
        let mut ultimo = 0;
        for ms in 0..=LARGO_MS {
            let i = idx(fotograma(ms).acto);
            assert!(i >= ultimo, "el guion retrocedio a un acto anterior en {}", ms);
            ultimo = i;
            vistos[i] = true;
        }
        for d in 0..=CIERRE_MS {
            vistos[idx(cierre(d).acto)] = true;
        }
        assert!(vistos.iter().all(|&v| v), "hay un acto que no llega a verse");
    }

    /// ** LA ESPERA NO SE ACABA NUNCA, y eso es lo que se pidio: la animacion
    /// dura lo que dure el arranque.
    ///
    /// Es la prueba del fallo del video: el guion se acababa a los 2.400 ms y a
    /// partir de ahi la pantalla era una foto. Aqui se comprueba a los noventa
    /// segundos que sigue siendo la espera y que sigue **moviendose**.
    #[test]
    fn la_espera_no_se_acaba_y_sigue_viva() {
        assert_eq!(fotograma(LARGO_MS).acto, Acto::Espera);
        // En una ventana de un segundo cualquiera, algo tiene que cambiar. Si
        // los tres se quedaran quietos, la pantalla estaria congelada aunque el
        // acto dijera "Espera".
        for base in [3_000u32, 20_000, 60_000, 89_000] {
            let a = fotograma(base);
            let mut se_movio = false;
            for ms in base..base + 1_000 {
                let f = fotograma(ms);
                if f.avance != a.avance
                    || f.gato_flote != a.gato_flote
                    || f.ojos_pulso != a.ojos_pulso
                {
                    se_movio = true;
                    break;
                }
            }
            assert!(se_movio, "la espera se quedo congelada a partir del ms {}", base);
        }
    }

    /// ** Y NO SE SALE DEL MUNDO GENERADO. Es lo unico que la espera puede
    /// romper de verdad.
    ///
    /// `Ciudad::nueva` genera torres hasta `ancho + ancho/2`. Si la camara pasa
    /// de ese margen, el borde derecho se queda sin ciudad y se ve cielo pelado
    /// -- y solo en el panel mas estrecho, que es la clase de fallo que aparece
    /// en la maquina de otro.
    #[test]
    fn la_espera_no_se_sale_del_mundo_generado() {
        // El panel mas estrecho que arranca aqui. Su margen es la mitad.
        const ANCHO_MINIMO: i32 = 1280;
        let margen = ANCHO_MINIMO / 2;
        for ms in 0..=LARGO_MS {
            let a = fotograma(ms).avance;
            assert!(a >= 0, "la camara retrocedio antes del principio en {}", ms);
            assert!(
                a < margen,
                "en el ms {} la camara lleva {} px y el mundo se acaba en {}",
                ms,
                a,
                margen
            );
        }
    }

    /// La camara no retrocede DENTRO de la entrada. En la espera va y viene a
    /// proposito --ver [`VAIVEN_MS`]-- pero mientras el gato se enciende tiene
    /// que ir hacia delante o se leen como dos videos pegados.
    #[test]
    fn la_camara_avanza_durante_la_entrada() {
        let mut ant = -1;
        for ms in 0..=FIN_GATO {
            let a = fotograma(ms).avance;
            assert!(a >= ant, "la camara retrocedio en {}", ms);
            ant = a;
        }
        assert!(fotograma(FIN_GATO).avance > fotograma(0).avance);
    }

    /// Al final la pantalla esta NEGRA del todo y el terminal entero. Si no, el
    /// arranque entrega una pantalla a medio fundir y el shell aparece encima de
    /// la ciudad.
    #[test]
    fn acaba_en_negro_y_con_el_terminal_dentro() {
        let f = cierre(CIERRE_MS);
        assert_eq!(f.negro, 255);
        assert_eq!(f.terminal_pct, 100);
        assert_eq!(f.destello, 0);
    }

    /// Pasarse del final no rompe ni devuelve otra cosa: contesta el ultimo
    /// fotograma. Quien llama no tiene que comprobar el reloj.
    #[test]
    fn pasarse_del_final_devuelve_el_ultimo() {
        let a = cierre(CIERRE_MS);
        let b = cierre(CIERRE_MS * 10);
        assert_eq!((a.negro, a.terminal_pct), (b.negro, b.terminal_pct));
    }

    /// ** Los ojos se encienden DESPUES del trazo. Es lo que hace que el gato
    /// "se prenda" en vez de aparecer ya despierto.
    #[test]
    fn los_ojos_encienden_despues_del_trazo() {
        // En el instante en que el trazo llega a la mitad, los ojos siguen
        // apagados.
        let mut hallado = false;
        for ms in 0..FIN_GATO {
            let f = fotograma(ms);
            if f.gato_alfa >= 120 && f.gato_alfa < 255 {
                assert_eq!(f.ojos_alfa, 0, "los ojos ya estaban encendidos en {}", ms);
                hallado = true;
                break;
            }
        }
        assert!(hallado, "el trazo nunca paso por la mitad");
    }

    /// ** EL GATO LLEGA A ENCENDERSE ENTERO, y esta prueba nace del video del
    /// arranque real.
    ///
    /// Alli no se vio ni una vez: el arranque llegaba al 100% antes del ms 700 y
    /// el cierre entraba directamente en los ojos, saltandose el acto entero del
    /// gato. Con la espera, el logo esta encendido **desde el ms 1.500 hasta que
    /// el kernel termine**, dure lo que dure.
    #[test]
    fn el_gato_esta_encendido_durante_toda_la_espera() {
        for ms in [FIN_GATO, FIN_GATO + 1, 10_000, LARGO_MS] {
            let f = fotograma(ms);
            assert_eq!(f.gato_alfa, 255, "el gato no esta entero en el ms {}", ms);
            assert_eq!(f.ojos_alfa, 255, "los ojos no estan abiertos en el ms {}", ms);
        }
    }
}
