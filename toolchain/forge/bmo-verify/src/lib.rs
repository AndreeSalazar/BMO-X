//! `bmo-verify` -- el gate de verificacion (unico checkpoint comun).
//!
//! Reemplaza el rol de seguridad que tendria un IR central, pero como
//! CONTRATO, no como embudo: cada lenguaje emite su BEF por su cuenta y el
//! verificador lo revisa de forma independiente.
//!
//! ```text
//! BEF (de cualquier lenguaje) -> [bmo-verify] -> pasa?
//!                                               si -> admitido al BMO ABI
//!                                               no -> rechazado (con razones)
//! ```
//!
//! **NO es un stub**: delega en el validador estructural REAL de
//! `bmo_abi::bef::validator` (header, tabla de secciones, imports/exports,
//! relocs, firma, flags). Este crate es la CARA de toolchain de ese gate:
//! los frontends llaman `verify()` sin acoplarse a la estructura interna de
//! bmo-abi.
//!
//! **Conexion Singularity**: si el BEF pasa, esta probado seguro -> puede
//! correr como Software Isolated Process (mismo espacio, sin transicion de
//! anillo). La verificacion --no un IR-- habilita el aislamiento barato.

use bmo_abi::bef::validator;

// -- ** RAM_VERIFY: que puede hacer el cargador con este fichero --------------
//
// Fichero aparte porque es otra pregunta. `verify()` contesta *"es admisible?"*;
// esto contesta *"como va a viajar?"*. Comparten el fichero de entrada y nada
// mas -- y la regla de esta casa es que dos preguntas distintas no viven en el
// mismo cajon.
//
// Idea del dueno el 2026-08-12: que las tablas de `docs/LA_RAM.md` dejen de ser
// criterio que alguien recuerda y pasen a ser algo que se comprueba sobre el
// archivo que se va a aplicar.

/// Como puede viajar cada seccion de un BEF. Ver `docs/LA_RAM.md`, PARTE IX.
pub mod ram;


/// Veredicto de la verificacion de un BEF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Paso: admisible al ABI, apto para SIP.
    Ok,
    /// Rechazado, con las razones (mensajes de error del validador).
    Rejected(Vec<String>),
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Ok)
    }
}

/// Verifica un buffer BEF completo. FUNCIONA: corre el validador estructural
/// real y colapsa su resultado a un veredicto binario con razones.
///
/// Las advertencias (secciones inusuales, etc.) no rechazan -- solo los
/// errores (magic malo, secciones fuera de rango, ABI incompatible...).
pub fn verify(bef: &[u8]) -> Verdict {
    // ** LA PUERTA PRIMERO, Y ES LA MISMA QUE CORRE EN RING 0 (2026-08-10).
    //
    // `bmo-bex-gate` es la decision, sin `alloc` y sin dependencias, y la comparte
    // este verificador con el cargador del kernel. Preguntarle aqui **antes** que
    // al validador convierte una promesa en una garantia:
    //
    // > Nada que el kernel vaya a rechazar puede salir de este toolchain.
    //
    // Sin esto, las dos listas de comprobaciones podian separarse y el sintoma
    // seria el peor de todos: un binario que **compila limpio y no carga**, con
    // el compilador diciendo que todo esta bien.
    //
    // El validador se sigue ejecutando detras porque hace mas: avisos que no
    // rechazan, y mensajes con numeros dentro que en Ring 0 no se pueden
    // construir. La puerta decide; el validador explica.
    if let Err(falta) = bmo_bex_gate::revisar(bef, bef.len()) {
        return Verdict::Rejected(vec![String::from(falta.nombre())]);
    }

    let result = validator::validate(bef);
    if result.is_valid {
        Verdict::Ok
    } else {
        let reasons = result
            .issues
            .iter()
            .filter(|i| matches!(i.severity, validator::IssueSeverity::Error))
            .map(|i| i.message.clone())
            .collect();
        Verdict::Rejected(reasons)
    }
}

/// Igual que `verify`, pero devuelve TAMBIEN las advertencias (para
/// herramientas que quieran inspeccionar sin rechazar).
pub fn verify_verbose(bef: &[u8]) -> (Verdict, Vec<String>) {
    let result = validator::validate(bef);
    let warnings = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, validator::IssueSeverity::Warning))
        .map(|i| i.message.clone())
        .collect();
    let verdict = if result.is_valid {
        Verdict::Ok
    } else {
        let reasons = result
            .issues
            .iter()
            .filter(|i| matches!(i.severity, validator::IssueSeverity::Error))
            .map(|i| i.message.clone())
            .collect();
        Verdict::Rejected(reasons)
    };
    (verdict, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmo_abi::bef::writer::{BefBuilder, BefSection};

    fn minimal_valid_bef() -> Vec<u8> {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16])); // ret
        b.add_section(BefSection::rodata(b"ok\0".to_vec()));
        b.build().unwrap()
    }

    #[test]
    fn accepts_a_real_valid_bef() {
        // FUNCIONA de verdad: construye un BEF valido y lo verifica.
        assert_eq!(verify(&minimal_valid_bef()), Verdict::Ok);
    }

    #[test]
    fn rejects_garbage_with_reasons() {
        let v = verify(&[0u8; 48]); // magic malo
        match v {
            Verdict::Rejected(reasons) => assert!(!reasons.is_empty(), "debe dar razones"),
            Verdict::Ok => panic!("basura no debe pasar el gate"),
        }
    }

    #[test]
    fn rejects_too_small() {
        assert!(!verify(&[0u8; 4]).is_ok());
    }

    #[test]
    fn verbose_reports_warnings_without_rejecting() {
        // BEF valido pero con Code duplicada = advertencia, no rechazo.
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::code(vec![0xCC; 16]));
        let bytes = b.build().unwrap();
        let (verdict, warnings) = verify_verbose(&bytes);
        assert_eq!(verdict, Verdict::Ok, "duplicado es warning, no error");
        assert!(warnings.iter().any(|w| w.contains("duplicate")));
    }
}

// =====================================================================
//  LA AUDITORIA DEL DCE -- que no sobre, y que no falte
// =====================================================================
//
// El enlazador tira lo que nadie referencia, y acierta. El problema es que
// **nadie lo comprueba**: se confia, que es otra forma de decir que se reza.
//
// Ya mordio una vez. El 2026-08-09 se borro un `static mut` de 8 MiB del kernel
// esperando recuperar RAM, y el `.bss` no se movio ni un byte: el enlazador ya
// lo habia tirado. La suposicion era "esto ocupa" y el numero dijo que no.
//
// Y la vuelta de esa misma pregunta es la que importa: **si se comio aquello,
// como se sabe que no se comio algo que si hacia falta?**
//
// Un `.bex` puede contestar las dos sin ejecutarse, porque lleva su tabla de
// secciones con tamanos exactos y sus relocations:
//
//   QUE NO FALTE   toda relocation apunta dentro de una seccion que EXISTE.
//                  El cargador de Ring 0 ya lo comprueba -- pero al ARRANCAR,
//                  cuando ya es tarde y el sintoma es una pantalla negra.
//   QUE NO SOBRE   una seccion con bytes y sin una sola referencia entrante es
//                  peso muerto. No es un error: es un numero que hay que mirar.
//
// * Y esto NO es un DCE. Es un AUDITOR de lo que el DCE dejo. La diferencia
// importa: aqui no se borra nada -- se cuenta, y el que decide es una persona
// mirando el numero. Un verificador que ademas modifica es un compilador con
// mala conciencia.

/// Lo que la auditoria encontro. Son numeros, no un veredicto: **sobra** no es
/// un error, es una cifra que alguien tiene que mirar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Auditoria {
    /// Bytes que el `.bex` lleva en secciones con contenido.
    pub bytes_totales: u64,
    /// De esos, los que estan en secciones a las que apunta ALGO.
    pub bytes_alcanzables: u64,
    /// Secciones con bytes y sin una sola referencia entrante.
    pub secciones_huerfanas: Vec<u8>,
    /// Relocations que apuntan a una seccion que este `.bex` no lleva.
    ///
    /// **Esto si es un error**, y del grave: es el caso en el que el DCE se
    /// llevo algo que hacia falta. Da pantalla negra en el arranque y ni una
    /// linea que lo relacione con el build de hace tres dias.
    pub relocs_al_vacio: usize,
    /// Relocations que caen fuera de los limites de su propia seccion.
    pub relocs_desbordadas: usize,
}

impl Auditoria {
    /// Bytes emitidos que nadie alcanza. **No es un error**: es lo que hay que
    /// mirar cuando un `.bex` crece y no se sabe por que.
    pub fn bytes_muertos(&self) -> u64 {
        self.bytes_totales.saturating_sub(self.bytes_alcanzables)
    }

    /// Hay algo que impide cargar esto. Solo lo roto -- lo que sobra no cuenta.
    pub fn hay_rotura(&self) -> bool {
        self.relocs_al_vacio > 0 || self.relocs_desbordadas > 0
    }
}

/// Audita un `.bex` ya escrito. No lo ejecuta y no lo modifica.
///
/// El recorrido es el mismo que hace el cargador de Ring 0 (`task/proc.rs`) y
/// el arnes del banco de C: se leen las secciones por su tabla y las relocs por
/// la suya. Que los tres lean el mismo formato con tres lectores distintos es
/// una debilidad conocida -- por eso `bmo-abi/tests/abi_layout.rs` fija los
/// offsets a mano.
pub fn auditar(bef: &[u8]) -> Auditoria {
    use bmo_abi::bef::header::BefHeader;
    use bmo_abi::bef::relocations::Relocation;
    use bmo_abi::bef::sections::{SectionEntry, SectionKind};

    let mut a = Auditoria::default();
    if bef.len() < core::mem::size_of::<BefHeader>() {
        return a;
    }
    let hdr = unsafe { &*(bef.as_ptr() as *const BefHeader) };
    let sec_off = hdr.section_table_offset as usize;
    let n = hdr.section_count as usize;

    // Codigos de seccion TAL COMO LOS NOMBRAN LAS RELOCS: 0 = code, 1 = data,
    // 2 = rodata.
    //
    // [!] **No son los de `SectionKind`**, que son 1/2/3 -- y rodata coincide
    // en 2 en las dos tablas, asi que cruzarlas acierta en rodata y falla en
    // las otras dos: parece funcionar a medias. Es la trampa que ya esta
    // apuntada en `bef::relocations` y esta funcion la respeta.
    let mut tam = [0u64; 3];
    let mut existe = [false; 3];
    for i in 0..n {
        let e = sec_off + i * SectionEntry::SIZE;
        if e + SectionEntry::SIZE > bef.len() {
            break;
        }
        let cod = match bef[e] {
            x if x == SectionKind::Code as u8 => 0usize,
            x if x == SectionKind::Data as u8 => 1usize,
            x if x == SectionKind::RoData as u8 => 2usize,
            _ => continue,
        };
        let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap_or([0; 8]));
        tam[cod] = size;
        existe[cod] = size > 0;
        a.bytes_totales += size;
    }

    // El CODIGO siempre es alcanzable: es por donde se entra. Sin esta linea,
    // un programa sin una sola reloc --que es lo normal-- saldria entero
    // muerto, y un auditor que grita en el caso comun no lo lee nadie.
    let mut alcanzada = [false, false, false];
    alcanzada[0] = existe[0];

    for i in 0..n {
        let e = sec_off + i * SectionEntry::SIZE;
        if e + SectionEntry::SIZE > bef.len() || bef[e] != SectionKind::Relocs as u8 {
            continue;
        }
        let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap_or([0; 8])) as usize;
        let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap_or([0; 8])) as usize;
        for k in 0..(size / Relocation::SIZE) {
            let r = off + k * Relocation::SIZE;
            if r + Relocation::SIZE > bef.len() {
                break;
            }
            let destino = u32::from_le_bytes(bef[r + 8..r + 12].try_into().unwrap_or([0; 4])) as usize;
            let donde = bef[r + 13] as usize;
            let donde_off = u64::from_le_bytes(bef[r..r + 8].try_into().unwrap_or([0; 8]));

            if destino >= 3 || !existe[destino] {
                a.relocs_al_vacio += 1;
                continue;
            }
            alcanzada[destino] = true;
            // Y que el sitio donde se PARCHEA quepa: ocho bytes escritos justo
            // en el borde de una seccion pisan la siguiente.
            if donde >= 3 || donde_off + 8 > tam[donde] {
                a.relocs_desbordadas += 1;
            }
        }
    }

    for c in 0..3 {
        if existe[c] {
            if alcanzada[c] {
                a.bytes_alcanzables += tam[c];
            } else {
                a.secciones_huerfanas.push(c as u8);
            }
        }
    }
    a
}
