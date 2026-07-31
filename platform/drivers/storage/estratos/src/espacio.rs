//! **Cuánto queda** — la contabilidad del espacio, y los avisos de la §9.
//!
//! ═══ Por qué esto va antes que escribir ═══
//!
//! Un sistema de ficheros que no sabe cuánto le queda no puede decidir si
//! acepta una escritura. Y uno que **no sobreescribe nunca** —que es la idea
//! entera de ESTRATOS— se llena aunque nadie cree un archivo: cada versión se
//! queda. Saber la ocupación no es un adorno del panel: es la condición previa
//! al paso 5 del diseño.
//!
//! ═══ La cuenta es trivial, y eso es una decisión ═══
//!
//! ESTRATOS reserva con un **puntero que sólo avanza**: `log_head` es el primer
//! bloque libre, y todo lo de debajo está usado. No hay mapa de bits, no hay
//! listas de huecos, no hay fragmentación que medir. La ocupación es una resta.
//!
//! Eso es posible *porque* nada se sobreescribe. El precio es que el espacio no
//! vuelve solo: quien lo devuelve es el recolector (§9), y hasta que exista,
//! esta cuenta sólo sube. Que suba y se vea es exactamente lo que se quiere —
//! un FS que se llena por sorpresa es el que pierde datos.
//!
//! ═══ Sobre el recolector, dicho aquí porque es donde se nota ═══
//!
//! La observación de Eddi es correcta y ya estaba en el diseño: **para un disco
//! personal grande, el GC importa poco**. 414 GiB son ~108 millones de bloques;
//! un `.bex` ocupa cinco. Se pueden hacer *millones* de estratos antes de que
//! haga falta soltar uno.
//!
//! Y para quien acumula a propósito —una empresa que quiere poder retroceder—
//! el historial **es el producto**, no la basura. Es la misma postura de Git:
//! `git gc` no borra tu historia, sólo lo que ya nadie alcanza, y el reflog
//! guarda 90 días por si acaso. Por eso aquí el recolector será **explícito y
//! con la lista delante**, nunca un demonio que decide solo.
//!
//! Lo que sí hace falta desde el primer día es **el aviso**. Eso es esto.

/// Los cuatro estados del volumen, y qué significa cada uno.
///
/// Los umbrales están en el diseño (§9) y no se eligen aquí: se copian. Cambiar
/// uno es cambiar el documento primero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    /// Por debajo del 70 %. No hay nada que decir.
    Holgado,
    /// 70 % — ámbar. Se avisa con cuánto ocupa el historial.
    Ambar,
    /// 85 % — FAULT rojo, y con una propuesta concreta de qué soltar.
    Rojo,
    /// 95 % — **solo lectura**. Antes de perder datos por falta de sitio, el
    /// sistema se planta y lo dice. No es una degradación: es la respuesta
    /// correcta, porque un FS transaccional necesita sitio para *escribir* la
    /// transacción que libera sitio.
    SoloLectura,
}

impl Nivel {
    /// Para el panel: qué tan grave, en una palabra.
    pub fn nombre(self) -> &'static str {
        match self {
            Nivel::Holgado => "holgado",
            Nivel::Ambar => "AVISO: por encima del 70%",
            Nivel::Rojo => "FAULT: por encima del 85%",
            Nivel::SoloLectura => "SOLO LECTURA: por encima del 95%",
        }
    }

    /// ¿Se puede escribir en este estado?
    ///
    /// Es la pregunta que hará el paso 5, y vive aquí para que la respuesta sea
    /// **una** y no una comparación repetida en cada sitio que escribe.
    pub fn admite_escritura(self) -> bool {
        !matches!(self, Nivel::SoloLectura)
    }
}

/// Umbrales en tantos por mil, para no usar coma flotante en Ring 0.
const AMBAR: u32 = 700;
const ROJO: u32 = 850;
const SOLO_LECTURA: u32 = 950;

/// La foto del espacio del volumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ocupacion {
    /// Bloques por debajo de `log_head`: todo lo escrito alguna vez.
    pub usados: u64,
    /// Los que tiene el volumen entero.
    pub totales: u64,
    pub bloque: u32,
}

impl Ocupacion {
    /// La cuenta, a partir de lo que dice el superbloque.
    ///
    /// `log_head` es el primer bloque LIBRE, así que también es cuántos hay
    /// usados — los bloques se numeran desde cero. Un volumen recién formateado
    /// tiene `log_head = 2`: los dos superbloques.
    pub fn de(log_head: u64, total_blocks: u64, bloque: u32) -> Self {
        Self {
            // Un `log_head` mayor que el total es un superbloque corrupto, y se
            // trata como "lleno" en vez de dar la vuelta a la resta. Un
            // underflow aquí diría "quedan 18 trillones de bloques libres".
            usados: log_head.min(total_blocks),
            totales: total_blocks,
            bloque,
        }
    }

    pub fn libres(&self) -> u64 {
        self.totales - self.usados
    }

    /// Ocupación en tantos por mil. Sin flotantes y sin desbordar: con 108
    /// millones de bloques, `usados * 1000` cabe de sobra en 64 bits.
    pub fn por_mil(&self) -> u32 {
        if self.totales == 0 {
            return 0;
        }
        ((self.usados.saturating_mul(1000)) / self.totales) as u32
    }

    /// Ocupación en enteros, para el panel.
    pub fn por_ciento(&self) -> u32 {
        self.por_mil() / 10
    }

    pub fn bytes_usados(&self) -> u64 {
        self.usados.saturating_mul(self.bloque as u64)
    }

    pub fn bytes_libres(&self) -> u64 {
        self.libres().saturating_mul(self.bloque as u64)
    }

    pub fn nivel(&self) -> Nivel {
        let p = self.por_mil();
        if p >= SOLO_LECTURA {
            Nivel::SoloLectura
        } else if p >= ROJO {
            Nivel::Rojo
        } else if p >= AMBAR {
            Nivel::Ambar
        } else {
            Nivel::Holgado
        }
    }

    /// Cuántos bloques caben todavía de un objeto de `bytes`.
    ///
    /// Para contestar "¿cuántos estratos más caben?" sin que quien pregunta
    /// tenga que saber el tamaño de bloque. Redondea hacia arriba: medio bloque
    /// ocupa un bloque.
    pub fn caben_de(&self, bytes: u64) -> u64 {
        if bytes == 0 {
            return u64::MAX;
        }
        let por_objeto = bytes.div_ceil(self.bloque as u64).max(1);
        self.libres() / por_objeto
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El disco real de Eddi: BMO-DATA, 414 GiB en bloques de 4 KiB.
    const BLOQUES_BMO_DATA: u64 = 108_670_208;

    #[test]
    fn un_volumen_recien_formateado_esta_practicamente_vacio() {
        let o = Ocupacion::de(2, BLOQUES_BMO_DATA, 4096);
        assert_eq!(o.por_ciento(), 0);
        assert_eq!(o.nivel(), Nivel::Holgado);
        assert!(o.nivel().admite_escritura());
    }

    /// ★ La respuesta concreta a "¿cuándo hará falta el recolector?".
    ///
    /// Un `.bex` de C ocupa ~20 KiB = 5 bloques. Aunque cada estrato guardara
    /// uno entero sin compartir nada, caben más de VEINTE MILLONES antes de
    /// llegar al 70 %. Por eso el GC es "algún día" y no "antes de escribir".
    #[test]
    fn en_414_gib_caben_millones_de_estratos() {
        let o = Ocupacion::de(2, BLOQUES_BMO_DATA, 4096);
        let bex = 20 * 1024;
        assert!(
            o.caben_de(bex) > 20_000_000,
            "sólo caben {} de {} bytes",
            o.caben_de(bex),
            bex
        );
    }

    #[test]
    fn los_umbrales_caen_donde_dice_el_diseno() {
        let en = |porcentaje: u64| Ocupacion::de(porcentaje * 10, 1000, 4096).nivel();
        assert_eq!(en(69), Nivel::Holgado);
        assert_eq!(en(70), Nivel::Ambar);
        assert_eq!(en(84), Nivel::Ambar);
        assert_eq!(en(85), Nivel::Rojo);
        assert_eq!(en(94), Nivel::Rojo);
        assert_eq!(en(95), Nivel::SoloLectura);
    }

    /// ★ Al 95 % se deja de escribir, y no es una degradación: un FS
    /// transaccional necesita sitio **para escribir la transacción que libera
    /// sitio**. Llegar a cero sin margen es no poder ni recoger basura.
    #[test]
    fn al_95_por_ciento_ya_no_se_admite_escritura() {
        assert!(!Ocupacion::de(950, 1000, 4096).nivel().admite_escritura());
        assert!(Ocupacion::de(949, 1000, 4096).nivel().admite_escritura());
    }

    /// Un superbloque que dice que la cabeza del log está más allá del final
    /// del volumen está corrupto. Se trata como lleno.
    ///
    /// Sin el `min`, la resta da la vuelta y `libres()` anuncia dieciocho
    /// trillones de bloques libres — y el que pregunte se lo cree.
    #[test]
    fn una_cabeza_de_log_imposible_no_da_la_vuelta_a_la_resta() {
        let o = Ocupacion::de(u64::MAX, 1000, 4096);
        assert_eq!(o.libres(), 0);
        assert_eq!(o.por_ciento(), 100);
        assert!(!o.nivel().admite_escritura());
    }

    #[test]
    fn los_bytes_salen_de_multiplicar_por_el_bloque() {
        let o = Ocupacion::de(256, 1024, 4096);
        assert_eq!(o.bytes_usados(), 1024 * 1024);
        assert_eq!(o.bytes_libres(), 3 * 1024 * 1024);
        assert_eq!(o.por_ciento(), 25);
    }
}
