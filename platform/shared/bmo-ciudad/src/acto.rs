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
const FIN_CIUDAD: u32 = 700;
const FIN_GATO: u32 = 1500;
const FIN_OJOS: u32 = 2000;
// El cuarto acaba en DURACION_MS.

/// En que parte del guion estamos. Sirve para que quien pinta sepa **que ni
/// siquiera tiene que intentar dibujar**, que es mas barato que dibujarlo con
/// alfa cero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Acto {
    /// La ciudad sola, la camara ya andando. Todavia no hay gato.
    Ciudad,
    /// El gato aparece y **se enciende**: primero el trazo, luego los ojos.
    Gato,
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

/// **Que hay en pantalla en el milisegundo `ms`.**
///
/// Pasado [`DURACION_MS`] devuelve siempre el ultimo fotograma: pantalla negra y
/// terminal entero. Asi quien llama puede pasarse sin comprobar nada.
pub fn fotograma(ms: u32) -> Fotograma {
    // La camara no para en ningun acto: avanza durante toda la animacion, y por
    // eso se calcula fuera de la cascada. Que siga andando mientras el gato se
    // enciende es lo que hace que no parezcan dos videos pegados.
    let t = ms.min(DURACION_MS);
    let avance = (t as i32) * 120 / 1000; // 120 px por segundo

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
        };
    }

    if ms < FIN_GATO {
        let d = ms - FIN_CIUDAD;
        let dur = FIN_GATO - FIN_CIUDAD;
        // ** El trazo primero y los ojos DESPUES, con solape. Si los dos suben a
        // la vez, el gato aparece ya despierto y no hay nada que "se prenda".
        // Los ojos empiezan pasada la mitad.
        let mitad = dur / 2;
        return Fotograma {
            acto: Acto::Gato,
            avance,
            ciudad_pct: 100,
            gato_alfa: rampa(0, 255, d, mitad),
            ojos_alfa: if d < mitad { 0 } else { rampa(0, 255, d - mitad, dur - mitad) },
            destello: 0,
            negro: 0,
            terminal_pct: 0,
        };
    }

    if ms < FIN_OJOS {
        let d = ms - FIN_GATO;
        let dur = FIN_OJOS - FIN_GATO;
        // El destello crece y el negro empieza a entrar por detras. Se solapan a
        // proposito: el cian no se apaga, **se lo traga el negro**, que es lo
        // que se pidio -- el gato toma el control y todo se vuelve negro.
        return Fotograma {
            acto: Acto::Ojos,
            avance,
            ciudad_pct: 100,
            gato_alfa: 255,
            ojos_alfa: 255,
            destello: rampa(0, 255, d, dur),
            negro: rampa(0, 200, d, dur),
            terminal_pct: 0,
        };
    }

    let d = ms.saturating_sub(FIN_OJOS);
    let dur = DURACION_MS - FIN_OJOS;
    Fotograma {
        acto: Acto::Terminal,
        avance,
        ciudad_pct: 100,
        gato_alfa: 255,
        ojos_alfa: 255,
        // El destello se retira mientras el negro acaba de cerrar.
        destello: rampa(255, 0, d, dur / 2),
        negro: rampa(200, 255, d, dur / 2),
        terminal_pct: rampa(0, 100, d, dur),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// ** LA PRUEBA QUE JUSTIFICA EL MODULO: recorrer la animacion entera
    /// milisegundo a milisegundo y comprobar que **ningun valor da un salto**.
    ///
    /// Un salto es lo que se ve como un parpadeo o un tiron, y es justo lo que
    /// se cuela al mover una duracion sin mirar las de al lado. A ojo hace falta
    /// reiniciar la maquina y tener suerte de estar mirando; aqui son 2.400
    /// comparaciones.
    #[test]
    fn ningun_valor_da_un_salto() {
        // 40 de 255 es un octavo: por debajo de eso el ojo lo lee como un
        // cambio continuo a treinta fotogramas por segundo.
        const TOPE: i64 = 40;
        let mut ant = fotograma(0);
        for ms in 1..=DURACION_MS {
            let f = fotograma(ms);
            let salto = |a: u32, b: u32| (a as i64 - b as i64).abs();
            assert!(salto(f.gato_alfa, ant.gato_alfa) <= TOPE, "el gato salta en {}", ms);
            assert!(salto(f.ojos_alfa, ant.ojos_alfa) <= TOPE, "los ojos saltan en {}", ms);
            assert!(salto(f.destello, ant.destello) <= TOPE, "el destello salta en {}", ms);
            assert!(salto(f.negro, ant.negro) <= TOPE, "el negro salta en {}", ms);
            ant = f;
        }
    }

    /// Los cuatro actos ocurren, y en su orden. Si uno desaparece --porque dos
    /// fronteras se cruzaron al editarlas-- esto lo dice.
    #[test]
    fn los_cuatro_actos_salen_y_en_orden() {
        let mut vistos = [false; 4];
        let idx = |a: Acto| match a {
            Acto::Ciudad => 0,
            Acto::Gato => 1,
            Acto::Ojos => 2,
            Acto::Terminal => 3,
        };
        let mut ultimo = 0;
        for ms in 0..=DURACION_MS {
            let i = idx(fotograma(ms).acto);
            assert!(i >= ultimo, "el guion retrocedio a un acto anterior en {}", ms);
            ultimo = i;
            vistos[i] = true;
        }
        assert!(vistos.iter().all(|&v| v), "hay un acto que no llega a verse");
    }

    /// La camara no para en ningun acto. Si parase, se leerian como dos videos
    /// pegados en vez de una toma.
    #[test]
    fn la_camara_avanza_de_principio_a_fin() {
        let mut ant = -1;
        for ms in 0..=DURACION_MS {
            let a = fotograma(ms).avance;
            assert!(a >= ant, "la camara retrocedio en {}", ms);
            ant = a;
        }
        assert!(fotograma(DURACION_MS).avance > fotograma(0).avance);
    }

    /// Al final la pantalla esta NEGRA del todo y el terminal entero. Si no, el
    /// arranque entrega una pantalla a medio fundir y el shell aparece encima de
    /// la ciudad.
    #[test]
    fn acaba_en_negro_y_con_el_terminal_dentro() {
        let f = fotograma(DURACION_MS);
        assert_eq!(f.negro, 255);
        assert_eq!(f.terminal_pct, 100);
        assert_eq!(f.destello, 0);
    }

    /// Pasarse del final no rompe ni devuelve otra cosa: contesta el ultimo
    /// fotograma. Quien llama no tiene que comprobar el reloj.
    #[test]
    fn pasarse_del_final_devuelve_el_ultimo() {
        let a = fotograma(DURACION_MS);
        let b = fotograma(DURACION_MS * 10);
        assert_eq!((a.negro, a.terminal_pct), (b.negro, b.terminal_pct));
    }

    /// ** Los ojos se encienden DESPUES del trazo. Es lo que hace que el gato
    /// "se prenda" en vez de aparecer ya despierto.
    #[test]
    fn los_ojos_encienden_despues_del_trazo() {
        // En el instante en que el trazo llega a la mitad, los ojos siguen
        // apagados.
        let mut hallado = false;
        for ms in 0..DURACION_MS {
            let f = fotograma(ms);
            if f.gato_alfa >= 120 && f.gato_alfa < 255 {
                assert_eq!(f.ojos_alfa, 0, "los ojos ya estaban encendidos en {}", ms);
                hallado = true;
                break;
            }
        }
        assert!(hallado, "el trazo nunca paso por la mitad");
    }
}
