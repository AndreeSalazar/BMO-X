//! **EL RITMO** -- dos relojes, contados, y quien de los dos marca el paso.
//!
//! generacion: hijo
//!
//! Es el escalon E0 de `docs/maestro/INTI_Y_LA_GPU.md`, seccion 6: antes de
//! meter una GPU entre dos relojes, **medir los dos relojes que ya hay**.
//!
//! ```text
//!    la app          pinta y sube SECUENCIA cuando el dibujo esta entero
//!    el DIRECTOR     mira la secuencia en cada vuelta, y pega si cambio
//! ```
//!
//! Es el modelo de un juego --la CPU prepara, la GPU presenta-- con la app
//! haciendo de CPU y el DIRECTOR de GPU. Y los fotogramas por segundo son los
//! del MAS LENTO de los dos. Este crate dice cual.
//!
//! ## Lo que se cuenta, y de donde sale cada numero
//!
//! Cada vez que el DIRECTOR MIRA una superficie, le pasa aqui la secuencia que
//! hay. Nada mas: todo lo demas se deduce de la diferencia con la anterior.
//!
//! ```text
//!    diferencia 0     VACIA       el DIRECTOR miro y no habia nada nuevo:
//!                                 estaba esperando a la app
//!    diferencia 1     PUBLICADO   un fotograma nuevo, y se vio
//!    diferencia d>1   PERDIDOS    la app publico d, el DIRECTOR vio el
//!                                 ultimo: d-1 no se ensenaron nunca
//! ```
//!
//! ## ** Lo que NO es un fotograma perdido, y hay que separarlo
//!
//! 1. **Mientras la ventana esta MINIMIZADA** el DIRECTOR no mira. Al volver, la
//!    secuencia ha avanzado cientos: eso no es el DIRECTOR yendo lento, es que
//!    no se estaba viendo. [`Ritmo::se_oculta`] hace que la siguiente mirada solo
//!    tome la referencia.
//! 2. **Un salto absurdo** -- la app reinicio su contador, o escribio basura:
//!    la secuencia la escribe OTRO proceso. Mas de [`SALTO_MAXIMO`] no se cuenta
//!    como perdidos: se cuenta como reinicio, y aparte.
//!
//! Sin esas dos, el veredicto diria "el DIRECTOR va lento" cada vez que alguien
//! minimiza una ventana -- un numero que parece un dato y es un accidente.
//!
//! ## Y lo que este crate NO sabe, a proposito
//!
//! No sabe de tiempo. Cuenta MIRADAS, no milisegundos: el DIRECTOR mira una vez
//! por vuelta, y cuantas vueltas da por segundo lo sabe el, no esto. Asi el
//! veredicto no depende de un reloj que haya que calibrar.

#![cfg_attr(not(test), no_std)]

/// Por encima de esto, un salto de secuencia no son fotogramas perdidos: es un
/// reinicio. Un millon de fotogramas a 60 por segundo son casi cinco horas sin
/// que el DIRECTOR mirara -- eso no es ir lento, es otra cosa.
pub const SALTO_MAXIMO: u32 = 1 << 20;

/// Por debajo de estos fotogramas publicados no hay veredicto: con diez
/// fotogramas, un solo tiron decide el porcentaje.
pub const MUESTRA_MINIMA: u64 = 30;

/// Los dos relojes de UNA superficie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ritmo {
    /// Fotogramas que la app entrego, contados por la secuencia.
    pub publicados: u64,
    /// Fotogramas que el DIRECTOR pego de verdad.
    pub presentados: u64,
    /// Publicados que no llegaron a verse: la app fue mas deprisa que el
    /// DIRECTOR.
    pub perdidos: u64,
    /// Miradas sin nada nuevo: el DIRECTOR fue mas deprisa que la app.
    pub vacias: u64,
    /// Saltos de mas de [`SALTO_MAXIMO`]: contador reiniciado o basura.
    pub reinicios: u64,
    ultima: u32,
    empezado: bool,
    vuelve: bool,
}

/// Quien marca el paso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quien {
    /// Menos de [`MUESTRA_MINIMA`] fotogramas: no se puede decir.
    SinDatos,
    /// El DIRECTOR miro mas veces sin nada nuevo que con algo: espera a la
    /// app. En un juego seria "CPU-bound".
    LaApp,
    /// Mas del 10% de lo publicado no se llego a ensenar: la app produce mas
    /// deprisa de lo que el DIRECTOR compone. Seria "GPU-bound".
    ElDirector,
    /// Ninguno de los dos espera al otro de forma notable.
    Parejos,
}

impl Quien {
    /// Una frase para el informe, en ASCII.
    pub const fn texto(self) -> &'static [u8] {
        match self {
            Quien::SinDatos => b"sin datos todavia",
            Quien::LaApp => b"la APP: el DIRECTOR la espera",
            Quien::ElDirector => b"el DIRECTOR: la app publica mas de lo que se ve",
            Quien::Parejos => b"parejos: ninguno espera al otro",
        }
    }
}

impl Ritmo {
    pub const fn nuevo() -> Self {
        Ritmo {
            publicados: 0,
            presentados: 0,
            perdidos: 0,
            vacias: 0,
            reinicios: 0,
            ultima: 0,
            empezado: false,
            vuelve: false,
        }
    }

    /// **El DIRECTOR miro la superficie y habia esta secuencia.**
    ///
    /// La primera mirada --y la primera despues de [`Self::se_oculta`]-- solo
    /// toma la referencia: no hay diferencia con nada.
    pub fn miro(&mut self, ahora: u32) {
        if !self.empezado || self.vuelve {
            self.ultima = ahora;
            self.empezado = true;
            self.vuelve = false;
            return;
        }
        // `wrapping_sub`: la secuencia es un u32 y DA LA VUELTA. De 0xFFFFFFFF a
        // 0 es un fotograma, no cuatro mil millones.
        let d = ahora.wrapping_sub(self.ultima);
        if d == 0 {
            self.vacias += 1;
            return;
        }
        self.ultima = ahora;
        if d > SALTO_MAXIMO {
            self.reinicios += 1;
            return;
        }
        self.publicados += d as u64;
        self.perdidos += (d - 1) as u64;
    }

    /// El DIRECTOR pego los pixeles de verdad.
    pub fn presento(&mut self) {
        self.presentados += 1;
    }

    /// La ventana dejo de mirarse (minimizada). La siguiente mirada toma la
    /// referencia en vez de contar lo que paso mientras tanto como perdido.
    pub fn se_oculta(&mut self) {
        self.vuelve = true;
    }

    /// **Quien marca el paso**, con los numeros de hasta ahora.
    pub fn quien_marca(&self) -> Quien {
        if self.publicados < MUESTRA_MINIMA {
            return Quien::SinDatos;
        }
        if self.perdidos * 10 > self.publicados {
            return Quien::ElDirector;
        }
        if self.vacias > self.publicados {
            return Quien::LaApp;
        }
        Quien::Parejos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La app publica UNO por cada mirada: nadie espera a nadie.
    #[test]
    fn uno_por_mirada_es_parejo() {
        let mut r = Ritmo::nuevo();
        for s in 0..=100u32 {
            r.miro(s);
        }
        assert_eq!((r.publicados, r.perdidos, r.vacias), (100, 0, 0));
        assert_eq!(r.quien_marca(), Quien::Parejos);
    }

    /// DOOM a 58 fps contra un DIRECTOR a ~250 vueltas: tres miradas vacias
    /// por cada fotograma. El que marca el paso es la app.
    #[test]
    fn una_app_lenta_marca_el_paso() {
        let mut r = Ritmo::nuevo();
        let mut s = 0u32;
        r.miro(s);
        for _ in 0..100 {
            s += 1;
            r.miro(s);
            r.miro(s);
            r.miro(s);
            r.miro(s);
        }
        assert_eq!((r.publicados, r.vacias, r.perdidos), (100, 300, 0));
        assert_eq!(r.quien_marca(), Quien::LaApp);
    }

    /// La app publica tres por cada mirada: dos de cada tres no se ven.
    #[test]
    fn un_director_lento_pierde_fotogramas() {
        let mut r = Ritmo::nuevo();
        let mut s = 0u32;
        r.miro(s);
        for _ in 0..50 {
            s += 3;
            r.miro(s);
        }
        assert_eq!((r.publicados, r.perdidos), (150, 100));
        assert_eq!(r.quien_marca(), Quien::ElDirector);
    }

    /// ** Minimizada no es lenta: lo que paso mientras no se miraba NO se
    /// cuenta como perdido.
    #[test]
    fn minimizar_no_cuenta_como_perder() {
        let mut r = Ritmo::nuevo();
        for s in 0..=40u32 {
            r.miro(s);
        }
        r.se_oculta();
        // Vuelve 5.000 fotogramas despues.
        r.miro(5040);
        r.miro(5041);
        assert_eq!(r.perdidos, 0, "minimizada no pierde fotogramas: no se veia");
        assert_eq!(r.publicados, 41);
    }

    /// La secuencia es un u32 y da la vuelta: eso es UN fotograma.
    #[test]
    fn la_vuelta_del_contador_es_un_fotograma() {
        let mut r = Ritmo::nuevo();
        r.miro(u32::MAX);
        r.miro(0);
        assert_eq!((r.publicados, r.perdidos, r.reinicios), (1, 0, 0));
    }

    /// Un salto absurdo --la app reinicio o escribio basura-- no son millones
    /// de fotogramas perdidos.
    #[test]
    fn un_salto_absurdo_es_un_reinicio_y_no_perdidos() {
        let mut r = Ritmo::nuevo();
        r.miro(10);
        r.miro(10 + SALTO_MAXIMO + 1);
        assert_eq!((r.publicados, r.perdidos, r.reinicios), (0, 0, 1));
    }

    /// La primera mirada solo es la referencia.
    #[test]
    fn la_primera_mirada_no_cuenta() {
        let mut r = Ritmo::nuevo();
        r.miro(777);
        assert_eq!(r, Ritmo { ultima: 777, empezado: true, ..Ritmo::nuevo() });
    }

    /// Con poca muestra no hay veredicto: quince fotogramas, aunque dos de cada
    /// tres se pierdan, no dicen nada.
    ///
    /// ** Y el borde: con 30 justos SI lo hay. La primera version de esta prueba
    /// hacia once miradas de tres en tres --30 publicados-- y pedia `SinDatos`:
    /// la prueba estaba mal, no el umbral.
    #[test]
    fn con_poca_muestra_no_hay_veredicto() {
        let mut r = Ritmo::nuevo();
        for s in 0..=5u32 {
            r.miro(s * 3);
        }
        assert_eq!(r.publicados, 15);
        assert_eq!(r.quien_marca(), Quien::SinDatos);
        for s in 6..=10u32 {
            r.miro(s * 3);
        }
        assert_eq!(r.publicados, MUESTRA_MINIMA);
        assert_eq!(r.quien_marca(), Quien::ElDirector);
    }
}
