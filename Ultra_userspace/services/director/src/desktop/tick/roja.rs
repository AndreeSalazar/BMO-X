//! **CARRIL ROJO** -- dar el turno. Si esto se equivoca, se queda la maquina.
//!
//! [carril]  ROJO      es la unica parte del reloj que ACTUA. Todo lo demas
//!           mide, y medir mal no para nada
//!
//! [cuesta]  MAQUINA -- y esta probado el mismo dia que se escribio. Las dos
//!           averias de este fichero salieron de aqui: una se quedo el nucleo
//!           entero --el dueno vio su teclado ignorado-- y la otra tumbo el
//!           kernel con `ROTTEN CONTEXT: the seal is gone`. Ninguna vino de
//!           contar mal.
//!
//! [riesgo]  RELOJ AJENO
//!           RELOJ -- todo depende de que el latido llegue. Sin el, `ceder`
//!                    degrada a girar cediendo, y hay que notarlo.
//!           AJENO -- el numero que devuelve `WAIT` lo escribe OTRO, y encima
//!                    MIENTE cuando falla: un error trae `value = 0`, que con
//!                    el testigo en 0 se confunde con "durmio". Por eso el
//!                    juez de si durmio es el RELOJ y no lo que contesto.
//!
//! # La invariante, y es toda la razon de que este fichero exista aparte
//!
//! > **El bucle no puede acabar una vuelta sin soltar el turno, haga `WAIT` lo
//! > que haga.**
//!
//! Una optimizacion que se apaga sola tiene que apagarse HACIA EL LADO SEGURO.
//! La primera version de `ceder` se apagaba hacia el peor que habia: sustituyo
//! a un `yield_screen()` incondicional, `WAIT` fallo, y el escritorio dejo de
//! ceder. Ver `BITACORA.md`, Ep. 50.

use bmo_userland as bmo;

use super::{Tick, NO_CLOCK};

impl Tick {
    pub fn tomar_latido(&mut self) {
        if let Some(h) = bmo::latido_tomar() {
            self.latido = h;
            // El testigo arranca en `0` y se pone al dia SOLO en la primera
            // vuelta. No se pregunta la cuenta aqui, y eso no es pereza: ver
            // "el testigo se saca del propio WAIT" en `ceder`.
            self.visto = 0;
        }
    }

    /// **El escritorio va montado en el reloj del hardware Y ESO FUNCIONA.**
    ///
    /// ** No dice "tengo el handle": dice "he dormido". La version anterior
    /// contestaba lo primero, y por eso la barra puso `latido` con toda la
    /// confianza del mundo mientras `WAIT` volvia en el acto trece mil veces
    /// por segundo. El letrero existe para distinguir los dos modos, asi que
    /// tiene que mirar el modo, no el permiso.
    ///
    /// Basta con que haya dormido ALGUNA vez en el ultimo segundo: una vuelta
    /// con trabajo de verdad no duerme, y eso esta bien. Lo que no puede pasar
    /// desapercibido es que no duerma NINGUNA.
    pub fn en_latido(&self) -> bool {
        self.latido != 0 && self.dormidas_por_segundo > 0
    }

    /// El plazo de seguridad del `WAIT`, en nanosegundos.
    ///
    /// ** NO se pone `0` --que seria "solo el latido"-- y el motivo esta escrito
    /// dos veces en esta casa: un bloqueo que solo despierta un aviso se cuelga
    /// para siempre el dia que el aviso no llegue. Ver `dormir_un_rato`, que ya
    /// se lo encontro.
    ///
    /// 50 ms es el suelo: si el latido se parara, el escritorio seguiria dando
    /// 20 vueltas por segundo y **el pulso lo diria en la barra** en vez de
    /// quedarse negro. Un instrumento tiene que sobrevivir a lo que mide.
    const PLAZO_NS: u64 = 50_000_000;

    /// **Devolver el turno**, que es lo que cierra cada vuelta.
    ///
    /// Hace dos cosas y las dos van juntas a proposito: cierra el tramo del
    /// CUERPO --un `rdtsc`, sin cruzar ninguna puerta-- y da el turno. Que sea
    /// un solo sitio es lo que garantiza que las dos mitades del segundo sumen:
    /// medir en un metodo y ceder en otro deja un hueco entre los dos que no
    /// cuenta nadie.
    ///
    /// # Las dos formas de dar el turno, y en que se diferencian
    ///
    /// ```text
    ///    yield_screen   "quitame de en medio y devuelveme el turno ya"
    ///                   -> vuelve en cuanto no haya nadie mejor. Girar con
    ///                      buenos modales
    ///    WAIT(latido)   "despiertame cuando lata el reloj"
    ///                   -> el nucleo queda LIBRE hasta entonces
    /// ```
    ///
    /// *** El techo util de este bucle son 250 vueltas/s --lo pone el bus USB,
    /// que late cada 4 ms-- y el latido va a 1 kHz, o sea CUATRO VECES por
    /// encima de lo que la entrada puede refrescar. No se pierde ni un evento y
    /// se deja de quemar el nucleo para descubrir que no ha pasado nada.
    ///
    /// # *** EL TESTIGO SE SACA DEL PROPIO `WAIT`, y la primera version no
    ///
    /// La primera version releia la cuenta con `latido_cuenta` antes de esperar.
    /// **Y no funciono en el metal**, con un numero que no dejaba dudas:
    ///
    /// ```text
    ///    latido 12937/s   pinta 3   cuerpo 1066   puerta 29
    /// ```
    ///
    /// Trece mil vueltas pidiendo dormir mil veces, 29 ms de puerta en todo un
    /// segundo, y el cuerpo quedandose el resto. **`WAIT` no durmio ni una vez.**
    ///
    /// La cadena entera, y cada eslabon estaba escrito:
    ///
    /// ```text
    ///    latido::claim   cap::grant(..., RIGHT_WAIT, ...)   solo ese derecho
    ///                    y su comentario lo dice: "sobre este handle no se
    ///                    lee ni se escribe nada, se espera"
    ///    latido_cuenta   es un INVOKE -> resuelve con RIGHT_READ -> FALLA
    ///    .unwrap_or(0)   se traga el fallo -> `visto` = 0 PARA SIEMPRE
    ///    WAIT            `current != observed` (0) -> vuelve EN EL ACTO
    /// ```
    ///
    /// ** Y lo caro no fue no dormir: fue **dejar de ceder**. Este metodo habia
    /// sustituido al `yield_screen()` incondicional, asi que el escritorio se
    /// quedo el nucleo entero y el teclado y el raton del dueno parecieron
    /// ignorados. Una optimizacion que se apaga sola tiene que apagarse HACIA
    /// EL LADO SEGURO, y esta se apagaba hacia el peor.
    ///
    /// # Como se sabe si durmio: SE MIRA EL RELOJ, no lo que contesto
    ///
    /// El valor que devuelve `WAIT` es *advisory*, y encima **miente cuando
    /// falla**: un error trae `value = 0`, que con el testigo en 0 se confunde
    /// con "durmio". Preguntarle al mecanismo por su propio estado es
    /// preguntarle al sospechoso -- ver [`Tick::dormidas`].
    ///
    /// El reloj no es sospechoso:
    ///
    /// ```text
    ///    paso ~un milisegundo   durmio     -> el testigo avanza uno
    ///    no paso nada           NO durmio  -> se pone al dia con lo que
    ///                                         contesto **y SE CEDE IGUAL**
    /// ```
    ///
    /// ** Y esa ultima linea es la invariante entera: el bucle no puede acabar
    /// una vuelta sin soltar el turno, haga `WAIT` lo que haga. Si el mecanismo
    /// se rompe, esto degrada a lo que habia antes --girar CEDIENDO-- que es
    /// lento y no se lleva el teclado por delante.
    pub fn ceder(&mut self) {
        let antes = bmo::ciclos();
        let con_reloj = self.tsc_hz != 0 && self.tsc_hz != NO_CLOCK;
        if con_reloj {
            // ** EL CUERPO SE MIDE CON EL RELOJ DE CPU PROPIO, no con el de
            // pared (2026-09-08). `INFO_CPU_PROPIO` cuenta los ciclos que esta
            // tarea ha CORRIDO; `bmo::ciclos()` cuenta los que han pasado.
            // Cuando al compositor lo echan del CPU en mitad de su vuelta, el
            // segundo sube y el primero no -- y esa diferencia es exactamente
            // lo que confundio la caza del 08-09. Escalon E1 de
            // `docs/plan/PLAN_EL_COMPAS.md`.
            let cpu = bmo::info(bmo::INFO_CPU_PROPIO);
            if cpu != 0 && self.cpu_inicio != 0 {
                self.suma_cuerpo = self.suma_cuerpo
                    .wrapping_add(cpu.wrapping_sub(self.cpu_inicio));
            }
            self.cedio_en = antes;
        }
        if self.latido == 0 {
            bmo::yield_screen();
            return;
        }
        // *** EL TESTIGO SE RELEE, y esta es la tercera version de esta linea.
        //
        // La segunda lo sacaba del valor que devuelve `WAIT`, para no cruzar una
        // puerta. **Y eso solo vale si el bucle es MAS RAPIDO que el latido.**
        // El metal dijo que no lo es:
        //
        // ```text
        //    pulso 4/s   pinta 4   cuerpo 2   puerta 1237
        // ```
        //
        // Cuatro vueltas por segundo, o sea 300 ms por vuelta, o sea **300
        // latidos entre dos vueltas**. Con el testigo sacado de la vuelta
        // anterior llega caducado SIEMPRE, `current != observed`, y `WAIT`
        // vuelve en el acto -> se cede -> 300 ms -> y otra vez. Un circulo
        // vicioso que se alimenta de su propia lentitud.
        //
        // ** Releer cuesta UNA puerta: 969 ciclos sobre una vuelta de 300 ms es
        // la tres millonesima parte. Y ahora se PUEDE, porque el mismo dia se
        // arreglo el derecho que faltaba en `latido::claim`. Las dos mitades del
        // arreglo eran una sola pieza y las separe: esto lo junta.
        self.visto = bmo::latido_cuenta(self.latido).unwrap_or(self.visto);
        let vuelve = bmo::latido_esperar(self.latido, self.visto, Self::PLAZO_NS);
        // ** EL JUEZ ES EL RELOJ. Un latido son 1.000 us y una puerta 0,26, asi
        // que el umbral --la decima parte de un latido-- esta a 380 veces una
        // puerta y a 10 veces por debajo de un latido. No hay forma de
        // confundir las dos cosas.
        let durmio = con_reloj
            && bmo::ciclos().wrapping_sub(antes) >= (self.ciclos_de(1) / 10).max(1);
        if durmio {
            self.dormidas = self.dormidas.wrapping_add(1);
        } else {
            // No durmio. `vuelve` no se usa para nada --la cuenta se relee
            // arriba-- y lo unico que importa aqui es que **ceder no es
            // opcional**: si el mecanismo no duerme, esto degrada a girar
            // cediendo, que es lento y no se lleva el teclado por delante.
            let _ = vuelve;
            bmo::yield_screen();
        }
    }
}
