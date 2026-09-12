//! **CARRIL AMARILLO** -- medir. Si esto se equivoca no falla: convence.
//!
//! [consumo] NADA      mide, y solo cuando el bucle le pasa por encima. El que
//!                     late es `main.rs` y el que duerme es `tick/roja.rs`
//!                     (L6h)
//!
//! [carril]  AMARILLO  un instrumento. Su modo de fallo no es romperse, es
//!           seguir andando y decir algo que no es
//!
//! [cuesta]  NADA -- contar mal no rompe una app, ni un dato, ni la maquina. Y
//!           esa es la trampa: por eso el carril y el coste no dicen lo mismo.
//!           El precedente lo escribio la ley con nombre --`core/autopsy.rs`,
//!           coste NADA y carril AMARILLO-- por la misma razon: **manda la
//!           investigacion al sitio equivocado**.
//!
//! [riesgo]  RELOJ SILENCIO
//!           RELOJ    -- todo sale de `INFO_TSC_HZ`. Sin el no hay ni una cifra
//!                       que signifique algo, y por eso se DICE en vez de dar
//!                       un cero.
//!           SILENCIO -- ninguno de sus fallos da error. Los tres que ha tenido
//!                       --el numero sin aguja, el cero sin reloj, y `cuerpo`
//!                       midiendo reloj de pared-- compilaban, corrian y
//!                       mentian. Los tres se cazaron EN EL METAL.
//!
//! # Lo que mide, y por que cada cosa
//!
//! ```text
//!    loops_per_second   el bucle VIVE, y a que ritmo
//!    quarter            el suelo de repintado, medido con el TSC y no
//!                       contando vueltas -- ese fue el primer fallo
//!    cuerpo / puerta    en que se va el segundo: trabajar o esperar turno
//!    pintados           cuantas de esas vueltas SIRVIERON
//!    trafico            puertas por vuelta, x10. La cifra que sostenia el
//!                       presupuesto del bucle y que nadie habia medido
//! ```

use bmo_userland as bmo;

use super::{Tick, NO_CLOCK, QUARTER_LOOPS, QUARTER_MS};

impl Tick {

    /// **One pass of the main loop.** Counts it, raises the rhythm edge, and
    /// closes the second when it is due.
    ///
    /// `bmo::ciclos()` is `rdtsc`, not a syscall -- a couple of dozen cycles --
    /// so reading it every pass is affordable. What is not affordable is kept
    /// out on purpose: see `tsc_hz`.
    pub fn pulse(&mut self) {
        self.loops = self.loops.wrapping_add(1);
        let now = bmo::ciclos();
        if self.tsc_hz == 0 {
            let hz = bmo::info(bmo::INFO_TSC_HZ);
            self.tsc_hz = if hz == 0 { NO_CLOCK } else { hz };
            self.sample_at = now;
            self.sample_loops = self.loops;
            self.quarter_at = now;
            self.inicio = now;
            self.quarter = false;
            return;
        }
        if self.tsc_hz == NO_CLOCK {
            // Sin reloj no hay ritmo que medir: se cuenta como se contaba.
            self.inicio = now;
            self.quarter = self.loops % QUARTER_LOOPS == 0;
            return;
        }
        // ** SE CIERRA EL TRAMO DE LA PUERTA. Entre `cediendo()` y este
        // instante no corrio ni una linea del compositor: lo que haya pasado
        // ahi es tiempo que el escritorio ESPERO, no que gasto.
        if self.cedio_en != 0 {
            self.suma_puerta = self.suma_puerta.wrapping_add(now.wrapping_sub(self.cedio_en));
        }
        self.inicio = now;
        self.cpu_inicio = bmo::info(bmo::INFO_CPU_PROPIO);
        self.quarter = now.wrapping_sub(self.quarter_at) >= self.ciclos_de(QUARTER_MS);
        if self.quarter {
            self.quarter_at = now;
        }
        if now.wrapping_sub(self.sample_at) >= self.tsc_hz {
            self.loops_per_second = self.loops.wrapping_sub(self.sample_loops);
            // El reparto del segundo que se cierra, y a cero para el siguiente.
            // Se publica en ms enteros: decimas de ms en una barra de tareas es
            // precision que nadie puede usar leyendo de lejos.
            let por_ms = self.tsc_hz / 1_000;
            if por_ms > 0 {
                self.cuerpo_ms = (self.suma_cuerpo / por_ms) as u32;
                self.puerta_ms = (self.suma_puerta / por_ms) as u32;
            }
            // == *** EL TRAFICO DE PUERTAS, Y SE MIDE AQUI PORQUE AQUI ES
            // ==     CASI GRATIS (2026-09-12) ===========================
            //
            // UNA puerta por segundo para contar las ~9.000 de ese segundo:
            // el 0,01 %. En cada vuelta serian 9 -> 10, un 11 % mas de puertas
            // para poder contarlas. Ver `Tick::trafico_x10`, que lleva lo que
            // este numero NO es: es el trafico de la maquina, no el de este
            // proceso.
            // Las vueltas del segundo son `loops_per_second`, calculado ocho
            // lineas arriba. No se vuelve a restar: dos restas que tienen que
            // dar lo mismo son dos restas que un dia no lo daran.
            let t = bmo::info(bmo::INFO_SYSCALL_CUENTA);
            if self.trafico_visto != 0 && self.loops_per_second > 0 {
                let d = t.wrapping_sub(self.trafico_visto);
                self.trafico_x10 =
                    (d.saturating_mul(10) / self.loops_per_second as u64) as u32;
            }
            self.trafico_visto = t;
            self.pintados_por_segundo = self.pintados;
            self.pintados = 0;
            self.dormidas_por_segundo = self.dormidas;
            self.dormidas = 0;
            self.reposos_por_segundo = self.reposos;
            self.reposos = 0;
            self.suma_cuerpo = 0;
            self.suma_puerta = 0;
            self.sample_at = now;
            self.sample_loops = self.loops;
        }
    }

    /// **Esta vuelta pinto.** Una suma, sin cruzar ninguna puerta.
    ///
    /// Lo llama `compose`, que es el unico sitio donde `will_paint` ya es
    /// definitivo: la recogida de entrada todavia lo puede subir.
    pub fn anota_pintado(&mut self) {
        if self.will_paint {
            self.pintados = self.pintados.wrapping_add(1);
        }
    }

    /// **Tomar el LATIDO.** Una vez en la vida del proceso, al arrancar.
    ///
    /// Si el kernel dice que no, `latido` se queda en `0` y `ceder` sigue
    /// girando como giraba. **Nunca peor que antes** -- y se ve en la barra,
    /// porque la caja del pulso cambia de nombre segun por donde este.

    /// **No hay reloj de referencia**: el kernel contesto `0` a `INFO_TSC_HZ`.
    ///
    /// == ** POR QUE ESTO SE PUBLICA (2026-09-08) ==========================
    ///
    /// Sin reloj, `pulse` sale por su rama de emergencia y **`loops_per_second`
    /// no se calcula nunca**: se queda en el `0` con el que nacio. Eso esta
    /// escrito arriba, en el propio campo... y aun asi la barra pintaba
    /// `pulso 0/s`, que cualquiera lee como *"el bucle esta muerto"*.
    ///
    /// ```text
    ///    lo que pasa       no tengo con que medir el ritmo
    ///    lo que se veia    el ritmo es CERO
    /// ```
    ///
    /// ** Son dos cosas distintas y se veian iguales -- la misma clase de fallo
    /// que la aguja acaba de arreglar, un escalon mas abajo. Un instrumento que
    /// no sabe la respuesta tiene que decir QUE NO LA SABE, no dar un cero: un
    /// cero es una medida, y esa medida nunca se tomo.
    pub fn sin_reloj(&self) -> bool {
        self.tsc_hz == NO_CLOCK
    }

    /// The same quarter second as `quarter`, in cycles, for whoever cannot read
    /// the edge because they are not called on every pass.
    pub fn quarter_cycles(&self) -> u64 {
        self.ciclos_de(QUARTER_MS)
    }

    /// How many cycles `ms` milliseconds are **on this machine**.
    ///
    /// It is handed out instead of the frequency because the caller should not
    /// have to do this conversion again: three places got the same conversion
    /// wrong by assuming a rate instead of asking for one.
    ///
    /// ** ZERO when there is no reference clock -- and zero means *"no spacing
    /// is possible, look every time"*, not *"never"*. A caller that spaces two
    /// `OP_INFO` reads is then paying a few hundred cycles it did not have to;
    /// a caller that never looks again leaves a light lying about the bus. Of
    /// the two ways to be wrong without a clock, that is the cheap one.
    pub fn ciclos_de(&self, ms: u64) -> u64 {
        if self.tsc_hz == 0 || self.tsc_hz == NO_CLOCK {
            return 0;
        }
        (self.tsc_hz / 1_000) * ms
    }
}
