//! # EL ESCALADO: agrandar una imagen sin torcerla
//!
//! ## Por que este eje existe
//!
//! DOOM dibuja 320x200 y el panel del Ryzen tiene 1920x1080. Quien agranda es
//! `DG_DrawFrame`, y su bucle tiene **cuatro sitios donde una imagen sale
//! torcida en vez de salir mal**:
//!
//! | el sitio | el sintoma si falla |
//! |---|---|
//! | el paso (stride) del framebuffer | la imagen sale inclinada, en diagonal |
//! | el centrado (`x0`, `y0`) | descolocada, o escribiendo fuera del panel |
//! | la expansion de un pixel a `escala` | rayas, o una columna de mas/menos |
//! | el recorte de la ultima fila | **se escribe pasado el final del panel** |
//!
//! Los tres primeros se ven y se arreglan. El cuarto no se ve: escribe en
//! memoria de otro. En el Ryzen `fb.rs::mapped_bytes` mapea exactamente
//! `alto * stride * 4`, asi que pasarse es un `#PF` -- y ya se pago una vez con
//! el raycaster, al que le faltaban dos de los cuatro topes de la franja de
//! pared.
//!
//! ## El metodo: un panel de juguete que se puede LEER ENTERO
//!
//! Una fuente de 3x3 con los digitos 1..9, un panel de 8x8 y un paso de **10**
//! --a proposito distinto del ancho, que es lo que caza un bug de stride--. El
//! programa imprime el panel entero como digitos, asi que la casilla no
//! compara un pixel: compara **el dibujo**. Una diagonal se ve a simple vista.
//!
//! Es el mismo bucle que corre en metal, con los mismos topes, compilado por el
//! mismo compilador y ejecutado en el emulador. Lo que no cubre es el ancho de
//! banda hacia el framebuffer real -- eso lo mide el `[perf]` de DOOM.
//!
//! ## ** Primer barrido: las 3 verdes
//!
//! Escrito el 2026-08-14, el dia que la escala paso de estar clavada en el
//! binario a salir del panel.

use super::census::{sweep, Cell};

/// El bucle de `DG_DrawFrame`, con la escala y el panel como parametros.
///
/// Se genera en vez de escribirse tres veces porque lo que cambia entre las
/// casillas son dos numeros: copiarlo seria el patron 26 de la casa otra vez.
fn blit(escala: i32, panel: i32) -> &'static str {
    let src = format!(
        "#define FW 3\n\
         #define FH 3\n\
         #define PASO 10\n\
         #define PANEL {panel}\n\
         int fuente[FW * FH] = {{1,2,3,4,5,6,7,8,9}};\n\
         int fb[PASO * PANEL];\n\
         int fila[64];\n\
         int escala = {escala};\n\
         int dst_ancho;\n\
         int dst_alto;\n\
         int x0;\n\
         int y0;\n\
         int main() {{\n\
         \x20 int y; int x; int k; int filas; int p; int i;\n\
         \x20 dst_ancho = FW * escala;\n\
         \x20 dst_alto = FH * escala;\n\
         \x20 if (dst_ancho > PANEL) {{ dst_ancho = PANEL; }}\n\
         \x20 if (dst_alto > PANEL) {{ dst_alto = PANEL; }}\n\
         \x20 x0 = (PANEL - dst_ancho) / 2;\n\
         \x20 y0 = (PANEL - dst_alto) / 2;\n\
         \x20 for (i = 0; i < PASO * PANEL; i = i + 1) {{ fb[i] = 0; }}\n\
         \x20 for (y = 0; y < FH; y = y + 1) {{\n\
         \x20   filas = escala;\n\
         \x20   if (y * escala + filas > dst_alto) {{ filas = dst_alto - y * escala; }}\n\
         \x20   if (filas <= 0) {{ break; }}\n\
         \x20   if (escala == 1) {{\n\
         \x20     memcpy(&fb[(y0 + y) * PASO + x0], &fuente[y * FW], dst_ancho * 4);\n\
         \x20     continue;\n\
         \x20   }}\n\
         \x20   for (x = 0; x < FW; x = x + 1) {{\n\
         \x20     p = fuente[y * FW + x];\n\
         \x20     for (k = 0; k < escala; k = k + 1) {{ fila[x * escala + k] = p; }}\n\
         \x20   }}\n\
         \x20   for (k = 0; k < filas; k = k + 1) {{\n\
         \x20     memcpy(&fb[(y0 + y * escala + k) * PASO + x0], fila, dst_ancho * 4);\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 for (y = 0; y < PANEL; y = y + 1) {{\n\
         \x20   for (x = 0; x < PASO; x = x + 1) {{ printf(\"%d\", fb[y * PASO + x]); }}\n\
         \x20   printf(\"\\n\");\n\
         \x20 }}\n\
         \x20 return 0;\n\
         }}"
    );
    Box::leak(src.into_boxed_str())
}

fn census() -> Vec<Cell> {
    vec![
        Cell {
            // Escala 1: el camino directo, sin expandir nada. Es el que
            // gobernaba antes de que la escala saliera del panel, asi que si
            // esta cae es una regresion de lo que YA funcionaba en metal.
            //
            // [!] Aqui la imagen mide 3x3 --no 6x6-- asi que el centrado cae
            // en `(8-3)/2 = 2`, y no en 1. La primera version de esta casilla
            // esperaba 1 y **el compilador tenia razon**: el error estaba en la
            // cuenta escrita a mano, que es justo para lo que sirve un censo
            // que compara el dibujo entero.
            name: "escala 1, el camino directo",
            source: blit(1, 8),
            expects: "\
0000000000\n\
0000000000\n\
0012300000\n\
0045600000\n\
0078900000\n\
0000000000\n\
0000000000\n\
0000000000",
        },
        Cell {
            // Escala 2 en un panel que le sobra: 6x6 centrados en 8x8, o sea
            // x0 = y0 = 1. Cada pixel es un cuadro de 2x2 y las columnas 7,8,9
            // --las que el PASO anade y el panel no tiene-- se quedan en cero.
            // Si aparece un digito ahi, el stride esta mal.
            name: "escala 2, centrada y con paso",
            source: blit(2, 8),
            expects: "\
0000000000\n\
0112233000\n\
0112233000\n\
0445566000\n\
0445566000\n\
0778899000\n\
0778899000\n\
0000000000",
        },
        Cell {
            // ** La que importa: escala 3 pide 9x9 y el panel tiene 8x8.
            // La ultima fila de origen solo puede poner DOS de sus tres filas,
            // y la ultima columna se corta por la mitad. Sin los dos topes
            // esto escribe pasado el final del panel -- que en el Ryzen es un
            // #PF y no un garabato.
            name: "escala 3 que NO cabe: recorte",
            source: blit(3, 8),
            expects: "\
1112223300\n\
1112223300\n\
1112223300\n\
4445556600\n\
4445556600\n\
4445556600\n\
7778889900\n\
7778889900",
        },
    ]
}

#[test]
fn el_escalado_no_tuerce_la_imagen() {
    sweep(
        &census(),
        CENSUS,
        "EL ESCALADO DE DOOM CAMBIO.\n\
         Mirar el DIBUJO, no el diff: el sintoma dice cual de los cuatro\n\
         sitios es. Si sale en diagonal, es el PASO. Si sale entero pero\n\
         corrido, es el centrado. Si hay una columna de mas o de menos, es la\n\
         expansion. Y si la casilla que cae es la del recorte, eso en metal\n\
         **escribe fuera del framebuffer**: no desplegar.",
    );
}

/// **EL CENSO DEL ESCALADO, al 2026-08-14.** Verde entero desde el primer
/// barrido.
const CENSUS: &str = "\
escala 1, el camino directo    GOOD
escala 2, centrada y con paso  GOOD
escala 3 que NO cabe: recorte  GOOD
";
