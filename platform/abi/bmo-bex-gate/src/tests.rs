//! **EL CORPUS DE MENTIRAS.**
//!
//! Cada prueba coge una imagen buena y le cambia UNA cosa, la que un fichero de
//! fuera podria traer cambiada. La imagen sigue midiendo lo mismo y sigue
//! pareciendo un `.bex`.
//!
//! ## Por que esto vale mas que antes
//!
//! Este corpus iba a ser una **red sobre una duplicacion**: pasarle los mismos
//! ficheros al validador del toolchain y al del kernel y exigir que coincidieran.
//! Eso caza la divergencia despues de escribirla.
//!
//! Con la decision en un solo sitio, es un test unitario y ya esta -- y lo que
//! demuestra lo heredan **los dos** consumidores sin escribir nada.

extern crate std;
use super::*;
use std::vec;
use std::vec::Vec;

/// Escribe un `.bex` a mano. Sin usar el escritor de `bmo-abi` **a proposito**:
/// si las pruebas de la puerta usaran el mismo codigo que fabrica los ficheros
/// buenos, comprobarian que el escritor es coherente consigo mismo, que no es la
/// pregunta. Un extranjero no usa nuestro escritor.
struct Imagen {
    flags: u32,
    entry: u64,
    secciones: Vec<(u8, u32, u64, u64, u64, u16)>, // kind, flags, off, fsize, msize, align
    total_size: u32,
    abi: (u8, u8),
    cpu: u16,
    arch: u8,
}

impl Imagen {
    /// Una imagen minima y VALIDA: codigo ejecutable y nada mas.
    fn buena() -> Self {
        Self {
            flags: FLAG_EJECUTABLE,
            entry: 0,
            // La tabla acaba en 48 + 48 = 96, asi que el codigo empieza en 512.
            secciones: vec![(CODE, SECCION_FLAG_EXEC, 512, 256, 256, 8)],
            total_size: 768,
            abi: (2, 0),
            cpu: 0,
            arch: ARCH_X86_64,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let n = self.secciones.len();
        let mut b = vec![0u8; 48 + n * 48];
        b[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        b[4..6].copy_from_slice(&1u16.to_le_bytes());
        b[8..12].copy_from_slice(&self.flags.to_le_bytes());
        b[12] = self.arch;
        b[13] = ENDIAN_LE;
        b[14..16].copy_from_slice(&self.cpu.to_le_bytes());
        b[16] = self.abi.0;
        b[17] = self.abi.1;
        b[24..32].copy_from_slice(&self.entry.to_le_bytes());
        b[32..40].copy_from_slice(&48u64.to_le_bytes());
        b[40..44].copy_from_slice(&(n as u32).to_le_bytes());
        b[44..48].copy_from_slice(&self.total_size.to_le_bytes());
        for (i, (k, f, off, fs, ms, al)) in self.secciones.iter().enumerate() {
            let e = 48 + i * 48;
            b[e] = *k;
            b[e + 4..e + 8].copy_from_slice(&f.to_le_bytes());
            b[e + 8..e + 16].copy_from_slice(&off.to_le_bytes());
            b[e + 16..e + 24].copy_from_slice(&fs.to_le_bytes());
            b[e + 24..e + 32].copy_from_slice(&ms.to_le_bytes());
            b[e + 40..e + 42].copy_from_slice(&al.to_le_bytes());
        }
        b
    }

    /// El veredicto de la puerta sobre esta imagen.
    fn puerta(&self) -> Result<(), Falta> {
        let b = self.bytes();
        revisar(&b, self.total_size as usize).map(|_| ())
    }
}

/// La fila que hace utiles a todas las demas: **una imagen buena PASA**. Un
/// verificador que dijera "no" siempre tambien cazaria todas las mentiras.
#[test]
fn una_imagen_buena_pasa() {
    assert_eq!(Imagen::buena().puerta(), Ok(()));
}

/// ** DOS SECCIONES PELEANDOSE POR LOS MISMOS BYTES.
///
/// Es la mentira que mas dano hace y la que ninguna comprobacion de limites ve:
/// las dos secciones caben en el fichero, y aun asi no pueden ser ciertas las
/// dos. Quien se lo cree monta un proceso donde los mismos bytes son codigo
/// ejecutable y datos escribibles a la vez.
#[test]
fn dos_secciones_no_pueden_pisarse() {
    let mut img = Imagen::buena();
    // `.code` en [512, 768) y `.data` en [640, 896): se pisan 128 bytes.
    img.secciones.push((DATA, 0, 640, 256, 256, 8));
    img.total_size = 1024;
    assert_eq!(img.puerta(), Err(Falta::SeccionesSeSolapan));
}

/// Pegadas SI vale: `[512, 768)` y `[768, 1024)` no comparten ni un byte. Sin
/// esta prueba, un `<=` en vez de un `<` rechazaria todo fichero bien empaquetado
/// y nadie sabria por que.
#[test]
fn dos_secciones_pegadas_no_se_pisan() {
    let mut img = Imagen::buena();
    img.secciones.push((DATA, 0, 768, 256, 256, 8));
    img.total_size = 1024;
    assert_eq!(img.puerta(), Ok(()));
}

/// La `Bss` no ocupa fichero, asi que su offset no apunta a ningun byte y **no
/// puede pisar a nadie**. Si contara, toda imagen con Bss seria rechazada.
#[test]
fn la_bss_no_pisa_a_nadie() {
    let mut img = Imagen::buena();
    img.secciones.push((BSS, 0, 512, 0, 4096, 8)); // mismo offset que el codigo
    assert_eq!(img.puerta(), Ok(()));
}

/// ** BANDERAS QUE CAMBIAN LO QUE SIGNIFICAN LAS SECCIONES.
///
/// Un `.bex` que dice `COMPRIMIDO` esta diciendo *"mis bytes no son los que van a
/// memoria"*. Ignorarlo es cargar el bloque en crudo y saltar a el.
#[test]
fn las_banderas_que_nadie_implementa_se_rechazan() {
    for flag in [FLAG_COMPRIMIDO, FLAG_RECARGABLE] {
        let mut img = Imagen::buena();
        img.flags |= flag;
        assert_eq!(
            img.puerta(),
            Err(Falta::PideAlgoQueNadieImplementa),
            "la bandera {flag:#x} tiene que rechazar"
        );
    }
}

/// ** Y UNA BANDERA DESCONOCIDA QUE NO CAMBIA NADA, SI PASA.
///
/// Es la otra mitad de la regla y sin ella el formato no podria crecer: `PIE` y
/// las demas que el escritor pone hoy (bits 8, 9, 10) no significan nada para
/// este lector y **no cambian lo que hacen las secciones**. Rechazarlas seria
/// negarse a cargar los `.bex` que el propio sistema fabrica.
#[test]
fn una_bandera_desconocida_e_inofensiva_pasa() {
    let mut img = Imagen::buena();
    img.flags |= 1 << 9;
    assert_eq!(img.puerta(), Ok(()));
}

/// Dice venir firmado y no trae firma. Es la mentira mas barata: un bit cambiado
/// a mano hace que un binario cualquiera parezca avalado por alguien.
#[test]
fn decir_que_viene_firmado_sin_firma_no_cuela() {
    let mut img = Imagen::buena();
    img.flags |= FLAG_FIRMADO;
    assert_eq!(img.puerta(), Err(Falta::CabeceraQueSeDesmiente));
}

/// Y con firma de verdad, pasa.
#[test]
fn decir_que_viene_firmado_con_firma_pasa() {
    let mut img = Imagen::buena();
    img.flags |= FLAG_FIRMADO;
    img.secciones.push((SIGNATURE, 0, 768, 128, 128, 8));
    img.total_size = 1024;
    assert_eq!(img.puerta(), Ok(()));
}

/// ** LA IMAGEN DECLARA SU PROPIO TAMANO, y se comprueba antes que la tabla.
///
/// Si faltan bytes, las secciones apuntan mas alla y la primera que se salga
/// contesta `SeccionFueraDelFichero` -- cierto, y la pista equivocada: manda a
/// mirar el FORMATO cuando lo que fallo es el TRANSPORTE.
#[test]
fn una_imagen_cortada_lo_dice_como_transporte() {
    let img = Imagen::buena();
    let b = img.bytes();
    assert_eq!(revisar(&b, 700).map(|_| ()), Err(Falta::ImagenIncompleta));
}

/// `total_size = 0` se acepta: las imagenes que un kernel EMBEBE no pasan por el
/// escritor y lo dejan sin poner. Exigirselo a quien nunca lo prometio seria
/// dejar de arrancar.
#[test]
fn sin_tamano_declarado_no_se_exige_nada() {
    let mut img = Imagen::buena();
    img.total_size = 0;
    let b = img.bytes();
    assert_eq!(revisar(&b, 768).map(|_| ()), Ok(()));
}

/// Una seccion que promete bytes fuera del fichero.
#[test]
fn una_seccion_no_puede_salirse_del_fichero() {
    let mut img = Imagen::buena();
    img.secciones[0].2 = 700; // offset 700 + 256 = 956 > 768
    assert_eq!(img.puerta(), Err(Falta::SeccionFueraDelFichero));
}

/// El punto de entrada fuera del codigo: saltaria a cualquier sitio.
#[test]
fn el_entry_no_puede_caer_fuera_del_codigo() {
    let mut img = Imagen::buena();
    img.entry = 999;
    assert_eq!(img.puerta(), Err(Falta::EntryFueraDelCodigo));
}

/// Una seccion de codigo sin el bit de ejecutable: o miente el tipo o miente la
/// bandera, y en cualquier caso no se mapea RX algo que no lo pide.
#[test]
fn el_codigo_tiene_que_declararse_ejecutable() {
    let mut img = Imagen::buena();
    img.secciones[0].1 = 0;
    assert_eq!(img.puerta(), Err(Falta::LaCodigoNoEsEjecutable));
}

/// Sin codigo no hay nada que ejecutar.
#[test]
fn sin_seccion_de_codigo_no_hay_programa() {
    let mut img = Imagen::buena();
    img.secciones[0].0 = DATA;
    assert_eq!(img.puerta(), Err(Falta::SinCodigo));
}

/// `file_size > mem_size` es una seccion que trae mas bytes de los que dice
/// ocupar: al copiarla se escribiria fuera de lo reservado.
#[test]
fn una_seccion_no_puede_traer_mas_de_lo_que_ocupa() {
    let mut img = Imagen::buena();
    img.secciones[0].4 = 128; // mem 128 < file 256
    assert_eq!(img.puerta(), Err(Falta::SeccionInvalida));
}

/// Alineacion que no es potencia de dos: la cuenta de redondeo del cargador da
/// basura silenciosa.
#[test]
fn la_alineacion_tiene_que_ser_potencia_de_dos() {
    let mut img = Imagen::buena();
    img.secciones[0].5 = 300;
    assert_eq!(img.puerta(), Err(Falta::SeccionInvalida));
}

/// Extensiones de CPU que el sistema no sabe preservar en un cambio de contexto:
/// un programa con AVX se corromperia en silencio a la primera interrupcion.
#[test]
fn una_extension_de_cpu_desconocida_se_rechaza() {
    let mut img = Imagen::buena();
    img.cpu = 1;
    assert_eq!(img.puerta(), Err(Falta::ExtensionDeCpuQueNoSePreserva));
}

#[test]
fn lo_basico_de_la_cabecera() {
    let mut otra_arch = Imagen::buena();
    otra_arch.arch = 0x02;
    assert_eq!(otra_arch.puerta(), Err(Falta::OtraArquitectura));

    let mut otro_abi = Imagen::buena();
    otro_abi.abi = (9, 0);
    assert_eq!(otro_abi.puerta(), Err(Falta::OtraVersionDelAbi));

    let mut no_ejecutable = Imagen::buena();
    no_ejecutable.flags = 0;
    assert_eq!(no_ejecutable.puerta(), Err(Falta::NoEsEjecutable));

    let b = Imagen::buena().bytes();
    let mut magia = b.clone();
    magia[0] = b'X';
    assert_eq!(revisar(&magia, 768).map(|_| ()), Err(Falta::CabeceraInvalida));

    let mut version = b.clone();
    version[4] = 9;
    assert_eq!(revisar(&version, 768).map(|_| ()), Err(Falta::CabeceraInvalida));

    assert_eq!(
        revisar(&b[..20], 768).map(|_| ()),
        Err(Falta::NoLlegaNiALaCabecera)
    );
}

/// ** LA DISTINCION QUE VALE UNA TARDE: la tabla cabe en el FICHERO pero no en lo
/// que se LEYO.
///
/// No es lo mismo que una imagen mal formada. Quien llama puede traer mas bytes y
/// volver a preguntar, y por eso son dos faltas distintas y no una.
#[test]
fn tabla_que_no_cabe_en_lo_leido_no_es_tabla_invalida() {
    let img = Imagen::buena();
    let b = img.bytes(); // 96 bytes: cabecera + una entrada
    assert_eq!(revisar(&b[..60], 768).map(|_| ()), Err(Falta::TablaFueraDeLoLeido));
    // Y si tampoco cabe en el fichero, eso ya es otra cosa.
    assert_eq!(revisar(&b, 60).map(|_| ()), Err(Falta::ImagenIncompleta));
}

/// Mas secciones de las que la tabla admite. El numero viene del disco, asi que
/// se comprueba antes de multiplicarlo por nada.
#[test]
fn demasiadas_secciones() {
    let img = Imagen::buena();
    let mut b = img.bytes();
    b[40..44].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(revisar(&b, 768).map(|_| ()), Err(Falta::DemasiadasSecciones));
}

/// ** Y NINGUNA MENTIRA PUEDE HACER QUE EL LECTOR SE SALGA.
///
/// El caso que de verdad importa: estos bytes vienen del disco, y el disco es de
/// quien tenga la maquina. Se truca cada campo de la cabecera con el valor mas
/// hostil que cabe, y **ninguno puede acabar en un panic** -- el crate compila
/// con `#![forbid(unsafe_code)]` para que eso no sea una promesa sino una
/// imposibilidad, pero un indexado fuera de rango tambien mata en Rust seguro.
#[test]
fn ningun_campo_trucado_puede_reventar_al_lector() {
    let base = Imagen::buena().bytes();
    for campo in [24usize, 32, 40, 44] {
        for valor in [u32::MAX, 0x8000_0000, 1, 0] {
            let mut b = base.clone();
            b[campo..campo + 4].copy_from_slice(&valor.to_le_bytes());
            // Solo tiene que NO reventar. Que conteste no nos importa aqui.
            let _ = revisar(&b, 768);
            let _ = revisar(&b, usize::MAX);
            let _ = revisar(&b[..50], 768);
        }
    }
    // Y la tabla de secciones entera, byte a byte.
    for i in 48..base.len() {
        let mut b = base.clone();
        b[i] = 0xFF;
        let _ = revisar(&b, 768);
    }
}

/// `hasta_donde_hace_falta` no cuenta lo que el cargador no toca. Es el escalon 2
/// de `LA_RAM.md` en una funcion: un paquete con un WAD dentro mide seis megas y
/// lo que hay que leer para ejecutarlo son ochocientos kilos.
#[test]
fn los_recursos_no_cuentan_para_lo_que_hay_que_leer() {
    let mut img = Imagen::buena();
    const RESOURCES: u8 = 0x0B;
    img.secciones.push((RESOURCES, 0, 768, 1_000_000, 1_000_000, 8));
    img.total_size = 1_000_768;
    let b = img.bytes();
    let rev = revisar(&b, img.total_size as usize).expect("tiene que pasar");
    assert_eq!(
        rev.hasta_donde_hace_falta(),
        768,
        "el millon de bytes de recursos NO hay que traerlos para ejecutar"
    );
}

// -- ** `reloc_cabe`: la regla que el cargador no tenia (2026-08-25) ---------
//
// Cinco casos, y el que importa es el tercero: **no se sale de la imagen, se
// mete en la seccion de al lado.** Ese es el que el cargador dejaba pasar,
// porque su unica comprobacion era "cae en la pagina que estoy parcheando", y
// caia.

/// El caso bueno, y el borde exacto: ocho bytes que acaban justo en el final de
/// la seccion SI caben.
///
/// # *** ESTE BORDE NO ES TEORICO: ES DOOM, Y NO SOBRA NI UN BYTE
///
/// Se midieron las 1.285 relocations de `doom.bex` contra esta regla antes de
/// cablearla, para saber si rechazaba algo que hoy funciona. Ninguna. Pero la
/// mas ajustada sale asi:
///
/// ```text
///    .data de DOOM        151.560 bytes = 0x25008
///    la reloc #706        offset 0x25000, ocho bytes, acaba en 0x25008
///    holgura              CERO
/// ```
///
/// > **Un `<` en vez de un `<=` no habria fallado una prueba: habria dejado de
/// > cargar DOOM.** Y el sintoma no seria "relocation invalida", seria que el
/// > programa mas grande del arbol deja de arrancar por un byte.
///
/// El codegen pone punteros al final de `.data` porque es donde caen; que la
/// ultima acabe justo en el limite no es casualidad, es lo normal.
#[test]
fn una_reloc_que_acaba_justo_en_el_borde_cabe() {
    assert!(super::reloc_cabe(0x3F8, 8, 0x400, 0x400), "0x3F8 + 8 = 0x400, y la seccion son 0x400");
    assert!(super::reloc_cabe(0, 8, 0x400, 0x400));
    // Los numeros de verdad de `doom.bex`, reloc #706. Si esto se pone rojo,
    // DOOM no arranca.
    assert!(
        super::reloc_cabe(0x25000, 8, 0x25008, 0x25008),
        "la reloc mas ajustada de DOOM: holgura CERO y es legal"
    );
}

/// Un byte mas alla del borde NO cabe. Es el reverso del de arriba y va al lado
/// a proposito: los dos juntos fijan el `<=` y ninguno de los dos solo lo hace.
#[test]
fn un_solo_byte_de_mas_no_cabe() {
    assert!(!super::reloc_cabe(0x3F9, 8, 0x400, 0x400), "acabaria en 0x401");
}

/// *** EL CASO QUE ESTO EXISTE PARA CAZAR.
///
/// Una `.data` de 0x400 con una reloc en 0x9000. No se sale de la imagen: las
/// secciones van seguidas, asi que **cae dentro de otra**. El cargador
/// comprobaba que el destino estuviera en la pagina que estaba parcheando --lo
/// estaba-- y escribia ocho bytes en la seccion del vecino.
#[test]
fn una_reloc_que_apunta_a_la_seccion_de_al_lado_no_cabe() {
    assert!(!super::reloc_cabe(0x9000, 8, 0x400, 0x400));
}

/// Una `.bss` no ocupa en el fichero y si en memoria, y se parchea sobre lo que
/// hay EN MEMORIA. Con el tope puesto en `file_size` esto diria que no, y
/// rechazaria programas correctos.
#[test]
fn manda_el_tamano_en_memoria_y_no_el_del_fichero() {
    assert!(super::reloc_cabe(0x100, 8, 0, 0x1000), "una .bss: 0 en fichero, 0x1000 en memoria");
}

/// **El desbordamiento es un NO, no un panico.** `offset` viene del fichero, o
/// sea de fuera: `u64::MAX` es un valor que alguien puede escribir a mano, y un
/// `+` normal en `release` daria la vuelta y contestaria que SI cabe.
#[test]
fn un_offset_imposible_no_da_la_vuelta() {
    assert!(!super::reloc_cabe(u64::MAX, 8, 0x400, 0x400));
    assert!(!super::reloc_cabe(u64::MAX - 3, 8, u64::MAX, u64::MAX));
}
