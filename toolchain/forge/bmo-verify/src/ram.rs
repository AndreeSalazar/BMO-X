//! **RAM_VERIFY -- que puede hacer con este fichero el que lo cargue.**
//!
//! Idea del dueno, 2026-08-12: *"eso es para ser RAM_verify, para verificar los
//! archivos que se van a aplicar... porque asi las tablas que pusimos son el
//! motivo para cumplir lo que necesita, NO por condicion"*.
//!
//! La PARTE IX de `docs/LA_RAM.md` dice que herramienta de transporte va en cada
//! sitio. Este modulo la convierte en algo que **se comprueba sobre el fichero**
//! en vez de en un criterio que alguien recuerda.
//!
//! # Que significa "NO por condicion"
//!
//! Que no es un `if` en tiempo de ejecucion que decide y se calla. Es una
//! propiedad del fichero, contestada **antes de escribirlo**, con su motivo
//! cuando la respuesta es que no.
//!
//! La diferencia importa por algo concreto: hoy el cargador ya tiene ramas que
//! caen al camino lento sin decir nada --`disk::tramo_dma` rebota si el destino
//! no esta seguido, `Archivo::leer_de` se cae al camino viejo si el kernel no
//! conoce la op-- y **funcionan**. Pero un sistema que se cae al camino lento en
//! silencio no puede contestar *"por que va lento"*, y esa es la pregunta que
//! este proyecto quiere poder contestar siempre.
//!
//! # [!] ESTO INFORMA, NO RECHAZA. Y es a proposito.
//!
//! Hoy **ningun `.bex` es mapeable**, porque `BefBuilder::build` alinea los
//! `file_offset` a 8 bytes y la congruencia de pagina no se pide a nadie.
//! Rechazar aqui pararia todos los builds por una regla que el escritor todavia
//! no cumple, y una regla que rompe el build antes de que nadie pueda cumplirla
//! se desactiva el mismo dia.
//!
//! Asi que primero **se mide cuanto se esta perdiendo**. El dia que
//! `writer.rs` alinee de verdad, este informe pasa de decir cuanto falta a decir
//! cuanto se gano -- y ahi si puede volverse requisito.

use bmo_abi::bef::header::BefHeader;
use bmo_abi::bef::sections::{SectionEntry, SectionKind};

/// Tamano de pagina. **No se importa del kernel a proposito**: este crate corre
/// en el anfitrion y no puede depender de `Ultra_kernel`. 4096 no se va a mover.
pub const PAGE: u64 = 4096;

/// Como puede viajar una seccion, segun la PARTE IX de `docs/LA_RAM.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transporte {
    /// **Herramienta 1 -- no viaja.** El contenido es deducible (ceros), asi que
    /// no hay nada que transportar. Es la mejor de todas y la primera pregunta.
    NoViaja,
    /// **Herramienta 7 -- se puede MAPEAR del disco.** El fichero puede
    /// entregarse sin leerlo: `file_offset` y `virt_addr` son congruentes modulo
    /// pagina, que es lo unico que hace posible el demand paging.
    Mapeable,
    /// **Herramienta 2/3 -- hay que COPIARLA.** Que no es un fallo: para poco
    /// dato es lo correcto. Lo que si es un fallo es no saber por que.
    Copia,
}

impl Transporte {
    pub fn nombre(&self) -> &'static str {
        match self {
            Transporte::NoViaja => "no viaja",
            Transporte::Mapeable => "mapeable",
            Transporte::Copia => "copia",
        }
    }
}

/// Una seccion, y que se puede hacer con ella.
#[derive(Debug, Clone)]
pub struct Fila {
    /// `SectionKind as u8`, tal cual viene del fichero.
    pub kind: u8,
    pub transporte: Transporte,
    pub file_size: u64,
    pub mem_size: u64,
    /// **Por que no es mapeable**, cuando no lo es. Vacio si lo es o si no
    /// viaja.
    ///
    /// Es la mitad que convierte un informe en una herramienta: un `copia` sin
    /// motivo obliga a ir a mirar el fichero, que es justo el trabajo que este
    /// modulo existe para quitar.
    pub motivo: String,
}

/// Lo que se puede decir de un `.bex` entero sobre como va a viajar.
#[derive(Debug, Clone, Default)]
pub struct InformeRam {
    pub filas: Vec<Fila>,
    /// Bytes que NO viajan porque son deducibles.
    pub no_viajan: u64,
    /// Bytes que se podrian entregar mapeando en vez de leyendo.
    pub mapeables: u64,
    /// Bytes que hay que copiar si o si.
    pub copiados: u64,
}

impl InformeRam {
    /// Bytes que el cargador tendria que leer del disco hoy.
    pub fn se_leen_hoy(&self) -> u64 {
        self.mapeables + self.copiados
    }

    /// **Cuanto se ahorraria el dia que el escritor alinee.** Es el numero que
    /// justifica o entierra el escalon 7, y hasta ahora no lo tenia nadie.
    pub fn ahorro_si_se_mapea(&self) -> u64 {
        self.mapeables
    }
}

/// Lee un `u64` little-endian de `b` en `off`, o 0 si no cabe.
fn le_u64(b: &[u8], off: usize) -> u64 {
    if off + 8 > b.len() {
        return 0;
    }
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap_or([0; 8]))
}

/// **Audita como puede viajar cada seccion de un BEF.**
///
/// No rechaza nada: ver la cabecera del modulo.
pub fn auditar_ram(bef: &[u8]) -> InformeRam {
    let mut inf = InformeRam::default();
    if bef.len() < core::mem::size_of::<BefHeader>() {
        return inf;
    }
    let hdr = unsafe { &*(bef.as_ptr() as *const BefHeader) };
    let sec_off = hdr.section_table_offset as usize;
    let n = hdr.section_count as usize;

    for i in 0..n {
        let e = sec_off + i * SectionEntry::SIZE;
        if e + SectionEntry::SIZE > bef.len() {
            break;
        }
        let kind = bef[e];
        // Solo lo que el cargador MAPEA. Las tablas (relocs, firma, imports) las
        // lee y las tira: no ocupan memoria del proceso y meterlas aqui haria
        // que los totales no cuadraran con lo que el proceso pesa.
        let cargable = kind == SectionKind::Code as u8
            || kind == SectionKind::RoData as u8
            || kind == SectionKind::Data as u8
            || kind == SectionKind::Bss as u8;
        if !cargable {
            continue;
        }

        let file_offset = le_u64(bef, e + 8);
        let file_size = le_u64(bef, e + 16);
        let mem_size = le_u64(bef, e + 24);
        let virt_addr = le_u64(bef, e + 32);

        let (transporte, motivo) = clasificar(kind, file_offset, file_size, virt_addr);

        match transporte {
            Transporte::NoViaja => inf.no_viajan += mem_size,
            Transporte::Mapeable => inf.mapeables += file_size,
            Transporte::Copia => inf.copiados += file_size,
        }
        inf.filas.push(Fila { kind, transporte, file_size, mem_size, motivo });
    }
    inf
}

/// **La decision, aparte y pura.** Es la que se puede leer sin tener delante un
/// fichero, y la que los tests ejercitan directamente.
fn clasificar(kind: u8, file_offset: u64, file_size: u64, virt_addr: u64) -> (Transporte, String) {
    // 1. Lo que no existe no se transporta. Es la herramienta 1 y va primero
    //    siempre: las otras optimizan un transporte que quiza no deberia haber.
    if kind == SectionKind::Bss as u8 || file_size == 0 {
        return (Transporte::NoViaja, String::new());
    }

    // 2. Mapear exige que el byte del fichero y el byte de la memoria caigan en
    //    la MISMA posicion dentro de su pagina. No es un capricho heredado de
    //    ELF: una pagina se mapea entera y desde su principio, asi que si los
    //    dos restos no coinciden **no hay mapeo posible**, por mucho que las dos
    //    direcciones esten alineadas por su cuenta.
    //
    //    Es la regla `p_offset == p_vaddr (mod pagesize)`, que ya esta escrita
    //    en `bef/writer.rs:46` y hoy no la cumple nadie.
    let resto_fichero = file_offset % PAGE;
    let resto_memoria = virt_addr % PAGE;
    if virt_addr == 0 {
        // `virt_addr = 0` significa "elige tu, cargador". Entonces la
        // congruencia no se puede decidir aqui -- y decir "mapeable" seria
        // prometer algo que depende de una decision que aun no se ha tomado.
        return (
            Transporte::Copia,
            String::from("virt_addr = 0: la elige el cargador, asi que la congruencia no se sabe todavia"),
        );
    }
    if resto_fichero != resto_memoria {
        return (
            Transporte::Copia,
            format!(
                "file_offset % 4096 = {} y virt_addr % 4096 = {}: no son congruentes, asi que la pagina no se puede mapear",
                resto_fichero, resto_memoria
            ),
        );
    }
    (Transporte::Mapeable, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: u8 = SectionKind::Code as u8;
    const BSS: u8 = SectionKind::Bss as u8;

    /// ** LA HERRAMIENTA 1 GANA SIEMPRE Y VA PRIMERO.
    ///
    /// Una `Bss` no viaja, y eso no es una optimizacion del transporte: es que
    /// no hay transporte. La PARTE IX lo pone de primera pregunta por esto.
    #[test]
    fn lo_que_no_existe_no_viaja() {
        let (t, m) = clasificar(BSS, 0, 0, 0x400000);
        assert_eq!(t, Transporte::NoViaja);
        assert!(m.is_empty(), "no viajar no necesita excusa");
    }

    /// ** LA CONGRUENCIA NO ES ALINEACION, Y ES EL ERROR FACIL.
    ///
    /// Las dos direcciones pueden estar perfectamente alineadas a 8, a 16 o a lo
    /// que sea, y aun asi ser **imposibles de mapear**: lo que hace falta es que
    /// caigan en la MISMA posicion dentro de su pagina. Una pagina se mapea
    /// entera y desde su principio.
    #[test]
    fn alineado_no_es_lo_mismo_que_congruente() {
        // 0x200 y 0x400000: los dos alineadisimos, restos 512 y 0. No se puede.
        let (t, m) = clasificar(CODE, 0x200, 1000, 0x400000);
        assert_eq!(t, Transporte::Copia);
        assert!(m.contains("512"), "el motivo tiene que decir los dos restos: {m}");
        assert!(m.contains("no son congruentes"));

        // Mismo resto en los dos: mapeable, aunque el offset no sea multiplo de
        // pagina.
        let (t, _) = clasificar(CODE, 0x1200, 1000, 0x401200);
        assert_eq!(t, Transporte::Mapeable);
    }

    /// ** UN `virt_addr` DE 0 NO ES MAPEABLE, Y NO ES LO MISMO QUE UNO MALO.
    ///
    /// Cero significa *"elige tu, cargador"*. Decir "mapeable" seria prometer
    /// algo que depende de una decision que todavia no se ha tomado -- y esa
    /// clase de promesa es como nace un fallo que aparece tres arranques
    /// despues.
    #[test]
    fn una_direccion_sin_decidir_no_se_promete() {
        let (t, m) = clasificar(CODE, 0x1000, 1000, 0);
        assert_eq!(t, Transporte::Copia);
        assert!(m.contains("la elige el cargador"), "y se dice por que: {m}");
    }

    /// ** EL ESTADO DE HOY, FIJADO COMO TEST.
    ///
    /// `BefBuilder::build` alinea los `file_offset` a **8 bytes**, no a pagina.
    /// Asi que un `.bex` real de hoy sale entero en `copia`, y este test existe
    /// para que **el dia que eso cambie, falle** -- que es la unica forma de que
    /// un informe se entere de una mejora.
    #[test]
    fn hoy_ningun_bex_es_mapeable_y_este_test_lo_fija() {
        // Un code en 0x200 (lo que pone el escritor real) contra la base de
        // carga tipica.
        let (t, _) = clasificar(CODE, 0x200, 592_945, 0x400000);
        assert_eq!(
            t,
            Transporte::Copia,
            "si esto pasa a Mapeable es que writer.rs empezo a alinear: borra este test y celebra"
        );
    }

    /// El informe suma por clase, y las tres sumas tienen que cuadrar con lo que
    /// se lee del disco. Un informe cuyos totales no cuadran es peor que
    /// ninguno.
    #[test]
    fn las_tres_sumas_cuadran() {
        let mut inf = InformeRam::default();
        inf.no_viajan = 492_784;
        inf.mapeables = 0;
        inf.copiados = 807_072;
        assert_eq!(inf.se_leen_hoy(), 807_072);
        assert_eq!(inf.ahorro_si_se_mapea(), 0, "hoy no se ahorra nada, y por eso se mide");
    }

    /// Un fichero mas corto que su cabecera no produce un informe inventado.
    #[test]
    fn un_fichero_truncado_no_se_inventa() {
        let inf = auditar_ram(&[0u8; 4]);
        assert!(inf.filas.is_empty());
        assert_eq!(inf.se_leen_hoy(), 0);
    }
}
