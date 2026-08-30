//! **EL JUEZ DEL PHYSMAP** -- se puede caminar por esta direccion fisica?
//!
//! generacion: nieto
//!
//! [cuesta]  MAQUINA -- por herencia, igual que `bmo-mmio-juicio`. No toca
//!           hardware ni tiene un solo `unsafe`, pero `mm::vmm` y `mm::phys`
//!           DECIDEN con su respuesta si dereferencian. Instrumentar no
//!           contagia el coste; decidir si (L6e).
//!
//! [riesgo]  ESPEJO -- el numero que juzga lo juzgan DOS sitios del kernel, y
//!           el 2026-08-30 no coincidian. Este crate existe para que solo haya
//!           una respuesta y se pueda probar (L6f).
//!
//! # *** POR QUE ESTO ES UN JUEZ APARTE, Y LA FECHA EN QUE SE COBRO
//!
//! El 2026-08-30 la maquina se paro dos veces por la misma pregunta mal
//! contestada:
//!
//! ```text
//!    vec=0x0E  err=0x00000000   no-presente  leyendo  desde el KERNEL
//!    cr2=0xFFFFBD352B3AC000     -> fisica 0x3D352B3AC000 = 61,2 TiB
//! ```
//!
//! El physmap espeja **16 GiB**. El juez que decidia si esa fisica se podia
//! tocar comparaba contra `1 << 46` --64 TiB-- porque el techo estaba escrito a
//! mano en el sitio equivocado. Y su gemelo, `phys::free_frame`, comparaba
//! contra el bueno. **Dos jueces del mismo numero con dos techos, y el flojo
//! era el que dereferencia.**
//!
//! # *** EL TECHO NO ES SUYO, Y ESA ES LA IDEA ENTERA
//!
//! Esta caja **no tiene ni una constante de tamano**. El espejo se le pasa en
//! cada llamada, asi que:
//!
//! > **no puede tener un numero suelto, porque no tiene ningun numero.**
//!
//! Es la regla 3 de L6g --*"ningun numero suelto: todo tope sale de la
//! constante que lo define"*-- cumplida por construccion en vez de por
//! revision. Un juez que no puede inventarse el techo no puede equivocarse en
//! el techo; lo unico que puede hacer mal es la comparacion, y eso si se prueba
//! aqui abajo.
//!
//! # Lo que NO decide
//!
//! Si la pagina esta mapeada, si es de alguien, o si conviene tocarla. Solo
//! contesta si **cae dentro de lo que el espejo alcanza**. Confundir eso con
//! "es segura" seria darle un permiso que no da.

#![cfg_attr(not(test), no_std)]

/// **Por que NO se puede caminar por una fisica**, o que si.
///
/// Son tres respuestas y no un `bool` a proposito: las dos formas de fallar
/// mandan a sitios distintos, y un `false` pelado las junta.
///
/// ```text
///    Cero            una entrada de tabla vacia leida como si fuera un marco
///    FueraDelEspejo  un numero que el espejo no alcanza -- basura, o un
///                    espejo que se quedo corto
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    /// Cae dentro del espejo. Se puede convertir a virtual y tocar.
    Si,
    /// Es cero. **No es "fuera de rango": es una entrada vacia**, y eso apunta
    /// a quien la leyo, no a quien la escribio.
    Cero,
    /// Mas alla de lo que el espejo alcanza.
    FueraDelEspejo,
}

impl Veredicto {
    /// Para el sitio que solo quiere seguir o no seguir.
    pub const fn se_puede(self) -> bool {
        matches!(self, Veredicto::Si)
    }
}

/// **Se puede caminar por `fisica`, con un espejo de `espejo_bytes`?**
///
/// `espejo_bytes` es lo que el physmap refleja de verdad -- en BMO-X,
/// `mm::PHYSMAP_SIZE`. **Se pasa, no se sabe**: ver la cabecera.
///
/// [!] La frontera es EXCLUSIVA: una fisica igual al tamano del espejo es el
/// primer byte que ya no esta reflejado. Se prueba abajo, porque un `<=` aqui
/// es un fallo de un solo byte que no da la cara hasta que la RAM llega justo
/// al limite.
pub const fn se_puede_caminar(fisica: u64, espejo_bytes: u64) -> Veredicto {
    if fisica == 0 {
        return Veredicto::Cero;
    }
    if fisica >= espejo_bytes {
        return Veredicto::FueraDelEspejo;
    }
    Veredicto::Si
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que el physmap de esta maquina espeja hoy: 16 GiB. Vive en
    /// `mm/mod.rs` como `PHYSMAP_SIZE`; aqui es solo el valor con el que se
    /// prueba, y por eso puede estar escrito.
    const ESPEJO: u64 = 0x4_0000_0000;

    /// *** LA REGRESION DEL 2026-08-30, CON EL NUMERO EXACTO DE LA PANTALLA.
    ///
    /// `cr2 = 0xFFFFBD352B3AC000` menos `HIGH_MEM_BASE` da esta fisica. Si esta
    /// prueba pasa a fallar, la maquina vuelve a pararse abriendo DOOM.
    #[test]
    fn los_61_tib_del_ryzen_no_se_caminan() {
        assert_eq!(
            se_puede_caminar(0x3D35_2B3A_C000, ESPEJO),
            Veredicto::FueraDelEspejo
        );
    }

    /// *** Y LA PRUEBA QUE GUARDA LA LECCION ENTERA: con el techo VIEJO, esa
    /// misma direccion pasaba.
    ///
    /// No es una curiosidad. Es la demostracion de que **el techo es el
    /// veredicto**: la comparacion siempre estuvo bien y el numero estuvo mal.
    /// Quien vuelva a escribir un techo a mano tiene esta linea delante.
    #[test]
    fn con_el_techo_viejo_esa_misma_direccion_pasaba() {
        let techo_viejo = 1u64 << 46; // lo que decia `FISICA_MAX` hasta el 30-08
        assert_eq!(se_puede_caminar(0x3D35_2B3A_C000, techo_viejo), Veredicto::Si);
    }

    /// El cero es su propia respuesta, y no "fuera de rango".
    #[test]
    fn el_cero_no_es_lo_mismo_que_pasarse() {
        assert_eq!(se_puede_caminar(0, ESPEJO), Veredicto::Cero);
        assert_ne!(se_puede_caminar(0, ESPEJO), Veredicto::FueraDelEspejo);
    }

    /// La frontera, por los dos lados. Un `<=` en vez de un `<` se cuela por
    /// aqui y no da la cara hasta que la RAM llega justo al limite.
    #[test]
    fn la_frontera_es_exclusiva() {
        assert_eq!(se_puede_caminar(ESPEJO - 1, ESPEJO), Veredicto::Si);
        assert_eq!(se_puede_caminar(ESPEJO, ESPEJO), Veredicto::FueraDelEspejo);
    }

    /// Lo normal: un marco de los primeros y uno de los ultimos que existen.
    #[test]
    fn lo_que_si_se_camina() {
        assert!(se_puede_caminar(0x1000, ESPEJO).se_puede());
        assert!(se_puede_caminar(15 * (1 << 30), ESPEJO).se_puede());
    }

    /// ** Un espejo de cero no deja pasar NADA, y eso es lo correcto: si
    /// alguien pregunta con el tamano sin inicializar, la respuesta segura es
    /// que no. Un juez que en la duda dice que si no es un juez.
    #[test]
    fn sin_espejo_no_se_camina_por_ningun_sitio() {
        assert_eq!(se_puede_caminar(0x1000, 0), Veredicto::FueraDelEspejo);
    }
}
