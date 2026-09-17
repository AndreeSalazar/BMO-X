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

// -- E5: la libc, una vez -------------------------------------------------

fn objeto_con(nombre: &str, fuente: &str, libc: bmo_c_front::Libc) -> (String, Vec<u8>) {
    let bytes = bmo_c_front::compile_object_with_preprocessor(
        fuente,
        std::path::Path::new("prueba.c"),
        bmo_c_front::CStandard::C11,
        libc,
    )
    .unwrap_or_else(|e| panic!("{nombre} debe compilar: {}", e.message));
    (nombre.to_string(), bytes)
}

/// El programa usa `strncpy`, que vive en `<string.h>`. Con la libc APARTE su
/// cuerpo no viaja en la unidad: lo pone `libc.bo`.
const USA_LIBC: &str = r#"
#include <string.h>
int main() {
    char destino[8];
    strncpy(destino, "BMO-X", 6);
    printf("%s", destino);
    return 0;
}
"#;

/// *** E5: la misma respuesta por los dos caminos. Si la libc enlazada hiciera
/// algo distinto de la copiada, el que cambia de camino se lo encontraria en
/// ejecucion y no al compilar.
#[test]
fn la_libc_aparte_hace_lo_mismo_que_la_copiada() {
    let copia = enlazar(&[objeto_con("solo.bo", USA_LIBC, bmo_c_front::Libc::Copia)]).unwrap();
    assert_eq!(correr(&copia), "BMO-X");

    let libc = (
        "libc.bo".to_string(),
        bmo_c_front::compile_libc_object(bmo_c_front::CStandard::C11).expect("la libc debe compilar"),
    );
    let aparte = enlazar(&[
        objeto_con("principal.bo", USA_LIBC, bmo_c_front::Libc::Aparte),
        libc,
    ])
    .unwrap();
    assert_eq!(correr(&aparte), "BMO-X");
}

/// *** E5b HECHO, Y ESTE NUMERO SE DIO LA VUELTA.
///
/// Hasta el 2026-09-17 esta fila EXIGIA que enlazar contra la libc entera
/// saliera mas grande que llevarse la copia, y existia para ponerse roja el dia
/// que el enlazador aprendiera a tirar lo que nadie llama. Ese dia llego, asi
/// que ahora afirma lo contrario. Las cuatro esquinas, medidas:
///
/// ```text
///                           sin poda    con poda
///    la copia privada        5.100 B     2.044 B   (-60,0 %)
///    la libc APARTE         28.197 B     2.132 B   (-92,4 %)
/// ```
///
/// Los 88 bytes que la libc enlazada sigue costando de mas no son cuerpos: son
/// su `rodata`, que no se poda (ver `tirar.rs`).
#[test]
fn enlazar_contra_la_libc_entera_ya_no_cuesta_mas() {
    let copia = enlazar(&[objeto_con("solo.bo", USA_LIBC, bmo_c_front::Libc::Copia)]).unwrap();
    let libc = (
        "libc.bo".to_string(),
        bmo_c_front::compile_libc_object(bmo_c_front::CStandard::C11).unwrap(),
    );
    let aparte = enlazar(&[
        objeto_con("principal.bo", USA_LIBC, bmo_c_front::Libc::Aparte),
        libc,
    ])
    .unwrap();
    assert!(
        aparte.len() <= copia.len() + copia.len() / 10,
        "enlazar contra la libc entera no puede costar mas que copiarsela:          copiada {} B, enlazada {} B",
        copia.len(),
        aparte.len()
    );
}

/// Dos unidades que usan la MISMA funcion de cabecera no chocan: cada una se
/// quedo su copia privada (E2b). Sin eso, el enlazador tendria que elegir
/// entre dos `strncpy` identicos, y elegir sin motivo es adivinar.
#[test]
fn dos_unidades_que_incluyen_la_misma_cabecera_no_chocan() {
    let una = r#"
#include <string.h>
int copia_una(char *d, const char *s) { strncpy(d, s, 4); return 1; }
"#;
    let dos = r#"
#include <string.h>
int copia_dos(char *d, const char *s) { strncpy(d, s, 4); return 2; }
"#;
    let principal = r#"
int copia_una(char *d, const char *s);
int copia_dos(char *d, const char *s);
int main() {
    char a[8];
    char b[8];
    int n = copia_una(a, "uno") + copia_dos(b, "dos");
    printf("%s %s %d", a, b, n);
    return 0;
}
"#;
    let bex = enlazar(&[
        objeto_con("principal.bo", principal, bmo_c_front::Libc::Copia),
        objeto_con("una.bo", una, bmo_c_front::Libc::Copia),
        objeto_con("dos.bo", dos, bmo_c_front::Libc::Copia),
    ])
    .unwrap();
    assert_eq!(correr(&bex), "uno dos 3");
}

/// *** LO QUE COMPRA E5b, con nombres y no solo con bytes.
///
/// Las dos mitades se comprueban JUNTAS a proposito: un podador que tire lo que
/// nadie llama pero se lleve por delante algo que si se usa no es medio bueno,
/// es una trampa -- el programa enlaza, pasa el gate y salta a donde ya no hay
/// nadie. Por eso aqui se exige tambien que la salida sea la MISMA que sin
/// podar: tirar codigo muerto no puede cambiar lo que un programa hace.
#[test]
fn la_poda_tira_lo_que_nadie_llama_y_deja_lo_que_si() {
    let libc = || (
        "libc.bo".to_string(),
        bmo_c_front::compile_libc_object(bmo_c_front::CStandard::C11).unwrap(),
    );
    let con = enlazar_informado(
        &[objeto_con("principal.bo", USA_LIBC, bmo_c_front::Libc::Aparte), libc()],
        true,
    )
    .unwrap();
    let sin = enlazar_informado(
        &[objeto_con("principal.bo", USA_LIBC, bmo_c_front::Libc::Aparte), libc()],
        false,
    )
    .unwrap();

    assert_eq!(correr(&con.bytes), "BMO-X");
    assert_eq!(correr(&con.bytes), correr(&sin.bytes));
    assert!(
        con.tiradas.iter().any(|n| n == "isspace"),
        "nadie llama a 'isspace' y sigue dentro"
    );
    assert!(
        !con.tiradas.iter().any(|n| n == "strncpy"),
        "se tiro 'strncpy', que es justo lo que el programa usa"
    );
    assert!(
        con.bytes.len() * 5 < sin.bytes.len(),
        "podado {} B, sin podar {} B",
        con.bytes.len(),
        sin.bytes.len()
    );
    assert!(
        con.sin_podar.is_empty(),
        "hoy toda unidad de BMO C se puede podar; estas no: {:?}",
        con.sin_podar
    );
}

/// A una funcion a la que solo se llega por PUNTERO no la llama ningun `call`,
/// asi que no hay arista que seguir: quien la mantiene viva es su direccion
/// GUARDADA EN UN DATO. Sin esa raiz, la poda se la lleva y el programa salta
/// al vacio -- y lo haria en el metal, no aqui.
#[test]
fn la_poda_no_tira_lo_que_se_llama_por_puntero() {
    const POR_PUNTERO: &str = r#"
int doble(int x) { return x + x; }
int (*apunta)(int) = doble;
int main() {
    printf("%d", apunta(21));
    return 0;
}
"#;
    let i = enlazar_informado(&[objeto("puntero.bo", POR_PUNTERO)], true).unwrap();
    assert!(
        !i.tiradas.iter().any(|n| n == "doble"),
        "se tiro 'doble', a la que solo se llega por puntero"
    );
    assert_eq!(correr(&i.bytes), "42");
}

/// *** C++ TAMBIEN COMPILA POR SEPARADO, y no tuvo que escribir un emisor: baja
/// al arbol de BMO C y usa su codegen, asi que la compilacion separada de C++ la
/// pago E2 sin saberlo. Aqui una unidad define una clase y la otra la usa.
///
/// ** Y lo que esta fila NO dice: llamar a esto desde C. C++ DECORA los nombres
/// con la firma (`cobrar#i.i`), que es lo que hace posible sobrecargar, asi que
/// un `.bo` de C pide `cobrar` y no lo encuentra. Lo que falta para eso es
/// `extern "C"`, y es casilla E10 del plan -- no un arreglo del enlazador.
#[test]
fn dos_unidades_de_cpp_se_llaman_y_el_programa_corre() {
    const CLASE: &str = r#"
class Cuenta {
    int saldo;
public:
    void abrir(int s) { saldo = s; }
    int ingresar(int cuanto) { saldo = saldo + cuanto; return saldo; }
};
int cobrar(int base, int extra) {
    Cuenta c;
    c.abrir(base);
    return c.ingresar(extra);
}
"#;
    const PRINCIPAL_CPP: &str = r#"
int cobrar(int base, int extra);
int main() {
    printf("%d", cobrar(20, 22));
    return 0;
}
"#;
    let uno = bmo_cpp_front::compile_source_to_object(CLASE)
        .unwrap_or_else(|e| panic!("el C++ debe compilar a objeto: {}", e.message));
    let dos = bmo_cpp_front::compile_source_to_object(PRINCIPAL_CPP)
        .unwrap_or_else(|e| panic!("el C++ debe compilar a objeto: {}", e.message));
    let bex = enlazar(&[("principal.bo".to_string(), dos), ("cuenta.bo".to_string(), uno)])
        .expect("tiene que enlazar");
    assert_eq!(correr(&bex), "42");
}
