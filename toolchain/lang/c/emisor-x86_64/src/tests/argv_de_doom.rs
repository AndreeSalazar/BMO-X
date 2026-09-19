//! **`!(-6)` valia -256**: por que DOOM iba a escribir su config sobre el WAD
//!
//! El arranque del 2026-08-13 dejo tres lineas que no cuadraban:
//!
//! ```text
//!    Development mode ON.                 (nadie paso -devparm)
//!    turbo scale: 200%                    (nadie paso -turbo)
//!    saving config in apps/doom1.wad      <- ESTA, y es la peligrosa
//! ```
//!
//! La tercera significa que `M_SaveDefaults` --registrado con `I_AtExit`-- iba a
//! escribir la configuracion de DOOM **encima del WAD**. No llego a ocurrir
//! porque el proceso murio antes; el fichero se comprobo despues y seguia con
//! sus 4.196.020 bytes intactos.
//!
//! ## De donde salia
//!
//! `M_LoadDefaults` solo imprime esa linea si `M_CheckParmWithArgs("-config", 1)`
//! **encontro** algo, y entonces usa `myargv[i+1]`. Con
//! `argv = {"doom.bex", "-iwad", "apps/doom1.wad"}` eso solo puede pasar si
//! `-config` casa con `-iwad`.
//!
//! Y casaba. La comparacion es `m_argv.c:49`:
//!
//! ```c
//! if (!strcasecmp(check, myargv[i])) return i;
//! ```
//!
//! `strcasecmp("-config", "-iwad")` contesta **-6** --correcto, la `c` es menor
//! que la `i`-- y esta perfecto, como demuestra el primer test de abajo. Lo que
//! estaba roto era el `!`:
//!
//! ```text
//!    !(-6)  ->  -256      y en un `if` eso es VERDADERO
//!    !(11)  ->  0         correcto
//!    !(0)   ->  1         correcto
//! ```
//!
//! `Expr::Not` emitia `test eax,eax` + `sete al` **y nada mas**. `setcc` solo
//! escribe `al`: con `rax = 0xFFFF_FFFF_FFFF_FFFA` (un -6), poner `al = 0` deja
//! `0xFFFF_FFFF_FFFF_FF00`, que es -256.
//!
//! Eso explica las tres lineas de golpe: `-devparm` ('d' < 'i') casa,
//! `-config` ('c' < 'i') casa, y `-turbo` ('t' > 'i') **no** casa con `-iwad`
//! pero si con `apps/doom1.wad` ('-' < 'a'), en el indice 2 -- que es el ultimo,
//! asi que `p < myargc-1` sale falso y la escala se queda en su 200 por defecto.
//! **Cada linea rara tiene su explicacion exacta en el mismo bug.**
//!
//! ## Lo que esto significa fuera de DOOM
//!
//! **`if (!strcmp(a, b))` es EL idioma de C para comparar cadenas**, y acertaba
//! siempre que `a` fuera alfabeticamente MENOR que `b`. Cualquier programa en C
//! que comparase cadenas asi ha estado tomando decisiones al azar.
//!
//! [!] Y la leccion ya estaba escrita en este mismo fichero del codegen:
//! `emit_cmp` lleva su `movzx` con un comentario que dice *"el movzx del final
//! NO es decorativo"*. Se aprendio en un sitio y no se aplico en el de al lado.

use super::*;

/// El dato crudo: que devuelve `strcasecmp` para los pares que importan.
///
/// `-config` vs `-iwad` difieren en la posicion 1: 'c'(99) vs 'i'(105) = **-6**.
/// `-turbo`  vs `-iwad`:                          't'(116) vs 'i'(105) = **+11**.
///
/// Y en el Ryzen `-config` CASA y `-turbo` NO. O sea que la sospecha es el
/// SIGNO, no la comparacion.
#[test]
fn que_devuelve_strcasecmp() {
    let out = run_c_con_pp(
        "#include <string.h>\n\
         int main() {\n\
           printf(\"cfg=%d turbo=%d dev=%d igual=%d\\n\",\n\
             strcasecmp(\"-config\", \"-iwad\"),\n\
             strcasecmp(\"-turbo\", \"-iwad\"),\n\
             strcasecmp(\"-devparm\", \"-iwad\"),\n\
             strcasecmp(\"-iwad\", \"-iwad\"));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "cfg=-6 turbo=11 dev=-5 igual=0");
}

/// Y lo que hace el `!` con esos mismos valores. `!(-6)` es **0** en C.
#[test]
fn el_not_de_un_negativo_es_cero() {
    let out = run_c_con_pp(
        "int menos_seis() { return -6; }\n\
         int mas_once() { return 11; }\n\
         int cero() { return 0; }\n\
         int main() {\n\
           printf(\"%d %d %d\\n\", !menos_seis(), !mas_once(), !cero());\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "0 0 1");
}

#[test]
fn tolower_no_aplasta_los_signos() {
    let out = run_c_con_pp(
        "#include <ctype.h>\n\
         int main() {\n\
           printf(\"%d %d %d %d\\n\", tolower('-'), tolower('c'), tolower('C'), tolower('i'));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "45 99 99 105");
}

#[test]
fn atoi_de_una_ruta_da_cero() {
    let out = run_c_con_pp(
        "#include <stdlib.h>\n\
         int main() { printf(\"%d %d\\n\", atoi(\"apps/doom1.wad\"), atoi(\"200\")); return 0; }\n",
    );
    assert_eq!(out.trim(), "0 200");
}

#[test]
fn m_checkparm_de_doom_reducido() {
    let out = run_c_con_pp(
        "#include <string.h>\n\
         static char *g_argv[3] = { \"doom.bex\", \"-iwad\", \"apps/doom1.wad\" };\n\
         static int myargc = 3;\n\
         static char **myargv = g_argv;\n\
         int M_CheckParmWithArgs(char *check, int num_args) {\n\
           int i;\n\
           for (i = 1; i < myargc - num_args; i++) {\n\
             if (!strcasecmp(check, myargv[i])) { return i; }\n\
           }\n\
           return 0;\n\
         }\n\
         int main() {\n\
           printf(\"iwad=%d config=%d turbo=%d devparm=%d\\n\",\n\
             M_CheckParmWithArgs(\"-iwad\", 1),\n\
             M_CheckParmWithArgs(\"-config\", 1),\n\
             M_CheckParmWithArgs(\"-turbo\", 1),\n\
             M_CheckParmWithArgs(\"-devparm\", 0));\n\
           return 0;\n\
         }\n",
    );
    assert_eq!(out.trim(), "iwad=1 config=0 turbo=0 devparm=0");
}
