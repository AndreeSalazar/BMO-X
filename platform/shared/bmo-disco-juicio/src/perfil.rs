//! **EL PERFIL** -- lo que el disco NO puede contestar, declarado a mano.
//!
//! [exige]   R-DISCO8 (lo que decide el diseno es lo que el disco calla),
//!           R-DISCO9 (una cifra de catalogo no es una medida),
//!           R-CPU8 (un presupuesto tiene dueno), R-CPU9 (los dos lados)
//!
//! # ** DONDE CAE LA RAYA, Y POR QUE NO ES UNA PREFERENCIA
//!
//! ```text
//!    se PREGUNTA al aparato    rotacional, TRIM, sector fisico, cola,
//!                              generacion SATA, capacidad, identidad
//!    se DECLARA aqui           bloque de borrado, TBW, DRAM, condensadores,
//!                              sostenido real
//! ```
//!
//! **La raya es exactamente lo que el aparato responde y lo que no.** Un perfil
//! que declarara la capacidad seria un perfil capaz de mentir sobre algo
//! comprobable; uno que declara el bloque de borrado dice **lo unico que nadie
//! mas puede decir** -- ningun SSD de consumo lo expone, en ninguna palabra del
//! IDENTIFY, y es el numero que decide si escribir 4 KB cuesta 4 KB o 2 MB.
//!
//! Por eso este fichero no repite nada de `bmo-identify`. Si algun dia un campo
//! de aqui se puede preguntar, **se borra de aqui**: un dato duplicado en las
//! dos columnas es un dato que puede discrepar consigo mismo.
//!
//! # La doctrina se hereda entera del perfil de CPU
//!
//! `cpu_vendor/` declara la EXPECTATIVA y el HECHO se le pregunta al silicio. Si
//! no coinciden, las filas contestan `sin declarar` y **el juez se calla**. Aqui
//! igual: el perfil trae la identidad con la que se midio, y si el disco de
//! delante no es ese, sus numeros no valen para el.
//!
//! Estrenar un disco = copiar el perfil, arrancar (dira `SIN PERFIL`, que es lo
//! correcto), correr la sonda y pegar las cifras. **Cero lineas de kernel.**

/// De donde sale un numero. Es L2 de la ley, aplicada campo a campo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origen {
    /// Medido en ESTA maquina, con su ventana declarada.
    Medido,
    /// De la ficha del fabricante. **No es una medida** (R-DISCO9).
    Catalogo,
    /// Deducido de un experimento indirecto -- el bloque de borrado se caza por
    /// su sombra, porque no se puede preguntar.
    Deducido,
}

/// Un numero con su procedencia pegada. Nunca viajan separados.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cifra {
    pub valor: u64,
    pub origen: Origen,
}

impl Cifra {
    pub const fn catalogo(valor: u64) -> Cifra {
        Cifra { valor, origen: Origen::Catalogo }
    }
    pub const fn medido(valor: u64) -> Cifra {
        Cifra { valor, origen: Origen::Medido }
    }
    pub const fn deducido(valor: u64) -> Cifra {
        Cifra { valor, origen: Origen::Deducido }
    }
    /// Se puede sostener una decision sobre esta cifra sin decir que es de
    /// catalogo? R-DISCO9: no, y por eso la pregunta existe.
    pub fn es_medida(self) -> bool {
        !matches!(self.origen, Origen::Catalogo)
    }
}

/// La identidad con la que se midio un perfil.
///
/// Es lo que hace que el perfil pertenezca a UN disco (R-CPU8). El modelo basta
/// para la familia; la capacidad desempata entre tamanos del mismo modelo, que
/// tienen bloque de borrado y TBW distintos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Identidad {
    pub modelo: &'static str,
    /// Sectores logicos. Se compara con tolerancia: ver `coincide`.
    pub sectores: u64,
}

impl Identidad {
    /// Un 1% de tolerancia en la capacidad.
    ///
    /// No es laxitud: entre dos unidades del mismo modelo la cuenta exacta puede
    /// variar por el sobreaprovisionamiento, y entre dos capacidades distintas
    /// del catalogo hay saltos del 100%. **No hay zona gris.** Es el mismo
    /// razonamiento que el 1% del TSC en el perfil de CPU.
    pub fn coincide(&self, modelo: &str, sectores: u64) -> bool {
        if !modelo.contains(self.modelo) {
            return false;
        }
        if self.sectores == 0 || sectores == 0 {
            return false;
        }
        let (a, b) = (self.sectores, sectores);
        let dif = if a > b { a - b } else { b - a };
        dif * 100 <= a
    }
}

/// Lo que hay que saber de un disco y el disco no dice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Perfil {
    pub identidad: Identidad,

    /// Bytes que hay que borrar para poder reescribir uno.
    ///
    /// ** El numero mas importante del perfil y el que ningun disco expone.**
    /// Si el frente del log de ESTRATOS cae en su frontera, la amplificacion de
    /// escritura tiende a 1; si no, cada avance puede tocar dos bloques.
    pub bloque_de_borrado: Cifra,

    /// Terabytes que el fabricante garantiza que se pueden escribir.
    pub tbw: Cifra,

    /// Tiene cache DRAM propia para la tabla del FTL?
    ///
    /// Sin ella, una lectura aleatoria fria puede costar **dos** accesos: uno
    /// para el mapa y otro para el dato. Es el patron del descenso por el arbol
    /// de ESTRATOS, que es punteros persiguiendo punteros.
    pub tiene_dram: bool,

    /// Puede terminar lo que empezo si se va la luz?
    ///
    /// ** Si es `false`, el `FLUSH CACHE` es lo UNICO que separa una transaccion
    /// de la corrupcion.** El disco confirma la escritura cuando la tiene en
    /// cache volatil, no cuando esta en la NAND.
    pub condensadores: bool,

    /// Escritura secuencial sostenida, en MB/s, **despues** de agotar la cache
    /// SLC. No es la cifra de la caja (R-DISCO9).
    pub sostenido_mb_s: Cifra,
}

/// # El Kingston de esta maquina
///
/// El unico perfil que hay, igual que `cpu_vendor/` empezo con un solo CPU. Para
/// anadir otro: copiar este bloque, cambiar la identidad y pegar las cifras de
/// su sonda. **Nada mas del arbol se toca.**
///
/// [!] **Cuatro de las cinco cifras son `Catalogo`**, o sea que hoy este perfil
/// no puede sostener ninguna decision sin decir de donde salen sus numeros. La
/// sonda que las convierte esta en la section 8 de
/// `docs/componente/EL_DISCO_EXIGE.md`.
pub const KINGSTON_A400_480: Perfil = Perfil {
    identidad: Identidad {
        modelo: "SA400S37480G",
        // 447 GiB, leidos del IDENTIFY de esta maquina.
        sectores: 937_703_088,
    },
    // [!] Suposicion de familia, NO medido y NO de catalogo: Kingston no publica
    // este dato. Se pone el valor tipico de la NAND TLC de su generacion para
    // que exista un numero con el que alinear, y se marca `Deducido` para que
    // nadie lo cite como hecho. **La sonda de paso creciente lo sustituye.**
    bloque_de_borrado: Cifra::deducido(2 * 1024 * 1024),
    tbw: Cifra::catalogo(160),
    tiene_dram: false,
    condensadores: false,
    sostenido_mb_s: Cifra::catalogo(450),
};

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_identidad_reconoce_su_disco() {
        let i = KINGSTON_A400_480.identidad;
        assert!(i.coincide("KINGSTON SA400S37480G", 937_703_088));
    }

    #[test]
    fn otro_modelo_no_es_este_perfil() {
        let i = KINGSTON_A400_480.identidad;
        assert!(!i.coincide("Samsung SSD 870 EVO 500GB", 976_773_168));
    }

    /// ** La que importa: el mismo modelo en otra capacidad NO comparte perfil.
    /// El bloque de borrado y el TBW cambian con el tamano.
    #[test]
    fn el_mismo_modelo_con_otra_capacidad_no_coincide() {
        let i = KINGSTON_A400_480.identidad;
        assert!(!i.coincide("KINGSTON SA400S37480G", 234_441_648), "120 GB");
    }

    #[test]
    fn una_capacidad_a_cero_nunca_coincide() {
        let i = KINGSTON_A400_480.identidad;
        assert!(!i.coincide("KINGSTON SA400S37480G", 0));
    }

    /// El 1% absorbe la variacion entre unidades y no llega a otro tamano.
    #[test]
    fn la_tolerancia_es_del_uno_por_ciento() {
        let i = Identidad { modelo: "X", sectores: 1_000_000 };
        assert!(i.coincide("X", 1_009_000), "0,9% dentro");
        assert!(!i.coincide("X", 1_020_000), "2% fuera");
    }

    /// R-DISCO9 con dientes: una cifra de catalogo no puede pasar por medida.
    #[test]
    fn el_catalogo_no_es_una_medida() {
        assert!(!KINGSTON_A400_480.tbw.es_medida());
        assert!(!KINGSTON_A400_480.sostenido_mb_s.es_medida());
        assert!(Cifra::medido(1).es_medida());
        assert!(Cifra::deducido(1).es_medida(), "deducir es un experimento");
    }
}
