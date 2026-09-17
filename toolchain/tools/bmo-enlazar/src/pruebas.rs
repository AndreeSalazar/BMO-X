//! **El banco del enlazador.** Cada fila compila C de verdad, enlaza y
//! EJECUTA: un enlazador que solo se probara contra objetos escritos a mano
//! estaria comprobando su propia idea del formato.
//!
//! El cargador de aqui es el mismo que el del banco de BMO C -- cada seccion en
//! su pagina, `Bss` a cero y las relocaciones aplicadas -- porque lo que se
//! quiere saber es si el `.bex` corre donde va a correr.

use super::*;
use bmo_abi::bef::sections::SectionEntry;

fn objeto(nombre: &str, fuente: &str) -> (String, Vec<u8>) {
    let bytes = bmo_c_front::compile_source_to_object(fuente)
        .unwrap_or_else(|e| panic!("{nombre} debe compilar: {}", e.message));
    (nombre.to_string(), bytes)
}

/// Carga la imagen como la carga el kernel y la ejecuta.
fn correr(bex: &[u8]) -> String {
    const PAGE: usize = 4096;
    let hdr_entry = u64::from_le_bytes(bex[24..32].try_into().unwrap()) as usize;
    let sec_off = u64::from_le_bytes(bex[32..40].try_into().unwrap()) as usize;
    let cuantas = u32::from_le_bytes(bex[40..44].try_into().unwrap()) as usize;

    let mut imagen = Vec::new();
    let mut base = [usize::MAX; 3];
    for (kind, cod) in [
        (SectionKind::Code, 0usize),
        (SectionKind::RoData, 2usize),
        (SectionKind::Data, 1usize),
        (SectionKind::Bss, usize::MAX),
    ] {
        for i in 0..cuantas {
            let e = sec_off + i * SectionEntry::SIZE;
            if bex[e] != kind as u8 {
                continue;
            }
            let off = u64::from_le_bytes(bex[e + 8..e + 16].try_into().unwrap()) as usize;
            let size = u64::from_le_bytes(bex[e + 16..e + 24].try_into().unwrap()) as usize;
            let mem = u64::from_le_bytes(bex[e + 24..e + 32].try_into().unwrap()) as usize;
            while !imagen.is_empty() && imagen.len() % PAGE != 0 {
                imagen.push(0xCC);
            }
            if cod != usize::MAX {
                base[cod] = imagen.len();
            }
            imagen.extend_from_slice(&bex[off..off + size]);
            imagen.resize(imagen.len() + mem.saturating_sub(size), 0);
        }
    }
    // Las relocaciones, como las aplica el cargador.
    for i in 0..cuantas {
        let e = sec_off + i * SectionEntry::SIZE;
        if bex[e] != SectionKind::Relocs as u8 {
            continue;
        }
        let off = u64::from_le_bytes(bex[e + 8..e + 16].try_into().unwrap()) as usize;
        let size = u64::from_le_bytes(bex[e + 16..e + 24].try_into().unwrap()) as usize;
        for k in 0..size / Relocation::SIZE {
            let r = off + k * Relocation::SIZE;
            let donde = u64::from_le_bytes(bex[r..r + 8].try_into().unwrap()) as usize;
            let destino = u32::from_le_bytes(bex[r + 8..r + 12].try_into().unwrap()) as usize;
            let kind = bex[r + 12];
            let donde_sec = bex[r + 13] as usize;
            let addend = i64::from_le_bytes(bex[r + 16..r + 24].try_into().unwrap());
            assert_eq!(kind, RelocationKind::SeccionAbs64 as u8, "el cargador solo aplica SeccionAbs64");
            let at = base[donde_sec] + donde;
            let valor = (base[destino] as i64 + addend) as u64;
            imagen[at..at + 8].copy_from_slice(&valor.to_le_bytes());
        }
    }

    let mut m = bmo_lower::emu::Machine::new(imagen);
    m.rip = hdr_entry as usize;
    let m = bmo_lower::emu::run(m, 2_000_000);
    assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
    m.console
}

const SUMA: &str = r#"
int suma(int a, int b) { return a + b; }
"#;

const PRINCIPAL: &str = r#"
int suma(int a, int b);
int main() {
    printf("%d", suma(20, 22));
    return 0;
}
"#;

/// *** LA FILA QUE AFIRMA EL PLAN: dos ficheros, un `.bex`, y la llamada de uno
/// al otro llega a su sitio.
#[test]
fn dos_unidades_se_llaman_y_el_programa_corre() {
    let bex = enlazar(&[objeto("principal.bo", PRINCIPAL), objeto("suma.bo", SUMA)])
        .expect("tiene que enlazar");
    assert_eq!(correr(&bex), "42");
}

/// Y el orden en que se dan no cambia lo que hace: la que define va detras o
/// delante, igual.
#[test]
fn el_orden_de_las_unidades_no_cambia_el_resultado() {
    let a = enlazar(&[objeto("suma.bo", SUMA), objeto("principal.bo", PRINCIPAL)]).unwrap();
    assert_eq!(correr(&a), "42");
}

/// Los DATOS tambien cruzan: una cadena y un global de otra unidad, leidos
/// desde codigo que vive en otra pagina. Es lo que rompen las distancias
/// calculadas por el compilador cuando hay mas de una unidad.
#[test]
fn las_cadenas_y_los_globales_cruzan_de_unidad() {
    let datos = r#"
int contador = 40;
const char *nombre = "BMO";
const char *quien(void) { return nombre; }
"#;
    let principal = r#"
extern int contador;
const char *quien(void);
int main() {
    const char *n = quien();
    printf("%s %d", n, contador + 2);
    return 0;
}
"#;
    let bex = enlazar(&[objeto("principal.bo", principal), objeto("datos.bo", datos)]).unwrap();
    assert_eq!(correr(&bex), "BMO 42");
}

/// Un `static` de cada unidad es SUYO: dos ficheros con el mismo nombre privado
/// no se pisan ni chocan.
#[test]
fn dos_statics_con_el_mismo_nombre_no_chocan() {
    let uno = r#"
static int n = 10;
int de_uno(void) { return n; }
"#;
    let dos = r#"
static int n = 32;
int de_dos(void) { return n; }
"#;
    let principal = r#"
int de_uno(void);
int de_dos(void);
int main() { printf("%d", de_uno() + de_dos()); return 0; }
"#;
    let bex = enlazar(&[
        objeto("principal.bo", principal),
        objeto("uno.bo", uno),
        objeto("dos.bo", dos),
    ])
    .unwrap();
    assert_eq!(correr(&bex), "42");
}

/// *** Y LOS TRES "NO", que valen tanto como los "si": cada uno dice el nombre
/// y la unidad, que es lo que hace falta para arreglarlo.
#[test]
fn dice_que_no_con_nombre_y_unidad() {
    let falta = enlazar(&[objeto("principal.bo", PRINCIPAL)]).unwrap_err();
    assert_eq!(
        falta,
        Fallo::NadieLoDefine { nombre: "suma".into(), usado_en: "principal.bo".into() }
    );

    let otra_suma = "int suma(int a, int b) { return a - b; }";
    let dos_veces = enlazar(&[
        objeto("principal.bo", PRINCIPAL),
        objeto("suma.bo", SUMA),
        objeto("otra.bo", otra_suma),
    ])
    .unwrap_err();
    assert!(matches!(dos_veces, Fallo::DefinidoDosVeces { ref nombre, .. } if nombre == "suma"), "{dos_veces:?}");

    let sin_main = enlazar(&[objeto("suma.bo", SUMA)]).unwrap_err();
    assert_eq!(sin_main, Fallo::SinMain);

    assert_eq!(enlazar(&[]).unwrap_err(), Fallo::NadaQueEnlazar);

    let no_es = enlazar(&[("basura.bo".to_string(), b"no soy un objeto".to_vec())]).unwrap_err();
    assert!(matches!(no_es, Fallo::NoEsObjeto { .. }), "{no_es:?}");

    // Y una IMAGEN no es un objeto: pasarle un `.bex` dice por que.
    let imagen = bmo_c_front::compile_source_to_bef("int main(){return 0;}").unwrap();
    let es_imagen = enlazar(&[("programa.bex".to_string(), imagen)]).unwrap_err();
    assert!(matches!(es_imagen, Fallo::NoEsObjeto { .. }), "{es_imagen:?}");
}

/// El mismo mandato, los mismos bytes. Sin esto no hay compilacion
/// reproducible (C7 de PLAN_SEGURIDAD) por mucho que el compilador la tenga.
#[test]
fn enlazar_dos_veces_da_los_mismos_bytes() {
    let u = [objeto("principal.bo", PRINCIPAL), objeto("suma.bo", SUMA)];
    assert_eq!(enlazar(&u).unwrap(), enlazar(&u).unwrap());
}

/// Lo enlazado pasa el MISMO gate que el kernel le pondria. Va aparte porque
/// `enlazar` ya lo comprueba: esta fila existe para que si alguien quita esa
/// llamada, algo se ponga rojo.
#[test]
fn lo_enlazado_pasa_el_gate_del_kernel() {
    let bex = enlazar(&[objeto("principal.bo", PRINCIPAL), objeto("suma.bo", SUMA)]).unwrap();
    assert!(bmo_verify::verify(&bex).is_ok());
    assert!(bmo_abi::bef::objeto::read(&bex).is_err(), "un programa ya NO es un objeto");
}
