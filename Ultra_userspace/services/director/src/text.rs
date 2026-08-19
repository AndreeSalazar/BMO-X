//! Numeros a texto. Lo mas pequeno que hace falta y nada mas: una ventana que
//! no sabe escribir un numero no sirve para mirar un disco.

/// Un `u64` a decimal en `dst`. Sin `alloc` no hay `format!`, y un terminal
/// que no sabe escribir un numero no sirve para mirar un disco.
#[inline(never)]
pub(crate) fn decimal(mut v: u64, dst: &mut [u8; 10]) -> usize {
    if v == 0 {
        dst[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = 0;
    while v > 0 && n < tmp.len() {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    let how_many = n.min(dst.len());
    for i in 0..how_many {
        dst[i] = tmp[n - 1 - i];
    }
    how_many
}

/// **Completar con TAB.** Devuelve el nuevo largo de la linea.
///
/// Mejor que el de Windows a proposito, y la diferencia es una decision, no
/// una casualidad:
///
/// - Windows CICLA: pulsas TAB y te pone un candidato, otra vez y te pone el
///   siguiente. Nunca te ENSENA lo que hay, asi que a ciegas vas probando.
/// - Aqui se completa hasta el PREFIJO COMUN mas largo y, si quedaba mas de
///   un candidato, **se listan todos**. Un TAB te dice cuanto se puede
///   avanzar sin riesgo y que opciones te quedan. Es lo que hace bash, y es
///   lo unico honesto: adivinar por ti cual de cinco querias es mentir.
///
/// Si el unico candidato es una carpeta, se anade la barra -- porque lo
/// siguiente que vas a escribir es lo de dentro.
/// Es la entrada `.` o `..`?
///
/// FAT las guarda como entradas de verdad y `entry_at` las devuelve. Aqui no
/// sirven para nada --este terminal no tiene "carpeta actual" a la que volver--
/// y estorban en los dos sitios donde aparecen: envenenan el prefijo comun del
/// TAB y ensucian el `ls`.
pub(crate) fn is_dot_entry(name: &[u8]) -> bool {
    name == b"." || name == b".."
}

