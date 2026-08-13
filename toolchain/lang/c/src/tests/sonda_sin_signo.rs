//! # LA SONDA DEL SIGNO -- las operaciones que preguntan por el bit alto
//!
//! ## Un eje mas, y por que NO es el de `sonda_de_anchos`
//!
//! Aquella pregunta *"que le pasa a un numero al GUARDARSE en un tipo mas
//! estrecho"* -- estrechar, extender, promocionar. Salieron sus 16 verdes.
//!
//! Esta pregunta otra cosa: **como se LEE el patron de bits al operar con el**.
//! Son ejes distintos porque un `unsigned long` no se estrecha nunca y aun asi
//! tiene cuatro operaciones que se equivocan con el.
//!
//! ## Por que este eje, y por que ahora
//!
//! Porque `angle_t` de DOOM es `unsigned int` y da la vuelta a proposito: el
//! renderizador entero --`R_PointToAngle`, `R_ScaleFromGlobalAngle`, el
//! recorrido del BSP-- vive en aritmetica que solo tiene sentido sin signo. Un
//! `>>` que copie el bit de signo convierte todos los angulos por encima de
//! 180 grados en basura, y eso no da un error: da una imagen.
//!
//! ## ** Lo que encontro: CUATRO operaciones, y las cuatro solo en 64 bits
//!
//! ```text
//!   (unsigned long)0x8000000000000000 >> 60   daba 18446744073709551608
//!   ...                               / 2     daba un negativo enorme
//!   ...                               % 10    idem
//!   ...                               > 1     daba 0
//! ```
//!
//! [!] **En 32 bits acertaban por casualidad**, y esa es la parte que hay que
//! entender o el arreglo parece innecesario. El codegen calcula todo en `rax`,
//! o sea en 64 bits: un `unsigned int` con el bit 31 puesto se carga con
//! `mov eax` y llega **extendido con ceros**, asi que el bit 63 vale 0 y `sar`
//! da lo mismo que `shr`. Con un `unsigned long` el bit 63 es del valor, y ahi
//! las cuatro se caen a la vez.
//!
//! O sea que el defecto llevaba ahi desde el primer dia, tapado por el ancho
//! del acumulador. Es hermano del patron 15: no lo escondia un test que
//! faltara, lo escondia que **el caso que lo destapa no se puede escribir en 32
//! bits**.
//!
//! ## Y el arm de `Shr` lo CONFESABA en prosa
//!
//! *"El desplazamiento a la derecha es ARITMETICO (`sar`), que es lo correcto
//! para `int`. Un tipo sin signo querria `shr`; hoy el codegen no arrastra esa
//! distincion hasta aqui."*
//!
//! Y era falso: la distincion SI llegaba. `var_type_of` existe, y `Field`,
//! `Arrow` e `IndexPtr` traen su `TypeSpec` dentro -- lo unico que faltaba era
//! preguntar. `expr_is_unsigned` se escribio calcada de `expr_is_float`, que
//! llevaba al lado todo el tiempo. **Un fallo confesado en prosa sigue siendo
//! un fallo**, y esta es la tercera vez que esta casa lo paga.

use super::censo::{barrer, Casilla};

fn censo() -> Vec<Casilla> {
    vec![
        Casilla {
            // ** LA DE `angle_t`: `angle >> ANGLETOFINESHIFT` con el bit alto
            // puesto. Con `sar` en vez de `shr` sale negativo.
            nombre: "unsigned >> con bit alto",
            fuente: "int main() { unsigned int a; a = 0x80000000; \
                       printf(\"%u\\n\", a >> 19); return 0; }",
            espera: "4096",
        },
        Casilla {
            nombre: "int >> con bit alto (sar)",
            fuente: "int main() { int a; a = -2147483648; \
                       printf(\"%d\\n\", a >> 19); return 0; }",
            espera: "-4096",
        },
        Casilla {
            nombre: "unsigned long >> con bit alto",
            fuente: "int main() { unsigned long a; a = 0x8000000000000000; \
                       printf(\"%lu\\n\", a >> 60); return 0; }",
            espera: "8",
        },
        Casilla {
            // `if (angle < ANG90)` con angulos por encima de 180 grados.
            nombre: "unsigned < con bit alto",
            fuente: "int main() { unsigned int a; unsigned int b; \
                       a = 0x90000000; b = 0x10000000; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            espera: "1",
        },
        Casilla {
            nombre: "int < con bit alto (con signo)",
            fuente: "int main() { int a; int b; a = -1879048192; b = 268435456; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            espera: "0",
        },
        Casilla {
            // El angulo da la vuelta: es la aritmetica de `angle_t` entera.
            nombre: "unsigned da la vuelta al sumar",
            fuente: "int main() { unsigned int a; a = 0xC0000000; a = a + 0x80000000; \
                       printf(\"%u\\n\", a); return 0; }",
            espera: "1073741824",
        },
        Casilla {
            nombre: "unsigned / con bit alto",
            fuente: "int main() { unsigned int a; a = 0x80000000; \
                       printf(\"%u\\n\", a / 4); return 0; }",
            espera: "536870912",
        },
        Casilla {
            nombre: "unsigned resto con bit alto",
            fuente: "int main() { unsigned int a; a = 0x80000007; \
                       printf(\"%u\\n\", a % 10); return 0; }",
            espera: "5",
        },
        Casilla {
            // `%u` contra `%d` sobre el mismo valor: es la forma de ver que el
            // printf tampoco se lleva el signo puesto.
            nombre: "printf %u no saca negativo",
            fuente: "int main() { unsigned int a; a = 3000000000; \
                       printf(\"%u\\n\", a); return 0; }",
            espera: "3000000000",
        },
        Casilla {
            // `FixedDiv`/`FixedMul` de DOOM: 64 bits en medio y vuelta a 32.
            nombre: "fixed mul: 64 en medio",
            fuente: "int main() { int a; int b; long long p; \
                       a = 65536 * 3; b = 65536 / 2; \
                       p = ((long long)a * (long long)b) >> 16; \
                       printf(\"%d\\n\", (int)p); return 0; }",
            espera: "98304",
        },
        Casilla {
            // `FixedDiv` con negativo, que es la mitad de las llamadas.
            nombre: "fixed div con negativo",
            fuente: "int main() { int a; int b; long long r; \
                       a = -65536 * 3; b = 65536 * 2; \
                       r = (((long long)a) << 16) / b; \
                       printf(\"%d\\n\", (int)r); return 0; }",
            espera: "-98304",
        },
        // -- Y lo mismo en 64 bits, que es donde el valor SI tiene el bit alto
        Casilla {
            nombre: "unsigned long / con bit 63",
            fuente: "int main() { unsigned long a; a = 0x8000000000000000; \
                       printf(\"%lu\\n\", a / 2); return 0; }",
            espera: "4611686018427387904",
        },
        Casilla {
            // [!] Segunda vez que el censo caza una cuenta MIA y no del
            // compilador: escribi `1` y contesto `3`. `0x8000000000000005` es
            // 9223372036854775813, y acaba en 3. La primera fue en
            // `sonda_de_disposicion`. Dos de dos: la aritmetica a ojo sobre
            // numeros de 19 digitos no es de fiar, y por eso el censo compara
            // un informe entero en vez de creerse un `assert` suelto.
            nombre: "unsigned long resto, bit 63",
            fuente: "int main() { unsigned long a; a = 0x8000000000000005; \
                       printf(\"%lu\\n\", a % 10); return 0; }",
            espera: "3",
        },
        Casilla {
            nombre: "unsigned long > con bit 63",
            fuente: "int main() { unsigned long a; unsigned long b; \
                       a = 0x8000000000000000; b = 1; \
                       printf(\"%d\\n\", (int)(a > b)); return 0; }",
            espera: "1",
        },
        Casilla {
            // La forma de `bmo_valor`: el kernel devuelve un `unsigned long
            // long` y el programa lo parte en dos mitades.
            nombre: "partir un u64 del kernel",
            fuente: "int main() { unsigned long long d; d = 0x0000028000000190; \
                       printf(\"%d %d\\n\", (int)(d >> 32), (int)(d & 0xFFFFFFFF)); return 0; }",
            espera: "640 400",
        },
        Casilla {
            // `unsigned short` que se promociona: los `children[2]` del BSP
            // llevan el bit 15 como marca de subsector.
            nombre: "unsigned short bit 15",
            fuente: "int main() { unsigned short c; c = 0x8005; \
                       printf(\"%d %d\\n\", (int)c, (int)(c & 0x8000)); return 0; }",
            espera: "32773 32768",
        },
    ]
}

#[test]
fn el_censo_sin_signo_no_ha_cambiado() {
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DEL SIGNO CAMBIO.\n\
         Si una casilla de 64 bits se puso en ROJO, mirar `expr_is_unsigned` en\n\
         el codegen: es lo que decide entre `shr`/`sar`, `div`/`idiv` y\n\
         `setb`/`setl`. Y ojo -- las de 32 bits aciertan aunque la regla este\n\
         mal, porque el valor llega a `rax` extendido con ceros.",
    );
}

/// **EL CENSO DEL SIGNO, al 2026-08-13.** Verde desde que el codegen pregunta
/// por el tipo antes de elegir la instruccion. Antes, las cuatro filas de
/// `unsigned long` estaban rojas.
const CENSO: &str = "\
unsigned >> con bit alto       BIEN
int >> con bit alto (sar)      BIEN
unsigned long >> con bit alto  BIEN
unsigned < con bit alto        BIEN
int < con bit alto (con signo) BIEN
unsigned da la vuelta al sumar BIEN
unsigned / con bit alto        BIEN
unsigned resto con bit alto    BIEN
printf %u no saca negativo     BIEN
fixed mul: 64 en medio         BIEN
fixed div con negativo         BIEN
unsigned long / con bit 63     BIEN
unsigned long resto, bit 63    BIEN
unsigned long > con bit 63     BIEN
partir un u64 del kernel       BIEN
unsigned short bit 15          BIEN
";
