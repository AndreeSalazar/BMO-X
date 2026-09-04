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
//!
//! ## ** 2026-09-04: la expansion dejo de ir de pixel en pixel
//!
//! `DG_DrawFrame` ya no escribe un pixel por vuelta. Quien expande es
//! `expandir_fila`, que camina dos punteros --sin `imul`, sin recargar una
//! global-- y mete **dos pixeles iguales en UNA escritura de 8 bytes**, que es
//! el ancho natural del CPU. A escala 5 son 3 escrituras por pixel de origen
//! en vez de 5.
//!
//! Las tres casillas de abajo **no cambian de dibujo**: son las mismas tres
//! imagenes, expandidas de otra manera. Por eso valen mas hoy que el dia que se
//! escribieron -- un censo solo demuestra algo cuando debajo cambia el codigo y
//! encima no cambia el resultado.
//!
//! Y hay una cuarta prueba que no dibuja: ejecuta la trampa del
//! ENSANCHAMIENTO. Ver `el_par_de_8_bytes_no_se_llena_de_unos`.

use super::census::{sweep, Cell};
use super::run_c;

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
         unsigned int fuente[FW * FH] = {{1,2,3,4,5,6,7,8,9}};\n\
         unsigned int fb[PASO * PANEL];\n\
         unsigned int fila[64];\n\
         int escala = {escala};\n\
         int dst_ancho;\n\
         int dst_alto;\n\
         int x0;\n\
         int y0;\n\
         void expandir_fila(unsigned int *destino, unsigned int *fuente,\n\
         \x20                  int pixeles, int escala) {{\n\
         \x20 unsigned long long *d8; unsigned long long par;\n\
         \x20 unsigned long long mascara; unsigned int *d4; unsigned int p;\n\
         \x20 int dobles; int suelto; int x; int j;\n\
         \x20 if (escala == 1) {{ memcpy(destino, fuente, pixeles * 4); return; }}\n\
         \x20 mascara = 1; mascara = (mascara << 32) - 1;\n\
         \x20 dobles = escala / 2; suelto = escala - dobles * 2;\n\
         \x20 d4 = destino;\n\
         \x20 for (x = 0; x < pixeles; x = x + 1) {{\n\
         \x20   p = *fuente; fuente = fuente + 1;\n\
         \x20   par = (unsigned long long)p; par = par & mascara;\n\
         \x20   par = par | (par << 32);\n\
         \x20   d8 = (unsigned long long *)d4;\n\
         \x20   j = dobles;\n\
         \x20   while (j > 0) {{ *d8 = par; d8 = d8 + 1; j = j - 1; }}\n\
         \x20   d4 = (unsigned int *)d8;\n\
         \x20   if (suelto != 0) {{ *d4 = p; d4 = d4 + 1; }}\n\
         \x20 }}\n\
         }}\n\
         int main() {{\n\
         \x20 int y; int x; int k; int filas; int i;\n\
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
         \x20   expandir_fila(fila, &fuente[y * FW], FW, escala);\n\
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

/// **EL PAR DE 8 BYTES NO SE LLENA DE UNOS.**
///
/// `expandir_fila` mete dos pixeles en una escritura de 8 bytes haciendo
/// `par | (par << 32)`. Si el paso de `unsigned int` a `unsigned long long`
/// llevara SIGNO, un pixel con el bit alto puesto --y los de DOOM lo tienen: el
/// alfa es 0xFF-- dejaria la mitad de arriba a unos, y el SEGUNDO pixel del par
/// saldria 0xFFFFFFFF, o sea blanco.
///
/// En metal el sintoma seria una imagen a rayas verticales blancas, que mirando
/// la pantalla no se atribuye a nadie. Aqui son tres numeros.
///
/// ** Se comprueba por IGUALDAD y no por el hexadecimal impreso, y luego se
/// imprime el hexadecimal aparte. Asi las dos cosas que podrian ensanchar mal
/// --la expansion y el `%x` de `printf`, que tambien pasa por 64 bits-- se
/// distinguen: si las igualdades salen a 1 y el hexadecimal sale largo, el
/// fallo no es de este bucle.
#[test]
fn el_par_de_8_bytes_no_se_llena_de_unos() {
    let out = run_c(
        r#"unsigned int fuente[2];
unsigned int fila[8];
void expandir_fila(unsigned int *destino, unsigned int *fuente,
                   int pixeles, int escala) {
  unsigned long long *d8; unsigned long long par;
  unsigned long long mascara; unsigned int *d4; unsigned int p;
  int dobles; int suelto; int x; int j;
  if (escala == 1) { memcpy(destino, fuente, pixeles * 4); return; }
  mascara = 1; mascara = (mascara << 32) - 1;
  dobles = escala / 2; suelto = escala - dobles * 2;
  d4 = destino;
  for (x = 0; x < pixeles; x = x + 1) {
    p = *fuente; fuente = fuente + 1;
    par = (unsigned long long)p; par = par & mascara;
    par = par | (par << 32);
    d8 = (unsigned long long *)d4;
    j = dobles;
    while (j > 0) { *d8 = par; d8 = d8 + 1; j = j - 1; }
    d4 = (unsigned int *)d8;
    if (suelto != 0) { *d4 = p; d4 = d4 + 1; }
  }
}
int main() {
  int i;
  /* 4286611488 = 0xFF808020: alfa 0xFF, o sea el bit alto puesto. */
  fuente[0] = 4286611488;
  fuente[1] = 7;
  for (i = 0; i < 8; i = i + 1) { fila[i] = 0; }
  expandir_fila(fila, fuente, 2, 3);
  printf("%d%d%d%d%d",
         fila[0] == 4286611488, fila[1] == 4286611488, fila[2] == 4286611488,
         fila[3] == 7, fila[5] == 7);
  printf(" | %x\n", fila[1]);
  return 0;
}"#,
    );
    assert_eq!(
        out.trim(),
        "11111 | ff808020",
        "el par de 8 bytes se ensancho CON SIGNO: el segundo pixel sale blanco"
    );
}

/// **EL CENSO DEL ESCALADO, al 2026-08-14.** Verde entero desde el primer
/// barrido, y las tres siguen verdes con la expansion nueva del 04-09.
const CENSUS: &str = "\
escala 1, el camino directo    GOOD
escala 2, centrada y con paso  GOOD
escala 3 que NO cabe: recorte  GOOD
";
