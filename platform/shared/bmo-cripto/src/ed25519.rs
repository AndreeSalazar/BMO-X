//! **Ed25519 -- COMPROBAR una firma.** La pieza que el `.bex` estaba esperando.
//!
//! # Que hace, y sobre todo que NO hace
//!
//! ```text
//!    verificar   [X]  esta pagina
//!    FIRMAR      --   y no se va a escribir aqui. Ver abajo
//! ```
//!
//! *** **Que falte firmar no es una obra a medias: es la decision.** La tiene
//! escrita `PLAN_SEGURIDAD.md` C3, y es sobre donde vive la clave privada:
//!
//! > *"En la maquina no, o vuelve el problema de 4.2 del maestro. Vive donde se
//! > firma, que es el anfitrion, y a BMO-X solo baja la publica."*
//!
//! Una maquina que puede firmar tiene dentro con que falsificar lo que ejecuta.
//! **BMO-X solo necesita saber decir que no**, y para eso la clave publica basta.
//! Escribir el firmador aqui seria meter en el sistema justo lo que el sistema
//! existe para no tener.
//!
//! # De donde sale cada pieza, y cuanto habia ya
//!
//! ```text
//!    el campo 2^255-19   campo25519.rs   YA ESTABA, escrito para X25519
//!    SHA-512             sha512.rs       25-08, y entro por esto
//!    la curva de Edwards aqui            lo unico de verdad nuevo
//! ```
//!
//! ** Eso es lo que hacia que esta casilla no fuera la que parecia. `PLAN_SEGURIDAD`
//! la llamaba *"LA pieza"* cuando no habia ni un hash en el arbol; **la
//! aritmetica modular sobre `2^255-19` --la parte que asusta-- llevaba escrita y
//! probada desde X25519.** Lo que faltaba era otra curva encima del mismo campo.
//!
//! # Por que se comprueba con `[S]B = R + [k]A` y no con la de los ochos
//!
//! RFC 8032 seccion 5.1.7 da las dos y dice cual es cual:
//!
//! > *"Check the group equation `[8][S]B = [8]R + [8][k]A'`. It's sufficient,
//! > but not required, to instead check `[S]B = R + [k]A'`."*
//!
//! Se usa la segunda. Es **mas estricta**: rechaza algunas firmas que la de los
//! ochos aceptaria --las que meten un punto de torsion pequena-- y no acepta
//! ninguna que aquella rechace. Para lo que hace falta aqui --*"este `.bex` lo
//! firmo quien digo"*-- ser mas estricto es lo correcto.
//!
//! [!] Y hay que decirlo porque tiene consecuencia: **dos implementaciones de
//! Ed25519 pueden discrepar en firmas raras y las dos cumplir el RFC.** Si algun
//! dia una firma valida en otro sitio se rechaza aqui, esta es la primera linea
//! que hay que releer.
//!
//! # Lo que esta pagina NO promete
//!
//! **No es de tiempo constante, y no tiene por que serlo.** Verificar no toca
//! ningun secreto: la clave publica, el mensaje y la firma son publicos los
//! tres. Medir cuanto tarda no dice nada que no se pueda leer.
//!
//! * Es la misma regla que declara `sha256.rs`, aplicada al reves: alli el hash
//! no lleva secreto; **aqui el que no lo lleva es todo el calculo.** El dia que
//! se escriba el firmador --en el anfitrion, no aqui-- esa promesa cambia y hay
//! que decirlo en su pagina.

use crate::campo25519::{self as fe, Fe};
use crate::sha512;

/// Bytes de una clave publica.
pub const CLAVE: usize = 32;
/// Bytes de una firma: `R` y `S`, 32 cada uno.
pub const FIRMA: usize = 64;

/// **`d` de la curva**, `-121665/121666 mod p`. La constante que define
/// Edwards25519 y la separa de cualquier otra curva sobre el mismo campo.
const D_BYTES: [u8; 32] = [
    0xa3, 0x78, 0x59, 0x13, 0xca, 0x4d, 0xeb, 0x75, 0xab, 0xd8, 0x41, 0x41, 0x4d, 0x0a, 0x70, 0x00,
    0x98, 0xe8, 0x79, 0x77, 0x79, 0x40, 0xc7, 0x8c, 0x73, 0xfe, 0x6f, 0x2b, 0xee, 0x6c, 0x03, 0x52,
];

/// **La raiz cuadrada de -1**, `2^((p-1)/4)`. Hace falta al descomprimir: la
/// formula da una raiz y **a veces es la de `-x^2`**, y entonces se corrige
/// multiplicando por esto.
const RAIZ_MENOS_UNO: [u8; 32] = [
    0xb0, 0xa0, 0x0e, 0x4a, 0x27, 0x1b, 0xee, 0xc4, 0x78, 0xe4, 0x2f, 0xad, 0x06, 0x18, 0x43, 0x2f,
    0xa7, 0xd7, 0xfb, 0x3d, 0x99, 0x00, 0x4d, 0x2b, 0x0b, 0xdf, 0xc1, 0x4f, 0x80, 0x24, 0x83, 0x2b,
];

/// **El punto generador**, en su forma comprimida. `y = 4/5`, y el bit de signo
/// a cero.
///
/// * Se guarda COMPRIMIDO y se descomprime al usarlo, en vez de escribir sus
/// coordenadas como constantes. Cuesta una descompresion por verificacion y
/// compra algo que vale mas: **si la descompresion estuviera rota, ni el
/// generador saldria** -- o sea que el fallo aparece en el primer vector en vez
/// de esconderse hasta que llegue una clave con el bit de signo puesto.
const BASE: [u8; 32] = [
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
];

/// **`L`, el orden del grupo**: `2^252 + 27742317777372353535851937790883648493`.
/// En little-endian, que es como viaja `S` dentro de la firma.
const L: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

/// **Un punto, en coordenadas extendidas.** `x = X/Z`, `y = Y/Z`, y `T` cumple
/// `T/Z = x*y`.
///
/// ** La cuarta coordenada es lo que hace que sumar dos puntos no lleve ni una
/// inversion. Una inversion del campo cuesta como 250 multiplicaciones; con
/// `T` se paga una sola al final, al comprimir.
#[derive(Clone, Copy)]
struct Punto {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

/// El neutro: `(0, 1)`, que en extendidas es `(0, 1, 1, 0)`.
const NEUTRO: Punto = Punto { x: fe::CERO, y: fe::UNO, z: fe::UNO, t: fe::CERO };

/// `z^((p-5)/8)`, o sea `z^(2^252 - 3)`.
///
/// Es la exponenciacion que hace falta para la raiz cuadrada, y va por una
/// **cadena de adicion**: 265 operaciones en vez de las 252 al cuadrado mas 252
/// multiplicaciones que costaria el metodo ingenuo.
///
/// [!] La cadena se lee mal a proposito -- no hay forma de que se lea bien-- y
/// por eso lo que la respalda no es la lectura: es que **si estuviera mal, ni
/// una sola firma de los vectores del RFC verificaria**. Un error aqui no da
/// "casi bien".
fn pow_p58(z: &Fe) -> Fe {
    let sq = fe::cuadrado;
    let ml = fe::mul;
    let mut t0 = sq(z);
    let mut t1 = sq(&t0);
    t1 = sq(&t1);
    t1 = ml(z, &t1);
    t0 = ml(&t0, &t1);
    t0 = sq(&t0);
    t0 = ml(&t1, &t0);
    t1 = sq(&t0);
    for _ in 1..5 {
        t1 = sq(&t1);
    }
    t0 = ml(&t1, &t0);
    t1 = sq(&t0);
    for _ in 1..10 {
        t1 = sq(&t1);
    }
    t1 = ml(&t1, &t0);
    let mut t2 = sq(&t1);
    for _ in 1..20 {
        t2 = sq(&t2);
    }
    t1 = ml(&t2, &t1);
    t1 = sq(&t1);
    for _ in 1..10 {
        t1 = sq(&t1);
    }
    t0 = ml(&t1, &t0);
    t1 = sq(&t0);
    for _ in 1..50 {
        t1 = sq(&t1);
    }
    t1 = ml(&t1, &t0);
    t2 = sq(&t1);
    for _ in 1..100 {
        t2 = sq(&t2);
    }
    t1 = ml(&t2, &t1);
    t1 = sq(&t1);
    for _ in 1..50 {
        t1 = sq(&t1);
    }
    t0 = ml(&t1, &t0);
    t0 = sq(&t0);
    t0 = sq(&t0);
    ml(&t0, z)
}

fn iguales(a: &Fe, b: &Fe) -> bool {
    fe::a_bytes(a) == fe::a_bytes(b)
}

fn es_cero(a: &Fe) -> bool {
    fe::a_bytes(a) == [0u8; 32]
}

/// **DESCOMPRIMIR: de 32 bytes a un punto.** `None` si esos bytes no son ningun
/// punto de la curva.
///
/// # Que llega, y por que hay que desconfiar
///
/// Llegan `y` en los 255 bits bajos y **el signo de `x` en el bit 255**. De ahi
/// sale `x` resolviendo `x^2 = (y^2-1)/(d*y^2+1)`, y ahi esta el peligro: **la
/// mayoria de los 32 bytes posibles NO son ningun punto.** Una clave publica
/// inventada llega aqui, y lo que este `None` impide es seguir calculando con un
/// punto que no existe.
///
/// ** Los tres motivos de rechazo, y son distintos:
///
/// ```text
///    1. no hay raiz              esos bytes no estan en la curva
///    2. x = 0 con signo 1        el cero no tiene dos signos: es UNA
///                               codificacion de dos, y aceptarla dejaria que
///                               el mismo punto tuviera dos claves distintas
///    3. (no se comprueba y = 0)  ese caso lo tapa el 1
/// ```
fn descomprimir(b: &[u8; 32]) -> Option<Punto> {
    let d = fe::desde_bytes(&D_BYTES);
    let y = fe::desde_bytes(b);
    let y2 = fe::cuadrado(&y);

    // u = y^2-1   v = d*y^2+1
    let u = fe::resta(&y2, &fe::UNO);
    let v = fe::suma(&fe::mul(&d, &y2), &fe::UNO);

    // x = u * v^3 * (u * v^7)^((p-5)/8), que es la raiz de u/v si la hay.
    let v2 = fe::cuadrado(&v);
    let v3 = fe::mul(&v2, &v);
    let v4 = fe::cuadrado(&v2);
    let v7 = fe::mul(&v3, &v4);
    let mut x = fe::mul(&pow_p58(&fe::mul(&u, &v7)), &fe::mul(&u, &v3));

    // ** Y AQUI SE COMPRUEBA, no se supone. `v*x^2` tiene que dar `u`.
    let chk = fe::mul(&v, &fe::cuadrado(&x));
    if !iguales(&chk, &u) {
        // La formula dio la raiz de `-u/v`. Se corrige y se vuelve a mirar.
        x = fe::mul(&x, &fe::desde_bytes(&RAIZ_MENOS_UNO));
        let chk = fe::mul(&v, &fe::cuadrado(&x));
        if !iguales(&chk, &u) {
            return None;
        }
    }

    // El signo. `x` es par o impar; el bit 255 dice cual queria el que firmo.
    let signo = (b[31] >> 7) & 1;
    if es_cero(&x) && signo == 1 {
        // El cero no tiene signo. Aceptar `-0` daria DOS codificaciones del
        // mismo punto, que es la puerta por la que se cuelan dos claves
        // "distintas" que son la misma.
        return None;
    }
    if (fe::a_bytes(&x)[0] & 1) != signo {
        x = fe::resta(&fe::CERO, &x);
    }

    Some(Punto { x, y, z: fe::UNO, t: fe::mul(&x, &y) })
}

/// **COMPRIMIR**: el punto a sus 32 bytes. Es la unica inversion de todo el
/// calculo, y por eso va al final y una sola vez.
fn comprimir(p: &Punto) -> [u8; 32] {
    let zi = fe::invertir(&p.z);
    let x = fe::mul(&p.x, &zi);
    let y = fe::mul(&p.y, &zi);
    let mut out = fe::a_bytes(&y);
    out[31] |= (fe::a_bytes(&x)[0] & 1) << 7;
    out
}

/// **Sumar dos puntos.** La formula de coordenadas extendidas, sin ramas y sin
/// casos especiales: **funciona tambien cuando los dos puntos son el mismo o
/// alguno es el neutro**, que es lo que la hace segura de usar en un bucle.
///
/// ** Esa propiedad --que se llama *completa*-- es lo que evita el fallo clasico
/// de las curvas de Weierstrass, donde sumar un punto consigo mismo pide OTRA
/// formula y olvidarlo da un resultado equivocado solo a veces.
fn suma(a: &Punto, b: &Punto) -> Punto {
    let d = fe::desde_bytes(&D_BYTES);
    let dos_d = fe::suma(&d, &d);

    let aa = fe::mul(&fe::resta(&a.y, &a.x), &fe::resta(&b.y, &b.x));
    let bb = fe::mul(&fe::suma(&a.y, &a.x), &fe::suma(&b.y, &b.x));
    let cc = fe::mul(&fe::mul(&a.t, &b.t), &dos_d);
    let dd = fe::mul(&fe::suma(&a.z, &a.z), &b.z);

    let e = fe::resta(&bb, &aa);
    let f = fe::resta(&dd, &cc);
    let g = fe::suma(&dd, &cc);
    let h = fe::suma(&bb, &aa);

    Punto {
        x: fe::mul(&e, &f),
        y: fe::mul(&g, &h),
        z: fe::mul(&f, &g),
        t: fe::mul(&e, &h),
    }
}

/// **`[n]P`**, con `n` en little-endian y de la longitud que sea.
///
/// Doblar-y-sumar, del bit alto al bajo. **Con rama, y a proposito**: en
/// verificar no hay ningun secreto --ni la firma, ni el mensaje, ni la clave
/// publica lo son-- asi que medir el tiempo no revela nada. Ver la cabecera.
///
/// [!] Y acepta escalares de **64 bytes** sin reducir, que es lo que pide el
/// RFC: el reto `k` es el digest de SHA-512 entero, leido como numero.
fn por_escalar(n: &[u8], p: &Punto) -> Punto {
    let mut r = NEUTRO;
    for byte in n.iter().rev() {
        for bit in (0..8).rev() {
            r = suma(&r, &r);
            if (byte >> bit) & 1 == 1 {
                r = suma(&r, p);
            }
        }
    }
    r
}

/// **`s < L`?** Con los dos numeros en little-endian, comparando de arriba abajo.
///
/// *** ESTA COMPROBACION NO ES UN DETALLE DE FORMATO. Sin ella, a una firma
/// valida `(R, S)` se le puede sumar `L` a la `S` y sale otra firma **que
/// tambien verifica** -- el mismo mensaje, la misma clave, dos firmas distintas.
///
/// Eso rompe cualquier cosa que use la firma como identidad: dos `.bex` con
/// bytes distintos y la misma autoria, o un registro que crea haber visto dos
/// entregas donde hubo una. Se llama *maleabilidad*, y el RFC la cierra aqui.
fn s_valida(s: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if s[i] < L[i] {
            return true;
        }
        if s[i] > L[i] {
            return false;
        }
    }
    // Exactamente `L` tampoco vale: el rango es `0 <= s < L`.
    false
}

/// **Tiene este punto orden PEQUENO?** O sea: `[8]P` es el neutro.
///
/// # *** ESTA FUNCION LA PIDIO UNA PRUEBA, EL 2026-08-25
///
/// La prueba de la firma de ceros --escrita porque el 24-08 se quito del arbol
/// un `verify_ed25519` que contestaba `true` a una firma de ceros-- **fallo en
/// la primera pasada**. Con clave y firma a cero, `verificar` decia que SI.
///
/// Y no era un fallo de la curva: era la curva funcionando.
///
/// ```text
///    32 bytes a cero  ->  y = 0  ->  x2 = (0-1)/(0+1) = -1
///                     ->  y -1 SI tiene raiz en este campo
/// ```
///
/// O sea que **una clave de ceros es un punto de verdad**: uno de orden 4. Con
/// `S = 0` la ecuacion se queda en `[-k]T == T`, que se cumple **una de cada
/// cuatro veces** segun lo que salga del hash. Con el mensaje del vector 1
/// salio.
///
/// *** Y ES LA TRAMPA DE C1 OTRA VEZ, POR OTRA PUERTA. Aquella decia:
///
/// > *"para pasar el control no hay que falsificar una firma, hay que
/// > BORRARLA."*
///
/// Se arreglo quitando el `if is_unsigned { return true; }`. Y la misma entrada
/// --todo ceros-- volvia a pasar, ahora **por matematicas en vez de por un
/// atajo**. Un agujero tapado por arriba y abierto por abajo.
///
/// # Como se cierra
///
/// Los puntos de orden pequeno son ocho, y todos cumplen `[8]P = O`. Un punto
/// legitimo tiene orden `L`, que es primo y enorme: `[8]P` no puede ser el
/// neutro. **Tres doblados y una comparacion**, y no hay mas.
///
/// [!] Y se aplica **a la clave publica Y a la `R` de la firma**. La `R` de una
/// firma honrada es `[r]B` con `r` aleatorio, asi que su orden es grande: no se
/// rechaza nada que alguien haya firmado de verdad.
fn orden_pequeno(p: &Punto) -> bool {
    let p2 = suma(p, p);
    let p4 = suma(&p2, &p2);
    let p8 = suma(&p4, &p4);
    comprimir(&p8) == comprimir(&NEUTRO)
}

/// **COMPROBAR UNA FIRMA.** `true` solo si `firma` es de `mensaje` bajo `publica`.
///
/// # Los cuatro pasos, y cada uno puede decir que no
///
/// ```text
///    1. S en rango          `0 <= S < L`, o la firma es maleable
///    2. A descomprime       o esa clave publica no es un punto
///    3. R descomprime       o esa firma no lleva un punto dentro
///    4. ni A ni R de orden PEQUENO   <- la que una prueba destapo el 25-08
///    5. [S]B == R + [k]A    k = SHA-512(R || A || mensaje)
/// ```
///
/// ** El orden es el barato primero: la comparacion de `S` son 32 bytes y las
/// descompresiones cuestan una exponenciacion cada una. Una firma basura se cae
/// en el paso 1 sin gastar nada.
///
/// [!] **Devuelve `bool` y no `Result`, y es la decision del 24-08 al reves de
/// como parece.** Aquel dia se quito un `bool` --`verify_ed25519`, que contestaba
/// `true` a una firma de ceros-- porque no habia con que comprobar y *"no lo se"*
/// no cabia en dos valores. **Ahora si hay con que**: aqui `false` significa
/// *"comprobado, y no"*, que es una respuesta y no una ausencia.
pub fn verificar(publica: &[u8; CLAVE], mensaje: &[u8], firma: &[u8; FIRMA]) -> bool {
    let mut r_bytes = [0u8; 32];
    let mut s_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&firma[..32]);
    s_bytes.copy_from_slice(&firma[32..]);

    if !s_valida(&s_bytes) {
        return false;
    }
    let Some(a) = descomprimir(publica) else {
        return false;
    };
    let Some(r) = descomprimir(&r_bytes) else {
        return false;
    };
    // *** Y NI A NI R PUEDEN SER DE ORDEN PEQUENO. Ver `orden_pequeno`: sin
    // esto, una clave de ceros --que ES un punto-- hace que la ecuacion se
    // cumpla una de cada cuatro veces.
    if orden_pequeno(&a) || orden_pequeno(&r) {
        return false;
    }

    // k = SHA-512(R || A || mensaje), leido como numero little-endian y **sin
    // reducir**: RFC 8032, 5.1.7 paso 2.
    let mut h = sha512::Sha512::nuevo();
    h.mete(&r_bytes);
    h.mete(publica);
    h.mete(mensaje);
    let k = h.cierra();

    // ** `-A` en vez de `A`, para poder comprobar `[S]B - [k]A == R` con una
    // suma. Negar un punto de Edwards es negar la `x` (y la `t`), y sale gratis.
    let menos_a = Punto {
        x: fe::resta(&fe::CERO, &a.x),
        y: a.y,
        z: a.z,
        t: fe::resta(&fe::CERO, &a.t),
    };

    let base = match descomprimir(&BASE) {
        Some(b) => b,
        // No puede pasar: `BASE` es una constante de este fichero. Si pasara,
        // la descompresion esta rota y **ninguna firma debe darse por buena**.
        None => return false,
    };

    let izquierda = suma(&por_escalar(&s_bytes, &base), &por_escalar(&k, &menos_a));
    comprimir(&izquierda) == r_bytes
}

#[cfg(test)]
mod pruebas;
