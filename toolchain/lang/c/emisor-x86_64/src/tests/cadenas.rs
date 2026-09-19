//! `strncmp`, `memcmp` y `strchr` -- las que DOOM usa en cada linea
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// =============== El tope, y el cero que corta ===============

#[test]
fn strncmp_solo_mira_los_primeros_n() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strncmp(\"HOLAxxx\", \"HOLAyyy\", 4)); return 0; }",
    );
    assert_eq!(out, "0\n", "los cuatro primeros son iguales");
}

#[test]
fn strncmp_encuentra_la_diferencia_dentro_del_tope() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strncmp(\"HOLA\", \"HOLO\", 4) != 0); return 0; }",
    );
    assert_eq!(out, "1\n");
}

/// * La diferencia entera entre `strncmp` y `memcmp`, y la razon de que sean
/// dos funciones: con el tope pasado del final, `strncmp` se para en el
/// terminador y `memcmp` seguiria leyendo lo que hubiera detras.
#[test]
fn strncmp_se_para_en_el_terminador_aunque_quede_cupo() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strncmp(\"AB\", \"AB\", 40)); return 0; }",
    );
    assert_eq!(out, "0\n", "los dos acaban a la vez: son iguales");
}

#[test]
fn strncmp_de_cero_bytes_siempre_es_igual() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strncmp(\"A\", \"B\", 0)); return 0; }",
    );
    assert_eq!(out, "0\n", "sin bytes que comparar no hay diferencia");
}

#[test]
fn strncmp_conserva_el_orden() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strncmp(\"A\", \"B\", 1) < 0); return 0; }",
    );
    assert_eq!(out, "1\n", "'A' va antes que 'B', y el signo lo dice");
}

// =============== `memcmp`: el cero es un byte mas ===============

#[test]
fn memcmp_compara_bytes_y_no_cadenas() {
    // Dos buffers iguales salvo DETRAS del terminador. `strncmp` los da por
    // iguales; `memcmp` con el tope entero tiene que ver la diferencia.
    let out = run_c(
        "int main() {\n\
             char a[6]; char b[6];\n\
             a[0]='A'; a[1]=0; a[2]='X'; a[3]=0; a[4]=0; a[5]=0;\n\
             b[0]='A'; b[1]=0; b[2]='Y'; b[3]=0; b[4]=0; b[5]=0;\n\
             printf(\"%d %d\\n\", strncmp(a,b,6)==0, memcmp(a,b,6)!=0);\n\
             return 0; }",
    );
    assert_eq!(
        out, "1 1\n",
        "strncmp se para en el cero; memcmp mira los seis bytes"
    );
}

// =============== `strchr` ===============

#[test]
fn strchr_encuentra_y_devuelve_el_resto() {
    let out = run_c(
        "int main() { char *p = strchr(\"mapa/E1M1\", '/'); printf(\"%s\\n\", p); return 0; }",
    );
    assert_eq!(out, "/E1M1\n");
}

#[test]
fn strchr_que_no_esta_devuelve_cero() {
    let out = run_c(
        "int main() { printf(\"%d\\n\", strchr(\"HOLA\", 'z') == 0); return 0; }",
    );
    assert_eq!(out, "1\n");
}

/// Buscar el terminador **lo encuentra**: es lo que dice el estandar y es la
/// forma normal de saber donde acaba una cadena.
#[test]
fn strchr_del_cero_encuentra_el_final() {
    let out = run_c(
        "int main() { char *s = \"AB\"; printf(\"%d\\n\", (int)(strchr(s, 0) - s)); return 0; }",
    );
    assert_eq!(out, "2\n");
}
