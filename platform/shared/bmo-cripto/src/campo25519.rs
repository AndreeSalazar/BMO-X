//! **LA ARITMETICA DE X25519**: numeros modulo `p = 2^255 - 19`.
//!
//! # Por que un fichero solo para sumar y multiplicar
//!
//! Porque **aqui es donde X25519 se rompe si se rompe**. La curva de arriba son
//! treinta lineas de escalera; esto son las cuentas que esas treinta lineas
//! hacen doscientas cincuenta y cinco veces, y un acarreo mal propagado da un
//! resultado que **parece un punto de la curva y no lo es**.
//!
//! Y es el mismo criterio que separa `forma.rs` de quien lo usa en todo este
//! arbol: aqui no hay ninguna decision, solo cuentas.
//!
//! # *** LA REPRESENTACION: CINCO LIMBOS DE 51 BITS
//!
//! Un numero de 255 bits no cabe en un registro, asi que se parte. Y **no se
//! parte en cuatro trozos de 64 sino en cinco de 51**, que parece desperdiciar
//! y es justo al reves:
//!
//! ```text
//!    4 x 64   lleno hasta el borde. CADA suma desborda y hay que propagar
//!             el acarreo en el acto -- una cadena de dependencias por
//!             operacion, que es lo que un CPU peor lleva
//!
//!    5 x 51   sobran 13 bits en cada limbo. Se pueden sumar cosas VARIAS
//!             VECES antes de tener que ordenar nada, y los acarreos se
//!             propagan de golpe al final
//! ```
//!
//! ** Y hay una segunda razon, y es la que hace elegante al 2^255 - 19: reducir
//! modulo ese primo es **multiplicar por 19 lo que se salio por arriba**.
//! Porque `2^255 = 19 (mod p)`, asi que un bit que se escapa del limbo 4 vuelve
//! a entrar por el 0 con un factor de 19. Eso es todo el truco.
//!
//! # [!] SOBRE EL `u128`, Y LA LEY DE LOS 64 BITS
//!
//! El dueno tiene escrita una regla para INTI: *"no imites todo Rust... necesito
//! que sea honesto a base de CPU desde 64 bit"*. Y aqui hay `u128`.
//!
//! *** No es una contradiccion, y conviene decir por que: **el silicio de esta
//! maquina TIENE una multiplicacion de 64x64 que devuelve 128 bits.** `mul` deja
//! la mitad alta en un registro y la baja en otro, y siempre ha sido asi. El
//! `u128` de Rust en x86-64 **no inventa un tipo que el CPU no tiene: nombra el
//! resultado que el CPU ya daba** y que en C no se puede escribir sin
//! intrinsecos.
//!
//! ** La regla de INTI dice que no se finja un entero de 128 bits *como tipo del
//! lenguaje* cuando el CPU no lo tiene. Un producto de 64x64 si lo tiene. Es la
//! misma distincion que separa `bits_de` --que cuesta cero porque el silicio ya
//! lo hacia-- de inventarse una aritmetica.

/// Un numero modulo `2^255 - 19`, en cinco limbos de 51 bits.
pub type Fe = [u64; 5];

/// La mascara de un limbo: 51 bits.
const M: u64 = (1u64 << 51) - 1;

/// **`2p`, para restar sin bajar de cero.**
///
/// ** Restar `a - b` cuando `b > a` daria la vuelta y el resultado no seria
/// congruente con nada. Sumando `2p` antes, la resta nunca baja de cero **y el
/// resultado sigue siendo el mismo modulo p** -- porque `2p = 0 (mod p)`.
///
/// Se usa `2p` y no `p` porque los limbos pueden venir con hasta 52 bits
/// puestos, y `p` no daria margen para todos.
const DOS_P: Fe = [
    0xFFFFFFFFFFFDA, // 2^52 - 38
    0xFFFFFFFFFFFFE, // 2^52 - 2
    0xFFFFFFFFFFFFE,
    0xFFFFFFFFFFFFE,
    0xFFFFFFFFFFFFE,
];

pub const CERO: Fe = [0; 5];
pub const UNO: Fe = [1, 0, 0, 0, 0];

/// **Ordena los limbos**: deja cada uno en 51 bits y devuelve lo que se salio
/// por arriba **multiplicado por 19**, que es la reduccion modulo `2^255 - 19`.
fn llevar(a: &mut Fe) {
    let mut c;
    c = a[0] >> 51; a[0] &= M; a[1] += c;
    c = a[1] >> 51; a[1] &= M; a[2] += c;
    c = a[2] >> 51; a[2] &= M; a[3] += c;
    c = a[3] >> 51; a[3] &= M; a[4] += c;
    // *** AQUI ESTA EL TRUCO DEL PRIMO. Lo que se sale del limbo 4 vale 2^255,
    // y `2^255 = 19 (mod p)`, asi que vuelve a entrar por el limbo 0 con un
    // factor de 19 en vez de perderse.
    c = a[4] >> 51; a[4] &= M; a[0] += c * 19;
    // Y una vuelta mas para el acarreo que ese `* 19` pueda haber creado.
    c = a[0] >> 51; a[0] &= M; a[1] += c;
}

pub fn suma(a: &Fe, b: &Fe) -> Fe {
    let mut r = [0u64; 5];
    for i in 0..5 {
        r[i] = a[i] + b[i];
    }
    llevar(&mut r);
    r
}

pub fn resta(a: &Fe, b: &Fe) -> Fe {
    let mut r = [0u64; 5];
    for i in 0..5 {
        // ** `+ 2p` ANTES de restar: ver `DOS_P`. Sin esto, `a - b` con `b > a`
        // da la vuelta y el numero deja de ser congruente con nada.
        r[i] = a[i] + DOS_P[i] - b[i];
    }
    llevar(&mut r);
    r
}

/// **El producto**, y es la funcion que hace el trabajo de todo el fichero.
///
/// ## La cuenta, y por que el 19 aparece cinco veces
///
/// Multiplicar dos numeros de cinco limbos da veinticinco productos parciales.
/// Los que caen por encima del limbo 4 valen `2^255` o mas, asi que **vuelven a
/// entrar multiplicados por 19** -- eso es lo que hacen los `19 *` de abajo, y
/// es la reduccion hecha DENTRO de la multiplicacion en vez de despues.
///
/// [!] Y los limites importan: cada limbo entra con menos de `2^52`, el `19 *`
/// lo sube a `2^57`, y cinco productos de `2^52 x 2^57` suman menos de `2^112`.
/// **Cabe de sobra en 128 bits**, y por eso esto no puede desbordar. Si algun
/// dia se cambiara la representacion, esta cuenta es la que hay que rehacer.
pub fn mul(a: &Fe, b: &Fe) -> Fe {
    let (a0, a1, a2, a3, a4) = (a[0] as u128, a[1] as u128, a[2] as u128, a[3] as u128, a[4] as u128);
    let (b0, b1, b2, b3, b4) = (b[0] as u128, b[1] as u128, b[2] as u128, b[3] as u128, b[4] as u128);
    let b1_19 = b1 * 19;
    let b2_19 = b2 * 19;
    let b3_19 = b3 * 19;
    let b4_19 = b4 * 19;

    let r0 = a0 * b0 + a1 * b4_19 + a2 * b3_19 + a3 * b2_19 + a4 * b1_19;
    let r1 = a0 * b1 + a1 * b0 + a2 * b4_19 + a3 * b3_19 + a4 * b2_19;
    let r2 = a0 * b2 + a1 * b1 + a2 * b0 + a3 * b4_19 + a4 * b3_19;
    let r3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4 * b4_19;
    let r4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

    // El acarreo, ahora en 128 bits porque los parciales no caben en 64.
    let mut c: u128;
    let mut t = [0u64; 5];
    let mm = M as u128;
    c = r0 >> 51; t[0] = (r0 & mm) as u64;
    let r1 = r1 + c;
    c = r1 >> 51; t[1] = (r1 & mm) as u64;
    let r2 = r2 + c;
    c = r2 >> 51; t[2] = (r2 & mm) as u64;
    let r3 = r3 + c;
    c = r3 >> 51; t[3] = (r3 & mm) as u64;
    let r4 = r4 + c;
    c = r4 >> 51; t[4] = (r4 & mm) as u64;
    // Y lo que se salio, de vuelta por el 0 con su 19.
    t[0] += (c as u64) * 19;
    let cc = t[0] >> 51; t[0] &= M; t[1] += cc;
    t
}

pub fn cuadrado(a: &Fe) -> Fe {
    mul(a, a)
}

/// `a` elevado a `2^n`. Es cuadrar `n` veces, y existe porque la inversion lo
/// pide en tandas de hasta cien.
fn cuadrado_n(a: &Fe, n: u32) -> Fe {
    let mut r = *a;
    for _ in 0..n {
        r = cuadrado(&r);
    }
    r
}

/// **El inverso multiplicativo**: el numero que multiplicado por `a` da 1.
///
/// ## Por que se calcula elevando y no con Euclides
///
/// El algoritmo de Euclides extendido es mas rapido **y tiene ramas que
/// dependen del valor**. En una pieza que toca claves privadas eso es un fallo,
/// no una optimizacion: el tiempo que tarda contaria algo del secreto.
///
/// *** Por el pequeno teorema de Fermat, `a^(p-2) = a^-1 (mod p)`. Elevar tarda
/// **siempre lo mismo**, porque el exponente es una constante del algoritmo y no
/// el dato. Es la regla 2 del crate aplicada donde de verdad muerde.
///
/// ** Y no se eleva a la brava: `p-2 = 2^255 - 21` se alcanza con una cadena de
/// unas 265 operaciones en vez de 510. La cadena es la conocida de Curve25519 y
/// los nombres dicen que hay dentro de cada paso: `z_10_0` es *"diez unos
/// seguidos, empezando en el bit 0"*.
pub fn invertir(a: &Fe) -> Fe {
    let z2 = cuadrado(a);
    let z8 = cuadrado_n(&z2, 2);
    let z9 = mul(&z8, a);
    let z11 = mul(&z9, &z2);
    let z22 = cuadrado(&z11);
    let z_5_0 = mul(&z22, &z9);

    let t = cuadrado_n(&z_5_0, 5);
    let z_10_0 = mul(&t, &z_5_0);

    let t = cuadrado_n(&z_10_0, 10);
    let z_20_0 = mul(&t, &z_10_0);

    let t = cuadrado_n(&z_20_0, 20);
    let z_40_0 = mul(&t, &z_20_0);

    let t = cuadrado_n(&z_40_0, 10);
    let z_50_0 = mul(&t, &z_10_0);

    let t = cuadrado_n(&z_50_0, 50);
    let z_100_0 = mul(&t, &z_50_0);

    let t = cuadrado_n(&z_100_0, 100);
    let z_200_0 = mul(&t, &z_100_0);

    let t = cuadrado_n(&z_200_0, 50);
    let z_250_0 = mul(&t, &z_50_0);

    let t = cuadrado_n(&z_250_0, 5);
    mul(&t, &z11)
}

/// **Intercambia `a` y `b` si `cual` vale 1. SIN RAMIFICAR.**
///
/// *** Es la funcion que hace que la escalera de Montgomery sea de tiempo
/// constante, y la razon entera de que la escalera se escriba asi.
///
/// ** Un `if bit == 1 { swap() }` daria el mismo resultado y **filtraria el
/// secreto**: el escalar de X25519 ES la clave privada, y sus 255 bits deciden
/// 255 veces si hay intercambio. Quien pudiera medir la diferencia entre
/// intercambiar y no hacerlo leeria la clave bit a bit.
///
/// Aqui se hace SIEMPRE la misma cuenta: una mascara de todo unos o todo ceros,
/// un `xor` y dos escrituras. El camino no depende del bit.
pub fn intercambio(cual: u64, a: &mut Fe, b: &mut Fe) {
    // `cual` es 0 o 1; la mascara sale todo ceros o todo unos.
    let mascara = 0u64.wrapping_sub(cual);
    for i in 0..5 {
        let t = mascara & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

/// **De 32 bytes a un numero.** Little-endian, y **el bit 255 se tira**.
///
/// [!] Ese bit se ignora porque lo dice el RFC 7748, y no es un detalle de
/// formato: una `u` con el bit alto puesto representa el mismo punto que sin el,
/// y aceptar las dos formas dejaria que el mismo punto tuviera dos
/// codificaciones -- que es como se cuelan dos claves publicas "distintas" que
/// dan el mismo secreto.
pub fn desde_bytes(b: &[u8; 32]) -> Fe {
    let l = |i: usize| -> u64 {
        let mut v = [0u8; 8];
        v.copy_from_slice(&b[i..i + 8]);
        u64::from_le_bytes(v)
    };
    let mut f = [0u64; 5];
    f[0] = l(0) & M;
    f[1] = (l(6) >> 3) & M;
    f[2] = (l(12) >> 6) & M;
    f[3] = (l(19) >> 1) & M;
    // ** El `& 0x7F...` del ultimo es lo que tira el bit 255.
    f[4] = (l(24) >> 12) & M;
    f
}

/// **Deja el numero en su forma unica** y lo pasa a 32 bytes.
///
/// ## *** POR QUE HACE FALTA "CONGELAR", Y ES EL FALLO QUE NO SE VE
///
/// Con cinco limbos de 51 bits, **el mismo numero se puede escribir de varias
/// formas**: `p` y `0` son el mismo elemento, y despues de sumar y multiplicar
/// los limbos quedan como quedan. Mientras se opera da igual -- las cuentas son
/// modulo `p`.
///
/// ** Pero al SALIR no da igual: dos maquinas que calculen el mismo secreto
/// compartido tienen que escribir **los mismos 32 bytes**, o el HKDF de encima
/// sacara claves distintas y el apreton fallara sin que nada diga por que.
///
/// Asi que aqui se ordena todo y **se resta `p` si el numero llego a valer `p` o
/// mas**. La resta condicional se hace sin ramificar, por lo mismo que el
/// intercambio.
pub fn a_bytes(a: &Fe) -> [u8; 32] {
    let mut t = *a;
    llevar(&mut t);
    llevar(&mut t);

    // ** `t` esta ya por debajo de `2^255`, pero puede estar entre `p` y
    // `2^255`. Se mira sumando 19: `t >= p`  <=>  `t + 19 >= 2^255`.
    let mut q = (t[0] + 19) >> 51;
    q = (t[1] + q) >> 51;
    q = (t[2] + q) >> 51;
    q = (t[3] + q) >> 51;
    q = (t[4] + q) >> 51;
    // `q` vale 1 si hay que restar `p`, y 0 si no. Se suma `19*q` y se tira el
    // bit 255: eso ES restar `p`, sin un `if`.
    t[0] += 19 * q;
    let mut c;
    c = t[0] >> 51; t[0] &= M; t[1] += c;
    c = t[1] >> 51; t[1] &= M; t[2] += c;
    c = t[2] >> 51; t[2] &= M; t[3] += c;
    c = t[3] >> 51; t[3] &= M; t[4] += c;
    t[4] &= M;

    // ** Los cinco limbos de 51 bits salen como 32 bytes little-endian con un
    // acumulador que se vacia de ocho en ocho. Se escribe asi --y no con
    // desplazamientos a mano por byte-- porque **este bucle se puede leer y
    // comprobar de un vistazo**, y el otro solo se puede creer.
    //
    // [!] El acumulador no desborda: se vacia hasta dejar menos de 8 bits antes
    // de meter los 51 siguientes, asi que nunca pasa de 59.
    let mut s = [0u8; 32];
    let mut acc: u128 = 0;
    let mut bits = 0u32;
    let mut o = 0usize;
    for i in 0..5 {
        acc |= (t[i] as u128) << bits;
        bits += 51;
        while bits >= 8 && o < 32 {
            s[o] = (acc & 0xFF) as u8;
            acc >>= 8;
            bits -= 8;
            o += 1;
        }
    }
    while o < 32 {
        s[o] = (acc & 0xFF) as u8;
        acc >>= 8;
        o += 1;
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// *** LO QUE SALE TIENE QUE VOLVER A ENTRAR IGUAL.
    ///
    /// ** Es la prueba mas barata del fichero y la primera que se corre cuando
    /// algo falla arriba: si la serializacion no cuadra, **nada** de lo que
    /// dependa de ella puede cuadrar, y buscar el fallo en la escalera seria
    /// buscarlo en el sitio equivocado. El 24-08 sirvio exactamente para eso --
    /// paso a la primera, y con eso el sospechoso quedo acotado a la escalera.
    #[test]
    fn ida_y_vuelta() {
        for s in [1u8, 7, 0x42, 0xFE] {
            let mut b = [s; 32];
            b[31] &= 0x7F;
            let f = desde_bytes(&b);
            assert_eq!(a_bytes(&f), b, "no vuelve con {s:#x}");
        }
    }
    #[test]
    fn el_nueve_va_y_vuelve() {
        let mut b = [0u8; 32];
        b[0] = 9;
        assert_eq!(a_bytes(&desde_bytes(&b)), b);
    }
}
