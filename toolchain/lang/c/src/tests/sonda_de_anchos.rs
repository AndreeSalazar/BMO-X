//! # LA SONDA DEL ANCHO -- que le pasa a un numero al guardarse mas estrecho
//!
//! ## El eje, y de donde sale
//!
//! `SHORT(x)` de DOOM es literalmente `((signed short)(x))` (`i_swap.h:26`), y
//! **cada campo que sale del WAD pasa por ahi**. Detras suele venir una
//! promocion a `int` o un `<<FRACBITS`. O sea que entre el disco y la pantalla
//! hay una cadena estrechar -> ensanchar por cada numero del juego.
//!
//! Y no es teorico que los valores sean negativos: `patch->leftoffset` y
//! `topoffset` de los sprites lo son casi siempre --es como DOOM centra un
//! sprite sobre su cosa-- y `sector->floorheight` tambien. Un `short` que no
//! extienda el signo al cargarse no da un error: da un suelo a una altura
//! absurda o un sprite fuera de la pantalla.
//!
//! ## ** Y el resultado del primer barrido fue: NADA
//!
//! Las 16 casillas salieron verdes a la primera. Eso tambien es un resultado,
//! y conviene que quede escrito: **el eje del ancho esta limpio**, asi que
//! cuando algo salga torcido en `R_Init` no hay que empezar por aqui.
//!
//! [!] Un censo que no encuentra nada no es un censo que sobra. Su trabajo
//! empieza ahora: si manana alguien toca la carga de un `short` y se le olvida
//! el `movsx`, estas 16 filas lo dicen en 0,2 segundos en vez de tres
//! flasheos.

use super::censo::{barrer, Casilla};

fn censo() -> [Casilla; 16] {
    [
        Casilla {
            nombre: "short guarda y devuelve negativo",
            fuente: "int main() { short s; s = -5; printf(\"%d\\n\", (int)s); return 0; }",
            espera: "-5",
        },
        Casilla {
            nombre: "short global negativo",
            fuente: "short g;\n\
                     int main() { g = -300; printf(\"%d\\n\", (int)g); return 0; }",
            espera: "-300",
        },
        Casilla {
            nombre: "campo short negativo",
            fuente: "typedef struct { short a; short b; } p_t;\n\
                     int main() { p_t s; s.a = -7; s.b = 3; \
                       printf(\"%d %d\\n\", (int)s.a, (int)s.b); return 0; }",
            espera: "-7 3",
        },
        Casilla {
            nombre: "(short) estrecha con signo",
            fuente: "int main() { int n; n = 65535; \
                       printf(\"%d\\n\", (int)(short)n); return 0; }",
            espera: "-1",
        },
        Casilla {
            nombre: "(short) de 0x8000",
            fuente: "int main() { int n; n = 32768; \
                       printf(\"%d\\n\", (int)(short)n); return 0; }",
            espera: "-32768",
        },
        Casilla {
            nombre: "unsigned short NO lleva signo",
            fuente: "int main() { unsigned short u; u = 65535; \
                       printf(\"%d\\n\", (int)u); return 0; }",
            espera: "65535",
        },
        Casilla {
            nombre: "short negativo << 16",
            fuente: "int main() { short s; s = -3; \
                       printf(\"%d\\n\", (int)(((int)s) << 16)); return 0; }",
            espera: "-196608",
        },
        Casilla {
            nombre: "short << 16 sin cast (DOOM)",
            fuente: "int main() { short s; int r; s = -3; r = s << 16; \
                       printf(\"%d\\n\", r); return 0; }",
            espera: "-196608",
        },
        Casilla {
            nombre: "char con signo",
            fuente: "int main() { char c; c = -1; printf(\"%d\\n\", (int)c); return 0; }",
            espera: "-1",
        },
        Casilla {
            nombre: "unsigned char sin signo",
            fuente: "int main() { unsigned char c; c = 200; \
                       printf(\"%d\\n\", (int)c); return 0; }",
            espera: "200",
        },
        Casilla {
            nombre: "short en array, negativo",
            fuente: "short t[4];\n\
                     int main() { t[2] = -1000; printf(\"%d\\n\", (int)t[2]); return 0; }",
            espera: "-1000",
        },
        Casilla {
            nombre: "short desde bytes crudos",
            fuente: "unsigned char b[4];\n\
                     int main() { short *p; b[0] = 0xFE; b[1] = 0xFF; \
                       p = (short *)b; printf(\"%d\\n\", (int)*p); return 0; }",
            espera: "-2",
        },
        Casilla {
            nombre: "short se desborda al guardar",
            fuente: "int main() { short s; s = 40000; printf(\"%d\\n\", (int)s); return 0; }",
            espera: "-25536",
        },
        Casilla {
            nombre: "division con negativo trunca a 0",
            fuente: "int main() { int a; a = -7; printf(\"%d %d\\n\", a / 2, a % 2); return 0; }",
            espera: "-3 -1",
        },
        Casilla {
            nombre: "desplazar a la derecha con signo",
            fuente: "int main() { int a; a = -256; printf(\"%d\\n\", a >> 4); return 0; }",
            espera: "-16",
        },
        Casilla {
            nombre: "short parametro y retorno",
            fuente: "short dob(short x) { return (short)(x * 2); }\n\
                     int main() { printf(\"%d\\n\", (int)dob(-1000)); return 0; }",
            espera: "-2000",
        },
    ]
}

#[test]
fn el_censo_de_los_anchos_no_ha_cambiado() {
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DE LOS ANCHOS CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION,\n\
         no un defecto viejo que sale a la luz. Mirar la carga del tipo\n\
         estrecho (`movsx` contra `movzx`) antes que nada.",
    );
}

/// **EL CENSO DE LOS ANCHOS, al 2026-08-13.** Verde entero desde el primer
/// barrido: no hizo falta arreglar nada.
const CENSO: &str = "\
short guarda y devuelve negativo BIEN
short global negativo          BIEN
campo short negativo           BIEN
(short) estrecha con signo     BIEN
(short) de 0x8000              BIEN
unsigned short NO lleva signo  BIEN
short negativo << 16           BIEN
short << 16 sin cast (DOOM)    BIEN
char con signo                 BIEN
unsigned char sin signo        BIEN
short en array, negativo       BIEN
short desde bytes crudos       BIEN
short se desborda al guardar   BIEN
division con negativo trunca a 0 BIEN
desplazar a la derecha con signo BIEN
short parametro y retorno      BIEN
";
