//! **La contabilidad de puertos**: a cual se puede tocar y a cual no.
//!
//! === Por que existe este modulo ===
//!
//! La foto del 2026-07-31 en el Ryzen: el teclado escribia **unos segundos** y
//! se moria, el RGB del raton parpadeando sin parar, y en el registro de
//! arranque los slots subiendo `0x30`, `0x31`, ... `0x40` hasta que el
//! controlador contesto `cc=0x9` (*No Slots Available*).
//!
//! Todo eso era **un solo bucle que se alimentaba a si mismo**:
//!
//! ```text
//!   el xHC avisa "cambio el puerto N"
//!     -> adoptar_puerto(N)
//!       -> port_reset(N)            <- y un reset ES un cambio de puerto
//!         -> el xHC avisa "cambio el puerto N"
//!           -> adoptar_puerto(N) ...
//! ```
//!
//! Y el puerto N era **el del propio teclado**, que ya estaba enumerado y
//! bombeando: cada vuelta lo reseteaba, lo volvia a direccionar en un slot
//! nuevo, encontraba que no habia nada que adoptar (su interfaz de teclado ya
//! estaba tomada), tiraba el slot y volvia a empezar. El teclado moria con el
//! primer reset -- de ahi los "unos segundos".
//!
//! === Las dos reglas, y por que son dos ===
//!
//! 1. **A un puerto que ya dio un aparato no se le vuelve a tocar.** Resetear
//!    un puerto que funciona no puede salir bien: en el mejor caso no cambia
//!    nada, en el peor mata lo que habia. Es la regla que salva al teclado.
//! 2. **Un puerto que no da nada se intenta un numero FINITO de veces.** La
//!    regla 1 no basta: un puerto con algo que no sabemos adoptar nunca queda
//!    "tomado", asi que sin un tope seguiria girando para siempre. Es la regla
//!    que corta la realimentacion.
//!
//! Ninguna de las dos impide el hot-plug de verdad: **desenchufar libera el
//! puerto y le devuelve los intentos**, que es lo que distingue "ya lo probe"
//! de "esto es otro aparato".
//!
//! Vive aparte del driver porque es la unica parte de todo esto que **se puede
//! probar sin un xHC delante** -- y era justo la parte que estaba mal.

/// Cuantos puertos se contabilizan.
///
/// * TREINTA Y DOS, y no dieciseis. El numero viejo era una suposicion --"el xHC
/// de esta placa declara menos"-- y un xHC declara los puertos USB2 y los USB3
/// por separado, asi que veinte o mas es corriente. Lo que hacia esa suposicion
/// no era desperdiciar memoria: era **cerrar el hot-plug de los puertos altos en
/// silencio**. Un puerto fuera de rango contesta `intentos = MAX_INTENTOS`, o
/// sea `se_puede_intentar = false`, o sea `adoptar_puerto` se va sin tocar el bus
/// y CABINA dice `nada que adoptar`. Y como el recorrido del arranque llama a
/// `cosechar_puerto` directamente --sin pasar por esta contabilidad--, el sintoma
/// era el peor posible: **enumera al arrancar y no vuelve nunca si se pierde**.
///
/// Cuesta un `u32` en vez de un `u16` y dieciseis bytes mas de array.
pub const MAX_PUERTOS: usize = 32;

/// Cuantas veces se intenta adoptar un puerto que no da nada.
///
/// Tres y no uno: la razon de que exista la re-enumeracion reactiva es que un
/// aparato puede tardar en engancharse (un raton con firmware RGB tarda mas
/// que un teclado). Tres y no infinito: ver la regla 2.
pub const MAX_INTENTOS: u8 = 3;

/// **Cuantos barridos descansa un puerto con los intentos gastados antes de
/// volver a intentarse** (2026-09-17). A 500 ms por barrido, cinco segundos.
///
/// Hasta hoy un puerto con algo dentro que fallaba tres veces se CERRABA hasta
/// desenchufarlo. Con el barrido cada 500 ms eso son 1,5 s: un raton con
/// firmware RGB, o un movil que aun esta arrancando, tarda mas -- y el dueno
/// lo veia como "se queda esperando entrar" y lo resolvia sacando y metiendo el
/// cable. Enfriar en vez de cerrar mantiene la regla 2 (nada gira para
/// siempre: tres resets cada cinco segundos, no doscientos por segundo) y
/// deja que lo lento acabe entrando.
pub const ENFRIAMIENTO_BARRIDOS: u8 = 10;

/// **Cuantos descansos seguidos doblan el siguiente** (2026-09-17, tarde).
///
/// La primera version del enfriamiento descansaba SIEMPRE cinco segundos, y
/// el Ryzen lo enseno al momento: un aparato que no contesta recibia tres
/// resets en 1,5 s, cinco segundos de paz, y otros tres -- para siempre, con
/// un aviso en CABINA cada cinco segundos. Eddi: *"es cada 5 segundos sin
/// sentido"*. Ahora cada descanso cumplido dobla el siguiente (5, 10, 20, 40
/// s) hasta este tope de doblados, y desenchufar lo devuelve todo a cero.
pub const MAX_DOBLADOS: u8 = 3;

/// **Tras cuantos descansos cumplidos se ABANDONA el puerto** hasta que se
/// desenchufe (2026-09-17, noche). Con `ENFRIAMIENTO_BARRIDOS` y
/// `MAX_DOBLADOS` son 5 + 10 + 20 + 40 = 75 segundos de intentos. Un aparato
/// de verdad contesta en segundos; uno que lleva mas de un minuto mudo no va a
/// entrar a base de resets, y cada reset congela el bus --y el teclado--
/// medio segundo. El Ryzen lo enseno con algo en el puerto 1 que acepta
/// direccion y no da descriptores. Desenchufar lo devuelve todo.
pub const ABANDONO_DESCANSOS: u8 = 4;

/// **Barridos de espera entre un intento fallido y el siguiente**: `1 <<
/// intentos` (2, 4, 8 barridos = 1, 2, 4 s). Antes los tres intentos caian
/// seguidos --uno por barrido y otro por el aviso del propio reset-- en
/// menos de 1,5 s, que es menos de lo que tarda un movil en volver a
/// presentarse tras cambiar de modo. Un aviso de enchufe que llegue durante
/// la espera NO gasta intento: `se_puede_intentar` dice que no.
pub fn espera_tras(intentos: u8) -> u8 {
    1u8 << intentos.min(3)
}

/// Que puertos estan tomados y cuantas veces se ha intentado cada uno.
#[derive(Debug, Clone, Copy)]
pub struct Puertos {
    tomados: u32,
    intentos: [u8; MAX_PUERTOS],
    /// Barridos que lleva descansando un puerto con los intentos gastados.
    enfriando: [u8; MAX_PUERTOS],
    /// Descansos cumplidos seguidos: cada uno dobla el siguiente.
    doblados: [u8; MAX_PUERTOS],
    /// Barridos que faltan antes de poder intentar otra vez.
    espera: [u8; MAX_PUERTOS],
}

impl Default for Puertos {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Puertos {
    pub const fn nuevo() -> Self {
        Self {
            tomados: 0,
            intentos: [0; MAX_PUERTOS],
            enfriando: [0; MAX_PUERTOS],
            doblados: [0; MAX_PUERTOS],
            espera: [0; MAX_PUERTOS],
        }
    }

    fn cabe(port: u8) -> bool {
        (port as usize) < MAX_PUERTOS
    }

    /// Esta tomado? Es decir: salio de ahi un aparato que esta funcionando?
    pub fn tomado(&self, port: u8) -> bool {
        Self::cabe(port) && self.tomados & (1 << port) != 0
    }

    /// Intentos gastados en este puerto.
    pub fn intentos(&self, port: u8) -> u8 {
        if Self::cabe(port) { self.intentos[port as usize] } else { MAX_INTENTOS }
    }

    /// * La pregunta que corta el bucle: **merece la pena tocar este puerto?**
    ///
    /// No, si ya dio un aparato (tocarlo solo puede romperlo) y no, si ya se
    /// intento lo suficiente. Un puerto fuera de rango tampoco: mejor no
    /// enumerar que enumerar a ciegas.
    pub fn se_puede_intentar(&self, port: u8) -> bool {
        Self::cabe(port)
            && !self.tomado(port)
            && self.intentos[port as usize] < MAX_INTENTOS
            && self.espera[port as usize] == 0
    }

    /// Esta esperando entre intentos? (y cuantos barridos le quedan)
    pub fn esperando(&self, port: u8) -> u8 {
        if Self::cabe(port) { self.espera[port as usize] } else { 0 }
    }

    /// Esta descansando con los intentos gastados?
    pub fn descansando(&self, port: u8) -> bool {
        Self::cabe(port) && !self.tomado(port) && self.intentos[port as usize] >= MAX_INTENTOS
    }

    /// Abandonado: no contesto en `ABANDONO_DESCANSOS` descansos. No se toca
    /// hasta que se desenchufe.
    pub fn abandonado(&self, port: u8) -> bool {
        Self::cabe(port) && self.doblados[port as usize] >= ABANDONO_DESCANSOS
    }

    /// **Un barrido mas de espera** entre intentos. `true` si ya puede.
    pub fn esperar(&mut self, port: u8) -> bool {
        if !Self::cabe(port) {
            return false;
        }
        let i = port as usize;
        if self.espera[i] > 0 {
            self.espera[i] -= 1;
        }
        self.espera[i] == 0
    }

    /// Cuantos segundos va a descansar este puerto en su proximo descanso, a
    /// medio segundo por barrido. Para decirlo en CABINA con el numero.
    pub fn descanso_s(&self, port: u8) -> u8 {
        if !Self::cabe(port) {
            return 0;
        }
        (ENFRIAMIENTO_BARRIDOS << self.doblados[port as usize].min(MAX_DOBLADOS)) / 2
    }

    /// Se va a intentar. Cuenta el intento **antes** de tocar el bus: si la
    /// enumeracion se cuelga o se sale por otro camino, el intento ya esta
    /// contado y el bucle no puede volver eternamente por el mismo sitio.
    pub fn anotar_intento(&mut self, port: u8) {
        if Self::cabe(port) {
            let i = port as usize;
            self.intentos[i] = self.intentos[i].saturating_add(1);
            // Si este intento falla, el siguiente espera; si acierta, `take`
            // o `aparcar` lo dejan sin efecto (el puerto queda tomado).
            self.espera[i] = espera_tras(self.intentos[i]);
        }
    }

    /// De aqui salio un aparato que esta instalado: el puerto queda tomado.
    pub fn take(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados |= 1 << port;
            self.espera[port as usize] = 0;
        }
    }

    /// **Aparcar**: el puerto contesto, se le leyeron los papeles y lo que hay
    /// no es mio (un movil, un disco, un hub). Se deja EN PAZ hasta que se
    /// desenchufe -- resetearlo cada cinco segundos seria molestar a un aparato
    /// sano por no tener driver. Es la misma marca que `take`, a proposito: las
    /// dos dicen "a este puerto no se le vuelve a tocar", y las dos se levantan
    /// al desenchufar. Lo que las distingue es quien lo cuenta, no el bit.
    pub fn aparcar(&mut self, port: u8) {
        self.take(port);
    }

    /// **Un barrido mas de descanso** para un puerto con los intentos gastados.
    /// Devuelve `true` cuando el descanso se cumplio y los intentos vuelven.
    /// Cada descanso cumplido dobla el siguiente, hasta `MAX_DOBLADOS`.
    pub fn enfriar(&mut self, port: u8) -> bool {
        if !Self::cabe(port) {
            return false;
        }
        let i = port as usize;
        if self.doblados[i] >= ABANDONO_DESCANSOS {
            // Abandonado: ni vuelve ni se cuenta. Solo desenchufar lo levanta.
            // `enfriando` pasa a 1 para que `recien_abandonado` sea verdad UNA
            // sola vez: el Ryzen enseno el mismo aviso cuarenta veces
            // (2026-09-17) porque aqui no se movia nada.
            self.enfriando[i] = 1;
            return false;
        }
        self.enfriando[i] = self.enfriando[i].saturating_add(1);
        let tope = ENFRIAMIENTO_BARRIDOS << self.doblados[i].min(MAX_DOBLADOS);
        if self.enfriando[i] >= tope {
            self.enfriando[i] = 0;
            self.doblados[i] = self.doblados[i].saturating_add(1);
            if self.doblados[i] >= ABANDONO_DESCANSOS {
                // El ultimo descanso no devuelve los intentos: se abandona.
                return false;
            }
            self.intentos[i] = 0;
            self.espera[i] = 0;
            return true;
        }
        false
    }

    /// Acaba de ser abandonado? (el barrido en que se decidio). Para avisar
    /// una vez.
    pub fn recien_abandonado(&self, port: u8) -> bool {
        Self::cabe(port)
            && self.doblados[port as usize] == ABANDONO_DESCANSOS
            && self.enfriando[port as usize] == 0
    }

    /// Acaba de entrar en descanso? (el primer barrido de este descanso).
    /// Para avisar UNA vez por descanso y no en cada evento.
    pub fn recien_descansando(&self, port: u8) -> bool {
        Self::cabe(port) && self.enfriando[port as usize] == 1
    }

    /// Se desenchufo: el puerto vuelve a estar libre **y con los intentos
    /// devueltos**. Sin esto, enchufar y desenchufar tres veces dejaria un
    /// puerto inservible hasta el siguiente reinicio.
    pub fn release(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados &= !(1 << port);
            self.intentos[port as usize] = 0;
            self.enfriando[port as usize] = 0;
            self.doblados[port as usize] = 0;
            self.espera[port as usize] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// * El fallo que mato al teclado en el Ryzen: el bucle de adopcion
    /// reseteaba el puerto donde ya habia un teclado enumerado y bombeando.
    #[test]
    fn a_un_puerto_que_ya_dio_un_aparato_no_se_le_vuelve_a_tocar() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(2);
        p.take(2);
        assert!(!p.se_puede_intentar(2), "resetearlo solo puede romperlo");
        assert!(p.tomado(2));
    }

    /// * Y la otra mitad: un puerto que NUNCA llega a estar tomado no puede
    /// girar para siempre. Es lo que pasaba con el puerto que enumeraba bien y
    /// no tenia nada que adoptar.
    #[test]
    fn un_puerto_que_no_da_nada_se_intenta_un_numero_finito_de_veces() {
        let mut p = Puertos::nuevo();
        gastar(&mut p, 3);
        assert!(!p.se_puede_intentar(3), "aqui es donde se corta la realimentacion");
    }

    /// Pero se reintenta lo justo: la re-enumeracion reactiva existe porque un
    /// aparato puede tardar en engancharse. Un solo intento seria volver al
    /// fallo anterior.
    #[test]
    fn se_reintenta_mas_de_una_vez() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(1);
        // Tras el fallo se espera (2026-09-17), pero se vuelve a poder.
        while !p.esperar(1) {}
        assert!(p.se_puede_intentar(1), "el primer fallo no puede ser el ultimo");
    }

    /// Desenchufar devuelve el puerto Y los intentos: lo que se enchufe despues
    /// es otro aparato y merece sus oportunidades.
    #[test]
    fn desenchufar_libera_el_puerto_y_devuelve_los_intentos() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            p.anotar_intento(4);
        }
        p.take(4);
        p.release(4);
        assert!(p.se_puede_intentar(4));
        assert_eq!(p.intentos(4), 0);
        assert!(!p.tomado(4));
    }

    /// Los puertos son independientes: agotar uno no puede cerrar el de al lado.
    #[test]
    fn los_puertos_no_se_pisan() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            p.anotar_intento(0);
        }
        p.take(5);
        assert!(!p.se_puede_intentar(0));
        assert!(!p.se_puede_intentar(5));
        assert!(p.se_puede_intentar(6), "este no tiene nada que ver");
    }

    /// Gasta los intentos de un puerto pasando por las esperas, como haria el
    /// barrido: intento, esperar lo que toque, intento...
    fn gastar(p: &mut Puertos, port: u8) {
        for _ in 0..MAX_INTENTOS {
            assert!(p.se_puede_intentar(port));
            p.anotar_intento(port);
            while !p.esperar(port) {}
        }
    }

    #[test]
    fn los_intentos_gastados_se_enfrian_y_vuelven() {
        let mut p = Puertos::nuevo();
        gastar(&mut p, 2);
        assert!(!p.se_puede_intentar(2));
        assert!(p.descansando(2));
        for _ in 0..ENFRIAMIENTO_BARRIDOS - 1 {
            assert!(!p.enfriar(2), "todavia descansa");
            assert!(!p.se_puede_intentar(2));
        }
        assert!(p.enfriar(2), "se cumplio el descanso");
        assert!(p.se_puede_intentar(2), "y los intentos volvieron");
        assert_eq!(p.intentos(2), 0);
    }

    /// Entre un intento fallido y el siguiente se espera, y cada vez mas: un
    /// aviso de enchufe en medio no gasta intento.
    #[test]
    fn entre_intentos_se_espera_y_cada_vez_mas() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(1);
        assert!(!p.se_puede_intentar(1), "recien fallado: se espera");
        assert_eq!(p.esperando(1), espera_tras(1));
        assert!(espera_tras(2) > espera_tras(1) && espera_tras(3) > espera_tras(2));
        for _ in 0..espera_tras(1) - 1 {
            assert!(!p.esperar(1));
        }
        assert!(p.esperar(1), "ya paso la espera");
        assert!(p.se_puede_intentar(1));
    }

    /// Cada descanso cumplido dobla el siguiente, con tope; desenchufar lo
    /// devuelve a cinco segundos.
    #[test]
    fn cada_descanso_dobla_el_siguiente_hasta_el_tope() {
        let mut p = Puertos::nuevo();
        let mut anteriores = 0u32;
        for ciclo in 0..ABANDONO_DESCANSOS as u32 - 1 {
            gastar(&mut p, 3);
            let mut barridos = 0u32;
            while !p.enfriar(3) {
                barridos += 1;
            }
            barridos += 1;
            let esperado = (ENFRIAMIENTO_BARRIDOS as u32) << ciclo.min(MAX_DOBLADOS as u32);
            assert_eq!(barridos, esperado, "ciclo {ciclo}");
            assert!(barridos >= anteriores);
            anteriores = barridos;
        }
        p.release(3);
        gastar(&mut p, 3);
        let mut barridos = 0u32;
        while !p.enfriar(3) {
            barridos += 1;
        }
        assert_eq!(barridos + 1, ENFRIAMIENTO_BARRIDOS as u32, "desenchufar devuelve el descanso corto");
    }

    /// Tras `ABANDONO_DESCANSOS` descansos sin contestar, el puerto se abandona:
    /// no vuelve a intentarse hasta desenchufar. Un reset cada poco congela el
    /// teclado, y un mudo de un minuto no va a entrar por resetearlo mas.
    #[test]
    fn un_puerto_mudo_se_abandona_hasta_desenchufar() {
        let mut p = Puertos::nuevo();
        for _ in 0..ABANDONO_DESCANSOS - 1 {
            gastar(&mut p, 5);
            while !p.enfriar(5) {}
            assert!(!p.abandonado(5));
        }
        gastar(&mut p, 5);
        // El ultimo descanso no devuelve los intentos.
        for _ in 0..(ENFRIAMIENTO_BARRIDOS as u32) << MAX_DOBLADOS {
            assert!(!p.enfriar(5));
        }
        assert!(p.abandonado(5));
        assert!(p.recien_abandonado(5), "una vez: el barrido que lo decidio");
        assert!(!p.se_puede_intentar(5));
        for _ in 0..1000 {
            assert!(!p.enfriar(5), "abandonado no vuelve solo");
            assert!(!p.recien_abandonado(5), "y no vuelve a ser 'recien'");
        }
        p.release(5);
        assert!(!p.abandonado(5));
        assert!(p.se_puede_intentar(5), "desenchufar lo levanta");
    }

    #[test]
    fn un_puerto_aparcado_no_se_toca_hasta_desenchufar() {
        let mut p = Puertos::nuevo();
        p.aparcar(7);
        assert!(!p.se_puede_intentar(7));
        assert!(p.tomado(7));
        p.release(7);
        assert!(p.se_puede_intentar(7));
    }

    /// Un puerto fuera de rango no rompe nada y no se enumera.
    #[test]
    fn un_puerto_fuera_de_rango_no_se_toca_ni_desborda() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(200);
        p.take(200);
        p.release(200);
        assert!(!p.se_puede_intentar(200));
        assert!(!p.tomado(200));
    }

    /// El estado inicial: todos libres. Si esto fallara, el arranque no
    /// enumeraria nada y la maquina se quedaria sin teclado.
    #[test]
    fn al_arrancar_todos_los_puertos_son_intentables() {
        let p = Puertos::nuevo();
        for puerto in 0..MAX_PUERTOS as u8 {
            assert!(p.se_puede_intentar(puerto));
        }
    }
}
