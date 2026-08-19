//! **EL NOMBRE DE VERDAD** -- las entradas de nombre largo, como FORMATO.
//!
//! Aqui no se toca el disco: entran y salen entradas de 32 bytes. Igual que
//! [`crate::bpb`], y por el mismo motivo -- asi el censo del final existe.
//!
//! # Por que esto no es opcional
//!
//! UEFI define su sistema de ficheros como FAT **con nombres UCS-2**:
//! `EFI_FILE_PROTOCOL::Open` recibe un `CHAR16*`. O sea que el nombre largo no
//! es una extension bonita que se pueda dejar para luego: **es el nombre**. Un
//! lector que solo entienda 8.3 no es un lector de UEFI, es un lector de
//! MS-DOS que casualmente arranca.
//!
//! `bmo-fat32` hace esto, literalmente, en dos sitios:
//!
//! ```text
//! if attr & 0x0F == 0x0F { continue; } // fragmento de nombre largo
//! ```
//!
//! **Se los salta.** Y ya esta mordiendo: el kernel declara
//! `LoadError::NameTooLong -- "Un nombre no cabe en 8.3"`. No es una carencia
//! teorica, es un error que ya se sabe devolver.
//!
//! # La forma, y por que es tan rara
//!
//! Las entradas largas van **delante** de la corta y **en orden inverso**, con
//! un numero de orden que empieza en 1 y donde la ULTIMA lleva el bit `0x40`.
//! Leyendo el directorio hacia adelante se encuentra primero el final del
//! nombre.
//!
//! Es rara porque tenia que serlo: un MS-DOS de 1994 no sabia de esto, y todo
//! lo que hacia era mirar el byte de atributos. `0x0F` es
//! `Solo lectura | Oculto | Sistema | Etiqueta de volumen` a la vez -- una
//! combinacion que ningun archivo real tiene, asi que los sistemas viejos
//! ignoraban esas entradas en vez de ensenarlas. **El formato se diseno para
//! ser invisible al software que no lo entiende**, y esa decision es la que
//! obliga a la suma de control de abajo.
//!
//! # La suma de control es el unico pegamento
//!
//! Nada ata una cadena larga a su entrada corta salvo un byte: la suma de los
//! once caracteres del nombre 8.3, repetida en cada fragmento. Si no cuadra,
//! **la cadena es basura huerfana** -- restos de un borrado que un sistema
//! viejo dejo a medias-- y hay que tirarla y quedarse con el 8.3.
//!
//! Eso no es una comprobacion de cortesia. Sin ella, un directorio con restos
//! devuelve nombres que pertenecieron a otro archivo.

/// Caracteres UCS-2 en una entrada larga: 5 + 6 + 2.
pub const POR_ENTRADA: usize = 13;

/// Cuantos fragmentos admite una cadena. El formato permite 20 (`Ord` llega a
/// 0x14), lo que da 260 caracteres; la especificacion limita el nombre a 255.
pub const MAX_FRAGMENTOS: usize = 20;

/// Caracteres de nombre que caben, y es el limite que declara el formato.
pub const MAX_NOMBRE: usize = 255;

/// Atributo que marca un fragmento de nombre largo.
pub const ATTR_LARGO: u8 = 0x0F;

/// Bit de `Ord` que marca el ULTIMO fragmento -- o sea el PRIMERO que se
/// encuentra al leer el directorio hacia adelante.
pub const ULTIMO: u8 = 0x40;

/// Un fragmento de nombre largo: los 32 bytes tal y como van al disco.
pub type Fragmento = [u8; 32];

/// **La suma de control que ata una cadena larga a su entrada 8.3.**
///
/// Es una rotacion a la derecha de 8 bits mas una suma, sobre los once bytes
/// del nombre corto tal y como estan en el disco -- **con los espacios de
/// relleno incluidos**. Calcularla sobre `"LEEME.TXT"` en vez de sobre
/// `"LEEME   TXT"` da un numero distinto y rompe todas las cadenas.
pub fn suma(nombre_8_3: &[u8; 11]) -> u8 {
    let mut s: u8 = 0;
    for &b in nombre_8_3.iter() {
        // La rotacion tiene que ser de 8 bits: en `u8` esto es exactamente
        // `rotate_right(1)`, y se escribe asi porque es lo que dice la
        // especificacion palabra por palabra.
        s = ((s & 1) << 7).wrapping_add(s >> 1).wrapping_add(b);
    }
    s
}

// ---------------------------------------------------------------------------
//  DESMONTAR: de una cadena de fragmentos a un nombre
// ---------------------------------------------------------------------------

/// Por que una cadena de fragmentos no da un nombre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoVale {
    /// No habia fragmentos.
    Vacia,
    /// El primero no llevaba el bit [`ULTIMO`]: falta la cabeza de la cadena.
    SinCabeza,
    /// Los numeros de orden no bajan de uno en uno hasta 1.
    OrdenRoto,
    /// Un fragmento no lleva `Attr == 0x0F`.
    NoEsFragmento,
    /// **La suma no cuadra con la entrada corta.** La cadena pertenecio a otro
    /// archivo: se tira y manda el 8.3.
    SumaMala,
    /// Mas caracteres de los que el formato admite.
    Larga,
    /// Un caracter que no cabe en el hueco que se dio.
    NoCabe,
}

/// Los 13 offsets de caracteres UCS-2 dentro de un fragmento: 5 en `Name1`
/// (1..11), 6 en `Name2` (14..26) y 2 en `Name3` (28..32).
///
/// Estan partidos asi porque en medio viven `Attr`, `Type`, `Chksum` y un
/// `FstClusLO` que siempre es cero -- los campos que hacen que un MS-DOS viejo
/// tome la entrada por un archivo con atributos imposibles y la ignore.
const HUECOS: [usize; POR_ENTRADA] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

fn car(frag: &Fragmento, i: usize) -> u16 {
    let o = HUECOS[i];
    u16::from_le_bytes([frag[o], frag[o + 1]])
}

/// **Junta una cadena de fragmentos en un nombre.**
///
/// `frags` van **en el orden en que aparecen en el directorio**, o sea del
/// ultimo al primero. Aqui se les da la vuelta; quien lee el directorio no
/// tiene que acordarse.
///
/// `corta` son los once bytes de la entrada 8.3 a la que dicen pertenecer, y
/// no es opcional: sin ella no se puede comprobar la suma, y sin comprobar la
/// suma este codigo devolveria nombres de archivos que ya no existen.
///
/// El nombre sale en UTF-16 crudo, sin convertir: la conversion a bytes es
/// decision de quien lo ensene, y aqui no se pierde nada por el camino.
pub fn desmontar(
    frags: &[Fragmento],
    corta: &[u8; 11],
    dst: &mut [u16],
) -> Result<usize, NoVale> {
    if frags.is_empty() {
        return Err(NoVale::Vacia);
    }
    if frags.len() > MAX_FRAGMENTOS {
        return Err(NoVale::Larga);
    }
    if frags[0][11] != ATTR_LARGO {
        return Err(NoVale::NoEsFragmento);
    }
    if frags[0][0] & ULTIMO == 0 {
        return Err(NoVale::SinCabeza);
    }

    let esperada = suma(corta);
    let cabeza = frags[0][0] & !ULTIMO;
    // La cabeza declara CUANTOS fragmentos hay. Si no coincide con los que se
    // pasaron, la cadena esta cortada -- y una cadena cortada da un nombre
    // cortado, que es peor que no dar ninguno porque parece bueno.
    if cabeza as usize != frags.len() || cabeza == 0 {
        return Err(NoVale::OrdenRoto);
    }

    for (i, f) in frags.iter().enumerate() {
        if f[11] != ATTR_LARGO {
            return Err(NoVale::NoEsFragmento);
        }
        // Los numeros bajan: el primero es `len`, el ultimo es 1. Solo el
        // primero lleva el bit ULTIMO.
        let orden = f[0] & !ULTIMO;
        let marcado = f[0] & ULTIMO != 0;
        if orden as usize != frags.len() - i || marcado != (i == 0) {
            return Err(NoVale::OrdenRoto);
        }
        if f[13] != esperada {
            return Err(NoVale::SumaMala);
        }
    }

    // Y ahora al reves, que es el orden del nombre.
    let mut n = 0usize;
    for f in frags.iter().rev() {
        for (i, _) in HUECOS.iter().enumerate() {
            let c = car(f, i);
            // `0x0000` termina el nombre; `0xFFFF` es el relleno de detras. Los
            // dos significan "aqui ya no hay nombre", y en cuanto aparece uno
            // se acabo: seguir leyendo trae el relleno como si fuera texto.
            if c == 0x0000 || c == 0xFFFF {
                return Ok(n);
            }
            if n >= dst.len() {
                return Err(NoVale::NoCabe);
            }
            if n >= MAX_NOMBRE {
                return Err(NoVale::Larga);
            }
            dst[n] = c;
            n += 1;
        }
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
//  MONTAR: de un nombre a una cadena de fragmentos
// ---------------------------------------------------------------------------

/// **Parte un nombre en la cadena de fragmentos que va al disco.**
///
/// Devuelve cuantos fragmentos se escribieron en `dst`, **ya en el orden en
/// que hay que grabarlos**: el ultimo trozo del nombre primero.
///
/// El relleno es el que pide la especificacion y no ceros: un `0x0000` para
/// cerrar el nombre si sobra sitio, y `0xFFFF` para todo lo demas. Rellenar con
/// ceros funciona al leer --el bucle de arriba para igual-- pero deja un
/// volumen que no se parece a lo que escribe Windows, y este formato es una
/// frontera: lo que se escribe aqui lo lee otro.
pub fn montar(
    nombre: &[u16],
    corta: &[u8; 11],
    dst: &mut [Fragmento],
) -> Result<usize, NoVale> {
    if nombre.is_empty() {
        return Err(NoVale::Vacia);
    }
    if nombre.len() > MAX_NOMBRE {
        return Err(NoVale::Larga);
    }
    let n = nombre.len().div_ceil(POR_ENTRADA);
    if n > MAX_FRAGMENTOS || n > dst.len() {
        return Err(NoVale::NoCabe);
    }
    let chk = suma(corta);

    for k in 0..n {
        // `k` es el indice del trozo contando desde el PRINCIPIO del nombre,
        // que es el orden `k + 1`. Y va al reves en el disco.
        let f = &mut dst[n - 1 - k];
        *f = [0u8; 32];
        f[0] = (k as u8) + 1;
        if k == n - 1 {
            f[0] |= ULTIMO;
        }
        f[11] = ATTR_LARGO;
        f[12] = 0;
        f[13] = chk;
        // `FstClusLO` (26..28) es cero siempre en un fragmento, y ya lo esta.

        for (i, &o) in HUECOS.iter().enumerate() {
            let idx = k * POR_ENTRADA + i;
            let c = if idx < nombre.len() {
                nombre[idx]
            } else if idx == nombre.len() {
                0x0000 // el cierre
            } else {
                0xFFFF // el relleno
            };
            f[o..o + 2].copy_from_slice(&c.to_le_bytes());
        }
    }
    Ok(n)
}

/// Cuantos fragmentos hacen falta para este nombre. Lo necesita quien busca
/// hueco en un directorio: hay que reservar `fragmentos + 1` entradas seguidas,
/// y **seguidas de verdad**, porque una cadena partida entre dos clusters no la
/// entiende nadie.
pub fn fragmentos_para(largo: usize) -> usize {
    largo.div_ceil(POR_ENTRADA)
}

/// Hace falta nombre largo para esto, o el 8.3 ya lo dice entero?
///
/// Merece la pena preguntarlo: un `KERNEL.ELF` no necesita cadena ninguna, y
/// escribirle una gasta una entrada de directorio por nada.
pub fn cabe_en_8_3(nombre: &[u16], corta: &[u8; 11]) -> bool {
    let mut esperado = [0u16; 12];
    let mut n = 0;
    for (i, &b) in corta[..8].iter().enumerate() {
        if b == b' ' {
            // El relleno es por la derecha: en cuanto hay un espacio se acabo
            // la base. Un nombre 8.3 no lleva espacios en medio.
            let _ = i;
            break;
        }
        esperado[n] = b as u16;
        n += 1;
    }
    if corta[8] != b' ' {
        esperado[n] = b'.' as u16;
        n += 1;
        for &b in corta[8..11].iter() {
            if b == b' ' {
                break;
            }
            esperado[n] = b as u16;
            n += 1;
        }
    }
    nombre == &esperado[..n]
}

// ===========================================================================
//  EL CENSO -- cadenas escritas a mano, cero discos encendidos
// ===========================================================================
#[cfg(test)]
mod censo {
    use super::*;

    /// Un nombre como los que escribe Windows, en UTF-16.
    fn u16s(s: &str) -> [u16; 64] {
        let mut v = [0u16; 64];
        for (i, c) in s.encode_utf16().enumerate() {
            v[i] = c;
        }
        v
    }
    fn largo(s: &str) -> usize {
        s.encode_utf16().count()
    }

    // -- La suma de control, que es el unico pegamento ----------------------

    #[test]
    fn la_suma_se_calcula_sobre_los_once_bytes_con_relleno() {
        // `PRESUP~1.XLS` en disco es "PRESUP~1XLS": ocho de base y tres de
        // extension, SIN punto y con espacios donde falte.
        let a = suma(b"PRESUP~1XLS");
        let b = suma(b"PRESUP~1TXT");
        assert_ne!(a, b, "la extension entra en la suma");

        // Y el relleno cuenta: "LEEME   TXT" no es "LEEMETXT   ".
        assert_ne!(suma(b"LEEME   TXT"), suma(b"LEEMETXT   "));
    }

    // -- Ida y vuelta -------------------------------------------------------

    #[test]
    fn presupuesto_anual_da_la_vuelta_entera() {
        let nombre = "Presupuesto anual 2026.xlsx";
        let n = largo(nombre);
        let src = u16s(nombre);
        let corta = *b"PRESUP~1XLS";

        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let cuantos = montar(&src[..n], &corta, &mut frags).expect("cabe");
        assert_eq!(cuantos, 3, "27 caracteres = 3 fragmentos de 13");

        let mut vuelta = [0u16; 64];
        let leidos = desmontar(&frags[..cuantos], &corta, &mut vuelta).expect("cadena sana");
        assert_eq!(leidos, n);
        assert_eq!(&vuelta[..leidos], &src[..n], "el nombre vuelve igual");
    }

    #[test]
    fn un_nombre_de_exactamente_trece_usa_un_solo_fragmento() {
        let nombre = "trece.chars12"; // 13
        assert_eq!(largo(nombre), 13);
        let src = u16s(nombre);
        let corta = *b"TRECEC~1CHA";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        assert_eq!(montar(&src[..13], &corta, &mut frags), Ok(1));

        let mut vuelta = [0u16; 64];
        let n = desmontar(&frags[..1], &corta, &mut vuelta).unwrap();
        assert_eq!(&vuelta[..n], &src[..13], "sin hueco para el 0x0000 de cierre");
    }

    #[test]
    fn los_acentos_y_la_enye_sobreviven() {
        // Aqui esta el motivo de que el nombre viaje en UTF-16 y no en bytes:
        // la pantalla de BMO es en espanol aunque las fuentes sean ASCII.
        let nombre = "Ma\u{00F1}ana caf\u{00E9}.txt";
        let n = largo(nombre);
        let src = u16s(nombre);
        let corta = *b"MAANAC~1TXT";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..n], &corta, &mut frags).unwrap();
        let mut vuelta = [0u16; 64];
        let leidos = desmontar(&frags[..c], &corta, &mut vuelta).unwrap();
        assert_eq!(&vuelta[..leidos], &src[..n]);
    }

    // -- LO QUE ESTE FICHERO EXISTE PARA IMPEDIR ---------------------------

    #[test]
    fn una_cadena_con_la_suma_cambiada_se_descarta() {
        let nombre = "Documento importante.pdf";
        let n = largo(nombre);
        let src = u16s(nombre);
        let mia = *b"DOCUME~1PDF";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..n], &mia, &mut frags).unwrap();

        // La misma cadena, pero la entrada corta de al lado es OTRA. Eso pasa
        // de verdad: restos de un borrado que un sistema viejo dejo sin
        // limpiar, con una entrada nueva reusando el hueco de detras.
        let ajena = *b"OTRACO~1TXT";
        let mut vuelta = [0u16; 64];
        assert_eq!(
            desmontar(&frags[..c], &ajena, &mut vuelta),
            Err(NoVale::SumaMala),
            "sin esto se devuelve el nombre de un archivo que ya no existe"
        );
    }

    #[test]
    fn una_cadena_sin_cabeza_no_da_un_nombre_cortado() {
        let nombre = "Presupuesto anual 2026.xlsx";
        let n = largo(nombre);
        let src = u16s(nombre);
        let corta = *b"PRESUP~1XLS";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..n], &corta, &mut frags).unwrap();

        // Se pierde el primero, que es el que lleva el bit ULTIMO.
        let mut vuelta = [0u16; 64];
        assert_eq!(
            desmontar(&frags[1..c], &corta, &mut vuelta),
            Err(NoVale::SinCabeza),
            "media cadena daria medio nombre, y medio nombre parece bueno"
        );
    }

    #[test]
    fn un_orden_que_no_baja_de_uno_en_uno_se_rechaza() {
        let nombre = "Presupuesto anual 2026.xlsx";
        let n = largo(nombre);
        let src = u16s(nombre);
        let corta = *b"PRESUP~1XLS";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..n], &corta, &mut frags).unwrap();

        frags[1][0] = 3; // deberia ser 2
        let mut vuelta = [0u16; 64];
        assert_eq!(desmontar(&frags[..c], &corta, &mut vuelta), Err(NoVale::OrdenRoto));
    }

    #[test]
    fn una_entrada_que_no_es_fragmento_se_rechaza() {
        let nombre = "Documento largo aqui.txt";
        let n = largo(nombre);
        let src = u16s(nombre);
        let corta = *b"DOCUME~1TXT";
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..n], &corta, &mut frags).unwrap();

        frags[1][11] = 0x20; // Archive: es una entrada corta colada en medio
        let mut vuelta = [0u16; 64];
        assert_eq!(desmontar(&frags[..c], &corta, &mut vuelta), Err(NoVale::NoEsFragmento));
    }

    #[test]
    fn una_cadena_vacia_no_es_un_nombre_vacio() {
        let mut vuelta = [0u16; 64];
        assert_eq!(desmontar(&[], b"KERNEL  ELF", &mut vuelta), Err(NoVale::Vacia));
    }

    // -- El relleno, que es lo que hace que Windows lo lea -----------------

    #[test]
    fn el_relleno_es_cero_para_cerrar_y_efe_efe_para_el_resto() {
        let nombre = "hola.txt"; // 8 caracteres: sobran 5 huecos de 13
        let src = u16s(nombre);
        let corta = *b"HOLA    TXT";
        let mut frags = [[0u8; 32]; 1];
        montar(&src[..8], &corta, &mut frags).unwrap();

        // hueco 8 = el cierre, huecos 9..13 = relleno
        assert_eq!(car(&frags[0], 8), 0x0000, "el cierre va justo detras");
        for i in 9..POR_ENTRADA {
            assert_eq!(car(&frags[0], i), 0xFFFF, "el resto es 0xFFFF, no ceros");
        }
    }

    #[test]
    fn un_fragmento_tiene_el_atributo_imposible_que_lo_esconde_de_ms_dos() {
        let src = u16s("hola.txt");
        let mut frags = [[0u8; 32]; 1];
        montar(&src[..8], b"HOLA    TXT", &mut frags).unwrap();
        assert_eq!(frags[0][11], 0x0F, "RO|Oculto|Sistema|Etiqueta a la vez");
        assert_eq!(&frags[0][26..28], &[0, 0], "FstClusLO es cero en un fragmento");
        assert_eq!(frags[0][12], 0, "Type es cero");
    }

    // -- Cuando NO hace falta cadena ---------------------------------------

    #[test]
    fn un_nombre_que_ya_cabe_en_8_3_no_necesita_cadena() {
        assert!(cabe_en_8_3(&u16s("KERNEL.ELF")[..10], b"KERNEL  ELF"));
        assert!(cabe_en_8_3(&u16s("BOOTX64.EFI")[..11], b"BOOTX64 EFI"));
        assert!(cabe_en_8_3(&u16s("LEEME")[..5], b"LEEME      "));
        // Y cuando no cabe, se dice.
        assert!(!cabe_en_8_3(&u16s("Presupuesto.xlsx")[..16], b"PRESUP~1XLS"));
        // Las minusculas TAMPOCO caben: el 8.3 va en mayusculas, y `readme.txt`
        // necesita cadena (o el truco de NTRes, que es otra cosa).
        assert!(!cabe_en_8_3(&u16s("kernel.elf")[..10], b"KERNEL  ELF"));
    }

    // -- Los limites --------------------------------------------------------

    #[test]
    fn doscientos_cincuenta_y_cinco_caben_y_doscientos_cincuenta_y_seis_no() {
        let n255 = [b'a' as u16; 255];
        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&n255, b"AAAAAA~1   ", &mut frags).expect("255 es el limite");
        assert_eq!(c, 20, "255 / 13 redondeado hacia arriba");

        let n256 = [b'a' as u16; 256];
        assert_eq!(montar(&n256, b"AAAAAA~1   ", &mut frags), Err(NoVale::Larga));
    }

    #[test]
    fn un_destino_que_no_da_se_dice_en_vez_de_cortar() {
        let src = u16s("Presupuesto anual 2026.xlsx");
        let corta = *b"PRESUP~1XLS";
        let mut frags = [[0u8; 32]; 2]; // hacen falta 3
        assert_eq!(montar(&src[..27], &corta, &mut frags), Err(NoVale::NoCabe));

        let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
        let c = montar(&src[..27], &corta, &mut frags).unwrap();
        let mut corto = [0u16; 10]; // hacen falta 27
        assert_eq!(desmontar(&frags[..c], &corta, &mut corto), Err(NoVale::NoCabe));
    }

    #[test]
    fn la_cuenta_de_fragmentos_cuadra_con_lo_que_monta() {
        for n in 1..=255usize {
            let nombre = [b'x' as u16; 255];
            let mut frags = [[0u8; 32]; MAX_FRAGMENTOS];
            let c = montar(&nombre[..n], b"XXXXXX~1   ", &mut frags).unwrap();
            assert_eq!(c, fragmentos_para(n), "n = {n}");
        }
    }
}
