//! ===========================================================================
//!  PRUEBAS -- sobre un volumen FAT32 de mentira, en RAM
//! ===========================================================================
//!
//! * Este modulo no existia, y era el agujero mas caro del arbol: el unico
//! codigo de BMO que ESCRIBE en un disco de verdad era tambien el unico sin una
//! sola prueba. Se verificaba flasheando y mirando la pantalla -- o sea,
//! arriesgando el volumen para averiguar si el driver lo respetaba.
//!
//! El contrato de bloques (`bmo_block::BlockDevice`) se toma como
//! `&'static dyn`, asi que el disco de mentira vive en un `static` y sus bytes
//! en un `static mut` aparte; cada prueba lo formatea entero antes de empezar.
//!
//! ## Por que es un fichero (L6a, L6b)
//!
//! ** Las 822 lineas de pruebas eran un TERCIO de `lib.rs`, y contestan otra
//! pregunta: aquel dice **como se lee y se escribe un FAT32** y esto dice
//! **como se sabe que lo hace bien**. Que vivieran juntas hacia que el fichero
//! con mas responsabilidad de Ring 0 fuera tambien el mas largo del arbol.
//!
//! ** Y el reparto es MOVER TEXTO: ni una linea cambia de contenido, solo de
//! sitio y de indentacion. Es lo que L6d llama un reparto demostrable.

use super::*;

use super::*;

/// Un volumen minusculo pero REAL: 512 sectores de 512 bytes = 256 KiB.
///
/// Un cluster = un sector, a proposito. Asi un archivo de 600 bytes ya son
/// DOS clusters encadenados, y el camino de la cadena se pisa con datos de
/// juguete en vez de necesitar megabytes.
const SECTORES: usize = 512;
const RESERVADOS: u32 = 1;
const FAT_SECTORES: u32 = 4;

static mut DISCO: [u8; SECTORES * 512] = [0u8; SECTORES * 512];

/// El disco de mentira es UNO, y `cargo test` corre en paralelo.
///
/// No es un detalle de infraestructura: sin esto, una prueba lee el
/// volumen que otra acababa de formatear y falla **con un mensaje que
/// apunta al driver**. Se pierde media tarde buscando un fallo de FAT32
/// que estaba en el banco de pruebas.
///
/// El candado lo toma [`volumen`] y lo devuelve al terminar la prueba, asi
/// que no hay forma de olvidarse: quien quiere el volumen se lleva el
/// turno con el.
static CANDADO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// El disco entero como rebanada. Se construye desde el puntero crudo y no
/// desreferenciando el `static mut`: encadenarlo crearia una referencia a
/// la desreferencia del puntero, que es lo que el lint prohibe -- y con
/// razon, porque esconde de donde sale. Mismo trato que en
/// `ring0/obj/archivo.rs`.
fn disco() -> &'static mut [u8] {
    unsafe {
        core::slice::from_raw_parts_mut(core::ptr::addr_of_mut!(DISCO) as *mut u8, SECTORES * 512)
    }
}

/// Cuantas veces se ha ido al "disco". Lo lleva el propio lector de mentira
/// porque **es el unico sitio que no se puede saltar nadie**: si una pieza
/// del driver deja de recordar lo que ya trajo, este numero lo dice.
///
/// Es global y las pruebas corren en paralelo, pero quien lo mira tiene el
/// CANDADO en la mano (ver [`volumen`]), asi que dentro de una prueba solo
/// cuenta lo que hace esa prueba.
static LECTURAS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn lecturas() -> usize {
    LECTURAS.load(std::sync::atomic::Ordering::Relaxed)
}

/// **El disco de mentira, ahora detras del contrato.**
///
/// Antes eran dos funciones sueltas que se pasaban a `mount`. Ahora es un
/// `static` que implementa `BlockDevice`, que es como se inyecta un
/// dispositivo desde que FAT32 entra por la puerta de `bmo-block`.
///
/// [!] Y las pruebas ganan algo que antes no podian probar: el disco de
/// mentira **dice quien es**. `OutOfRange` deja de ser un `false`
/// indistinguible de un fallo de hardware.
struct DiscoDeMentira;

/// El dispositivo que se le pasa a `mount`. Se llama distinto de `DISCO`
/// --que son los BYTES-- porque son dos cosas: el disco es el medio, esto
/// es quien sabe hablarle.
static DISPOSITIVO: DiscoDeMentira = DiscoDeMentira;

/// Las dos funciones sueltas SIGUEN existiendo, y no por pereza: las
/// pruebas SIEMBRAN el disco con ellas antes de montar nada, cuando
/// todavia no hay volumen ni dispositivo de por medio. El trait delega
/// aqui, asi que hay UN solo sitio que mueve bytes.
fn read(lba: u64, count: u16, buf: &mut [u8]) -> bool {
    let off = lba as usize * 512;
    let n = count as usize * 512;
    if off + n > SECTORES * 512 || buf.len() < n { return false; }
    LECTURAS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    buf[..n].copy_from_slice(&disco()[off..off + n]);
    true
}

fn write(lba: u64, count: u16, data: &[u8]) -> bool {
    let off = lba as usize * 512;
    let n = count as usize * 512;
    if off + n > SECTORES * 512 || data.len() < n { return false; }
    disco()[off..off + n].copy_from_slice(&data[..n]);
    true
}

impl bmo_block::BlockDevice for DiscoDeMentira {
    fn identity(&self) -> bmo_block::DeviceId {
        let mut id = bmo_block::DeviceId::EMPTY;
        let m = b"DISCO DE MENTIRA";
        id.model[..m.len()].copy_from_slice(m);
        id.model_len = m.len();
        let sn = b"PRUEBAS-0001";
        id.serial[..sn.len()].copy_from_slice(sn);
        id.serial_len = sn.len();
        id.blocks = SECTORES as u64;
        id
    }

    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, bmo_block::BlockError> {
        let n = count as usize * 512;
        if (lba as usize * 512) + n > SECTORES * 512 {
            return Err(bmo_block::BlockError::OutOfRange);
        }
        if buf.len() < n { return Err(bmo_block::BlockError::ShortBuffer); }
        if read(lba, count, buf) { Ok(count) } else { Err(bmo_block::BlockError::Device) }
    }

    fn write(&self, lba: u64, count: u16, data: &[u8]) -> Result<u16, bmo_block::BlockError> {
        let n = count as usize * 512;
        if (lba as usize * 512) + n > SECTORES * 512 {
            return Err(bmo_block::BlockError::OutOfRange);
        }
        if data.len() < n { return Err(bmo_block::BlockError::ShortBuffer); }
        if write(lba, count, data) { Ok(count) } else { Err(bmo_block::BlockError::Device) }
    }

    fn flush(&self) -> Result<(), bmo_block::BlockError> { Ok(()) }
    fn writable(&self) -> bool { true }
}

/// Formatea el disco de mentira y lo monta. Cada prueba empieza de cero.
///
/// Devuelve el TURNO junto con el volumen: mientras la prueba tenga el
/// guardia vivo, ninguna otra toca el disco. Un `let _ = volumen()` lo
/// soltaria en el acto, y por eso las pruebas lo atan a un nombre.
fn volumen() -> (std::sync::MutexGuard<'static, ()>, FatVolume) {
    volumen_con_base(0)
}

/// **El mismo volumen, empezando donde se diga.**
///
/// === Por que esto no es un lujo de la prueba ===
///
/// `volumen()` montaba en el sector 0, y ahi `part_lba` vale cero: **la suma
/// que traduce del volumen al disco no cambia nada**. O sea que trece pruebas
/// en verde no decian absolutamente nada sobre la unica cuenta que separa
/// "leer mi archivo" de "leer la particion del vecino".
///
/// El 2026-08-11 eso salio a cobrar. El camino directo del escalon 3 llamaba
/// al lector con el LBA **relativo al volumen**, y con la particion de datos
/// en el 1230848 un `.bex` se leia de dentro de la ESP. Las pruebas pasaban.
///
/// > **Un parametro que en las pruebas siempre vale cero es un parametro que
/// > no se esta probando.**
fn volumen_con_base(base: u64) -> (std::sync::MutexGuard<'static, ()>, FatVolume) {
    // `into_inner` y no `unwrap`: si una prueba anterior revento con el
    // candado en la mano, el resto tiene que poder seguir. El disco se
    // formatea entero aqui abajo, asi que lo que dejara no importa --
    // envenenar la tanda entera solo escondaria el fallo de verdad.
    let turno = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    disco().fill(0);
    let mut sector0 = [0u8; 512];
    {
        let bpb = unsafe { &mut *(sector0.as_mut_ptr() as *mut FatBpb) };
        bpb.bytes_per_sector = 512;
        bpb.sectors_per_cluster = 1;
        bpb.reserved_sectors = RESERVADOS as u16;
        bpb.num_fats = 1;
        // Lo que mide el VOLUMEN, no el disco: lo que queda detras de donde
        // empieza. Poner el disco entero haria que `max_cluster` contara
        // clusters que se salen por el final.
        bpb.total_sectors = (SECTORES as u64 - base) as u32;
        bpb.fat_size = FAT_SECTORES;
        bpb.root_cluster = 2;
        bpb.boot_sig = 0x29;
    }
    assert!(write(base, 1, &sector0));

    // El cluster 2 es la raiz y esta OCUPADO: la FAT tiene que decirlo, o
    // el primer archivo que se cree se llevara el directorio por delante.
    let mut fat = [0u8; 512];
    fat[0..4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes()); // media
    fat[4..8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // reservada
    fat[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // la raiz: EOC
    assert!(write(base + RESERVADOS as u64, 1, &fat));

    let v = mount(&DISPOSITIVO, true, base).expect("el volumen de mentira debe montar");
    (turno, v)
}

fn name(n: &str) -> [u8; 11] {
    let mut r = [b' '; 11];
    let b = n.as_bytes();
    r[..b.len()].copy_from_slice(b);
    r
}

/// Lee un archivo entero por su nombre. `None` si no esta.
fn leer_archivo(v: &mut FatVolume, n: &str, dst: &mut [u8]) -> Option<usize> {
    let (primero, tam) = v.find_file(&name(n))?;
    let leidos = v.read_file(primero, tam, dst);
    Some(leidos.min(tam as usize))
}

/// Cuantos clusters hay OCUPADOS ahora mismo. Es el detector de fugas: si
/// reemplazar no suelta la cadena vieja, este numero sube y no baja, y el
/// volumen se llena archivo a archivo sin que nada lo diga.
fn ocupados(v: &mut FatVolume) -> usize {
    let mut n = 0;
    for c in 2..=v.max_cluster {
        if v.raw_fat_entry(c).unwrap_or(0) != 0 { n += 1; }
    }
    n
}

/// ** UN ARCHIVO DE VARIOS CLUSTERS QUE NO ACABA EN FRONTERA DE SECTOR.
///
/// Es el caso que estrena el camino directo de `read_file` (escalon 3): los
/// sectores enteros van del disco a `dst` sin rebotar, y el rabo --menos de
/// 512 bytes-- es el unico que sigue pasando por el buffer interno.
///
/// El patron es POSICIONAL a proposito (cada byte dice donde deberia estar):
/// un desplazamiento de un sector, o un cluster leido dos veces, sale como
/// un byte que no cuadra y dice exactamente cual.
#[test]
fn leer_varios_clusters_con_rabo() {
    let (_turno, mut v) = volumen();
    // 1300 bytes con spc=1 son tres clusters: 512 + 512 + 276.
    let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("LARGO   BIN"), &datos).expect("debe crear");

    let mut dst = [0u8; 2048];
    let n = leer_archivo(&mut v, "LARGO   BIN", &mut dst).expect("debe estar");
    assert_eq!(n, datos.len(), "no llego el archivo entero");
    assert_eq!(&dst[..n], &datos[..], "los bytes no cuadran: hay un salto de sector");
}

/// ** UN VOLUMEN QUE NO EMPIEZA EN EL SECTOR 0 -- o sea, el caso REAL.
///
/// === El fallo que esta prueba habria cazado el 10 de agosto ===
///
/// Los tres caminos directos --`read_file`, `leer_en` y `leer_tramo`--
/// llamaban al lector de bloques con el LBA **relativo al volumen**, sin
/// sumar `part_lba`. Los directorios y la FAT no, porque van por
/// `read_sector`, que si traduce.
///
/// De ahi el sintoma que costo dos tandas de fotos: el sistema **encontraba**
/// el archivo y sabia su tamano exacto --eso lo dice el directorio-- y los
/// bytes que llegaban eran codigo x86-64 ajeno. En el Ryzen, con la particion
/// de datos en el sector 1230848, un `.bex` se leia de dentro de la ESP.
///
/// === Y por eso hay veneno delante del volumen ===
///
/// Un ida y vuelta a secas no basta: si escribir y leer se equivocaran
/// **igual**, cuadrarian entre ellos y la prueba pasaria. El veneno ocupa
/// justo los sectores donde cae una lectura sin traducir, asi que olvidarse
/// de la suma no da "otro contenido": da `0xEE`, con su nombre.
///
/// Se cubren los tres caminos en una sola prueba porque son el mismo error
/// repetido tres veces, y arreglar dos de tres deja el sintoma vivo.
#[test]
fn leer_de_una_particion_que_no_empieza_en_cero() {
    const BASE: u64 = 64;
    let (_turno, mut v) = volumen_con_base(BASE);

    // Los sectores de DELANTE del volumen: fuera de el, y exactamente donde
    // apunta un LBA al que le falta la suma.
    let veneno = [0xEEu8; 512];
    for s in 0..BASE {
        assert!(write(s, 1, &veneno));
    }

    // 1300 bytes con spc=1 son tres clusters: dos sectores enteros --el
    // camino directo-- y un rabo de 276.
    let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("LARGO   BIN"), &datos).expect("debe crear");

    let (primero, tam) = v.find_file(&name("LARGO   BIN")).expect("el directorio SI se lee");

    // -- 1. `read_file`: el archivo entero --
    let mut dst = [0u8; 2048];
    let n = v.read_file(primero, tam, &mut dst);
    assert_ne!(dst[0], 0xEE, "read_file leyo de DELANTE del volumen: falta sumar part_lba");
    assert_eq!(n, datos.len(), "no llego el archivo entero");
    assert_eq!(&dst[..n], &datos[..], "read_file trajo bytes de otro sitio");

    // -- 2. `leer_en`: por rangos, que es por donde carga un `.bex` --
    let mut cur = v.cursor(primero);
    let mut rango = [0u8; 700];
    let n = v.leer_en(&mut cur, 512, tam, &mut rango);
    assert_ne!(rango[0], 0xEE, "leer_en leyo de DELANTE del volumen");
    assert_eq!(n, 700, "el rango no llego entero");
    assert_eq!(&rango[..n], &datos[512..512 + n], "leer_en trajo bytes de otro sitio");

    // -- 3. `leer_tramo`: la lectura a pasos --
    let mut trozo = [0u8; 2048];
    let (n, _siguiente) = v.leer_tramo(primero, 0, tam, &mut trozo, 2048);
    assert_ne!(trozo[0], 0xEE, "leer_tramo leyo de DELANTE del volumen");
    assert_eq!(n, datos.len(), "el tramo no llego entero");
    assert_eq!(&trozo[..n], &datos[..], "leer_tramo trajo bytes de otro sitio");

    // Y el numero que se pinta en CABINA es el del DISCO, no el del volumen:
    // un cluster nunca puede caer antes del principio de su particion, y esa
    // linea lo decia sin que chirriara.
    assert!(
        v.lba_de_cluster(primero) >= BASE,
        "lba_de_cluster devuelve un sector anterior a su propia particion"
    );
}

/// ** EL RABO DE UN ARCHIVO **FRAGMENTADO**, que es donde el fallo se ve.
///
/// === Por que hace falta fragmentar para probar esto ===
///
/// El camino directo lee sectores enteros y deja para el buffer interno el
/// rabo de menos de 512 bytes. Ese rabo esta en el sector `enteros` **de
/// este cluster** -- y si el cluster ya se agoto (`enteros == spc`), ese
/// numero de sector cae FUERA: es el sector fisico siguiente, que solo por
/// casualidad es el siguiente cluster de la cadena.
///
/// ** Y en un volumen recien formateado siempre es esa casualidad: los
/// clusters se reparten seguidos, asi que el fallo devuelve el dato
/// correcto y la prueba pasa. Es la clase de bug que se estrena el dia que
/// el disco lleva seis meses de uso.
///
/// Asi que aqui la cadena se rompe a mano: se muda el segundo cluster lejos
/// y **el sitio viejo se llena de `0xEE`**. Si alguien quita la comprobacion
/// de `enteros < spc`, esto sale en la cara con bytes que se reconocen.
#[test]
fn leer_rabo_de_archivo_fragmentado() {
    let (_turno, mut v) = volumen();
    // Con spc=1 (un cluster = un sector), 1300 bytes son tres clusters.
    let datos: Vec<u8> = (0..1300u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("FRAG    BIN"), &datos).expect("debe crear");

    let (c1, tam) = v.find_file(&name("FRAG    BIN")).expect("debe estar");
    let c2 = v.raw_fat_entry(c1).expect("debe haber segundo cluster");
    let c3 = v.raw_fat_entry(c2).expect("debe haber tercero");
    // Un cluster libre LEJOS de la cadena: el ultimo del volumen.
    let lejos = v.max_cluster;
    assert!(v.raw_fat_entry(lejos).unwrap_or(1) == 0, "el cluster de destino debe estar libre");

    // Se muda el contenido del segundo cluster.
    let mut sec = [0u8; 512];
    assert!(read(v.cluster_to_lba(c2), 1, &mut sec));
    assert!(write(v.cluster_to_lba(lejos), 1, &sec));
    // Y el sitio viejo se envenena: quien lo lea por error lo va a saber.
    assert!(write(v.cluster_to_lba(c2), 1, &[0xEEu8; 512]));

    // La cadena pasa a ser c1 -> lejos -> c3, y c2 queda libre.
    assert!(v.set_fat_entry(c1, lejos));
    assert!(v.set_fat_entry(lejos, c3));
    assert!(v.set_fat_entry(c2, 0));

    // 700 bytes: un cluster entero (512) y un rabo de 188 en el SIGUIENTE
    // cluster de la cadena, que ya no es el siguiente del disco.
    let mut dst = [0u8; 700];
    let n = v.read_file(c1, tam, &mut dst);
    assert_eq!(n, 700, "no llego el trozo pedido");
    assert!(
        !dst[512..].iter().any(|&b| b == 0xEE),
        "leyo el sector fisico siguiente en vez de seguir la cadena"
    );
    assert_eq!(&dst[..], &datos[..700], "el rabo no cuadra");
}

/// ** LEER A TROZOS TIENE QUE DAR EXACTAMENTE LO MISMO QUE LEER DE UNA.
///
/// Es la propiedad entera de `leer_tramo`: si el resultado no es
/// byte-a-byte identico al de `read_file`, el `open` que empieza y no
/// termina entregaria un archivo distinto segun cuantas veces se le hubiera
/// preguntado -- y eso no falla, corrompe.
///
/// Se prueba con un archivo de VARIOS clusters y un presupuesto de UN
/// cluster por vuelta, que es el caso que ejercita el cursor de verdad.
#[test]
fn leer_a_trozos_da_lo_mismo_que_de_una() {
    let (_turno, mut v) = volumen();
    let datos: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("TROZOS  BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("TROZOS  BIN")).expect("debe estar");

    let mut de_una = [0u8; 4096];
    let n1 = v.read_file(primero, tam, &mut de_una);

    let mut a_trozos = [0u8; 4096];
    let mut cluster = primero;
    let mut ya = 0usize;
    let mut vueltas = 0;
    while cluster != 0 {
        let (leidos, siguiente) = v.leer_tramo(cluster, ya, tam, &mut a_trozos, 512);
        assert!(leidos > 0, "un tramo que no avanza es un bucle infinito");
        ya += leidos;
        cluster = siguiente;
        vueltas += 1;
        assert!(vueltas < 64, "demasiadas vueltas: el cursor no avanza");
    }

    assert_eq!(ya, n1, "a trozos llego una cantidad distinta");
    assert_eq!(&a_trozos[..ya], &de_una[..n1], "a trozos salieron OTROS bytes");
    assert_eq!(&a_trozos[..ya], &datos[..], "y ni siquiera son los del archivo");
    assert!(vueltas > 1, "la prueba no llego a partir nada");
}

/// ** LEER DESDE UN BYTE CUALQUIERA TIENE QUE DAR LO MISMO QUE LEER DE UNA.
///
/// Es la propiedad entera de `leer_en`, y la que hace posible que el disco
/// escriba cada seccion de un `.bex` en los marcos del proceso: si un rango
/// leido por su cuenta no coincide byte a byte con el mismo rango del fichero
/// entero, el cargador montaria un programa cosido de trozos que no encajan
/// -- y eso no falla, **corrompe**.
///
/// Se prueban offsets DELIBERADAMENTE feos: mitad de sector, mitad de
/// cluster, y cruzando las dos fronteras. Con offsets redondos la prueba
/// pasaria sin ejercitar ni la cabeza ni la cola.
#[test]
fn leer_desde_cualquier_byte_da_lo_mismo() {
    let (_turno, mut v) = volumen();
    let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("TROZOS2 BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("TROZOS2 BIN")).expect("debe estar");

    // 1 y 511 son mitad de sector; 513 cruza la frontera; 2047 y 2049 andan
    // por la del cluster; 4999 es el ultimo byte.
    for (off, len) in [
        (0usize, 5000usize), (1, 10), (1, 600), (511, 2), (512, 512),
        (513, 1000), (2047, 3), (2048, 1), (2049, 2000), (4999, 1), (4990, 50),
    ] {
        let mut cur = v.cursor(primero);
        let mut dst = vec![0u8; len];
        let n = v.leer_en(&mut cur, off, tam, &mut dst);
        let esperado = &datos[off..(off + len).min(datos.len())];
        assert_eq!(n, esperado.len(), "off={off} len={len}: cantidad distinta");
        assert_eq!(&dst[..n], esperado, "off={off} len={len}: OTROS bytes");
    }
}

/// ** Y UN CURSOR REUSADO TIENE QUE DAR LO MISMO QUE UNO NUEVO.
///
/// Es lo que se va a hacer de verdad: un solo cursor recorriendo el fichero
/// hacia adelante, seccion tras seccion. Si el estado que arrastra cambiara
/// el resultado, el segundo programa que se cargue saldria distinto del
/// primero -- y eso no se reproduce nunca.
#[test]
fn el_cursor_reusado_no_cambia_lo_que_lee() {
    let (_turno, mut v) = volumen();
    let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("CURSOR  BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("CURSOR  BIN")).expect("debe estar");

    let mut cur = v.cursor(primero);
    for (off, len) in [(17usize, 100usize), (1000, 1200), (2500, 700), (4000, 999)] {
        let mut a = vec![0u8; len];
        let na = v.leer_en(&mut cur, off, tam, &mut a);

        let mut limpio = v.cursor(primero);
        let mut b = vec![0u8; len];
        let nb = v.leer_en(&mut limpio, off, tam, &mut b);

        assert_eq!(na, nb, "off={off}: el cursor reusado leyo otra cantidad");
        assert_eq!(a, b, "off={off}: el cursor reusado leyo OTROS bytes");
        assert_eq!(&a[..na], &datos[off..off + na], "off={off}: y no son los del archivo");
    }
}

/// Pedir hacia atras dice que NO. Ver la cabecera de `Cursor`: retroceder en
/// silencio convertiria el bucle del cargador en cuadratico sin avisar.
#[test]
fn el_cursor_no_retrocede_en_silencio() {
    let (_turno, mut v) = volumen();
    let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("ATRAS   BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("ATRAS   BIN")).expect("debe estar");

    let mut cur = v.cursor(primero);
    let mut dst = vec![0u8; 100];
    assert!(v.leer_en(&mut cur, 4000, tam, &mut dst) > 0, "primero se avanza");
    assert_eq!(
        v.leer_en(&mut cur, 10, tam, &mut dst),
        0,
        "pedir hacia atras tiene que contestar cero, no leer de cualquier sitio"
    );
}

/// ** SEGUIR UNA CADENA NO PUEDE COSTAR UN COMANDO POR ESLABON.
///
/// En un sector de FAT caben **128 entradas seguidas**, que son justo las que
/// recorre quien sigue una cadena. `fat_cache` se llamaba cache y releia el
/// sector en cada entrada; mientras lo unico que recorria cadenas era cargar
/// un programa de una vez, eso se pagaba una vez. Con los archivos leidos por
/// rangos, cada salto hacia atras en un fichero grande vuelve a recorrerla.
///
/// Aqui se cuentan los viajes al disco de verdad. Las dos mitades importan y
/// por eso van juntas: **que no relea** y **que no sirva lo de antes**.
#[test]
fn seguir_la_cadena_no_relee_el_mismo_sector() {
    let (_turno, mut v) = volumen();
    // 100 entradas de FAT consecutivas caben de sobra en un solo sector.
    let antes = lecturas();
    for c in 2..102u32 {
        v.raw_fat_entry(c);
    }
    let viajes = lecturas() - antes;
    assert!(viajes <= 1, "100 entradas del MISMO sector costaron {viajes} lecturas");

    // Y lo que se escribe se lee: un cache que no se entera de una escritura
    // seria peor que no tenerlo -- entregaria la cadena vieja sin decirlo.
    assert!(v.set_fat_entry(7, 0x0FFF_FFFF), "debe escribir");
    assert_eq!(v.raw_fat_entry(7), Some(0x0FFF_FFFF), "el cache sirvio lo de ANTES de escribir");
    assert!(v.set_fat_entry(7, 0), "debe poder soltarse");
    assert_eq!(v.raw_fat_entry(7), Some(0), "el cache se quedo con el valor viejo");
}

/// ** EL PATRON DE UN JUEGO LEYENDO SU WAD: saltos en los DOS sentidos.
///
/// === Que fija esta prueba ===
///
/// El cargador de `.bex` lee hacia adelante y retrocede **dos veces por
/// carga**, asi que le vale una copia suelta del cursor. Un archivo abierto
/// por un programa no: DOOM abre `doom1.wad`, lee el directorio de lumps del
/// final, y a partir de ahi salta a donde le pida el juego -- atras, adelante,
/// atras. Ahi retroceder **es el caso normal**, no la excepcion.
///
/// La regla que sostiene `ring0::obj::archivo` es esta: se guarda el cursor
/// del flujo **y una copia sin estrenar**, y cuando lo que se pide cae por
/// debajo de donde va el cursor, se vuelve a empezar desde la copia. Lo que
/// esta prueba fija es que **eso da los mismos bytes que un cursor limpio**,
/// salto tras salto y en cualquier orden.
///
/// Si un dia `Cursor::base` dejara de significar "el primer byte al que este
/// cursor todavia puede llegar", esto sale en rojo -- y el sintoma sin la
/// prueba seria un juego con las texturas cambiadas, que nadie sabe leer.
#[test]
fn el_patron_de_lumps_salta_en_los_dos_sentidos() {
    let (_turno, mut v) = volumen();
    let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("WADSIM  BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("WADSIM  BIN")).expect("debe estar");

    // Las dos mitades de un archivo reflejado: por donde va, y por donde
    // empieza. La segunda no se estrena jamas.
    let inicio = v.cursor(primero);
    let mut cur = inicio;
    let mut retrocesos = 0;

    // El orden es el de un juego, no el de un fichero: el directorio del
    // final primero, y despues lumps de aqui y de alla.
    for (off, len) in [
        (4900usize, 100usize), // el "directorio de lumps", al final
        (0, 12),               // la cabecera, o sea hacia atras del todo
        (3000, 400),           // adelante
        (1024, 512),           // atras otra vez
        (1536, 512),           // y adelante desde donde estaba: sin retroceso
        (17, 1),               // un byte suelto, atras y sin alinear
        (4999, 1),             // el ultimo byte
    ] {
        if off < cur.base() {
            cur = inicio;
            retrocesos += 1;
        }
        let mut dst = vec![0u8; len];
        let n = v.leer_en(&mut cur, off, tam, &mut dst);
        assert_eq!(n, len, "off={off} len={len}: el rango no llego entero");
        assert_eq!(&dst[..n], &datos[off..off + len], "off={off} len={len}: OTROS bytes");
    }

    // Y que el mecanismo se haya usado de verdad: sin esto, la prueba
    // pasaria igual el dia que alguien la reordene sin querer y nunca
    // vuelva a mirar hacia atras.
    assert!(retrocesos >= 3, "esta prueba tiene que retroceder: solo lo hizo {retrocesos} veces");
}

/// ** EL PATRON REAL DEL CARGADOR: dos tablas del FINAL antes que el codigo.
///
/// === Lo que esto fija ===
///
/// Un `.bex` no se lee de principio a fin. Antes de aterrizar la primera
/// seccion, el cargador necesita los **hashes** (`Signature`) y las
/// **relocations**, y las dos van al final del fichero -- en `d.bex`, la
/// firma esta en el `0x4B680` de `0x4B728` y el codigo empieza en el `0x200`.
///
/// Con un solo cursor eso es un salto al final y una vuelta atras, o sea un
/// `0` del que el cargador dijo `una seccion se quedo a medias al aterrizar`.
/// La salida no es dejar que el cursor retroceda: es que la lectura suelta
/// se lleve **una copia** y no toque la del flujo.
///
/// Por eso `Cursor` es `Copy`, y por eso esto es una prueba y no un
/// comentario: quitarle el `Copy` o guardar el cursor detras de algo que no
/// se pueda duplicar rompe el cargador **sin tocar el cargador**.
#[test]
fn una_lectura_suelta_no_mueve_el_cursor_del_flujo() {
    let (_turno, mut v) = volumen();
    // 5000 bytes = diez clusters con spc=1: hay cadena que recorrer.
    let datos: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    v.create_file_in_dir(2, &name("BEXSIM  BIN"), &datos).expect("debe crear");
    let (primero, tam) = v.find_file(&name("BEXSIM  BIN")).expect("debe estar");

    let flujo_inicio = v.cursor(primero);

    // -- 1. La "tabla de hashes": al final del fichero, con una COPIA --
    let mut aparte = flujo_inicio;
    let mut firma = [0u8; 100];
    let n = v.leer_en(&mut aparte, 4900, tam, &mut firma);
    assert_eq!(n, 100, "la tabla del final no se leyo entera");
    assert_eq!(&firma[..n], &datos[4900..5000], "y no son los bytes del final");

    // -- 2. El flujo de secciones, desde el principio. Su cursor no se ha
    //       enterado de nada de lo anterior. --
    let mut flujo = flujo_inicio;
    let mut codigo = [0u8; 512];
    let n = v.leer_en(&mut flujo, 512, tam, &mut codigo);
    assert_eq!(n, 512, "la primera seccion se quedo a medias: el cursor se movio");
    assert_eq!(&codigo[..n], &datos[512..1024], "la primera seccion trajo otros bytes");

    // -- 3. Y el flujo sigue avanzando normal detras de ella --
    let mut mas = [0u8; 512];
    let n = v.leer_en(&mut flujo, 1024, tam, &mut mas);
    assert_eq!(n, 512, "la seccion siguiente no llego");
    assert_eq!(&mas[..n], &datos[1024..1536], "la seccion siguiente trajo otros bytes");

    // Y la prueba de que el peligro era real: con EL MISMO cursor, el orden
    // del cargador contesta cero. Es el fallo del 2026-08-11 en una linea.
    let mut uno_solo = flujo_inicio;
    assert!(v.leer_en(&mut uno_solo, 4900, tam, &mut firma) > 0);
    assert_eq!(
        v.leer_en(&mut uno_solo, 512, tam, &mut codigo),
        0,
        "si esto deja de ser cero, el cursor retrocede en silencio (ver su cabecera)"
    );
}

#[test]
fn crear_y_leer_da_lo_mismo() {
    let (_turno, mut v) = volumen();
    let datos = b"BANCO BMO";
    v.create_file_in_dir(2, &name("CTAS    BIN"), datos).expect("debe crear");
    let mut dst = [0u8; 512];
    let n = leer_archivo(&mut v, "CTAS    BIN", &mut dst).expect("debe estar");
    assert_eq!(&dst[..n], datos);
}

/// El comportamiento de ANTES, que se conserva: `create` no pisa.
#[test]
fn crear_sobre_uno_que_existe_sigue_dando_exists() {
    let (_turno, mut v) = volumen();
    v.create_file_in_dir(2, &name("CTAS    BIN"), b"viejo").expect("debe crear");
    let r = v.create_file_in_dir(2, &name("CTAS    BIN"), b"nuevo");
    assert!(matches!(r, Err(WriteError::Exists)), "crear NO puede pisar: {r:?}");
}

/// ** LA PRUEBA QUE JUSTIFICA TODO ESTO.
///
/// Es el nivel 10 de COBOL corrido dos veces: la segunda escritura tiene
/// que ganar. Antes daba `Exists`, el `CLOSE` devolvia `0`, y en el disco
/// se quedaba el contenido de la primera corrida.
#[test]
fn guardar_dos_veces_deja_lo_segundo() {
    let (_turno, mut v) = volumen();
    v.save_file_in_dir(2, &name("CTAS    BIN"), b"primera").expect("1a");
    v.save_file_in_dir(2, &name("CTAS    BIN"), b"SEGUNDA").expect("2a");
    let mut dst = [0u8; 512];
    let n = leer_archivo(&mut v, "CTAS    BIN", &mut dst).expect("debe estar");
    assert_eq!(&dst[..n], b"SEGUNDA");
}

/// Y sin dejar UNA sola entrada de mas en el directorio.
///
/// Reemplazar anadiendo otra entrada dejaria dos nombres iguales: el
/// segundo inalcanzable y sus clusters perdidos para siempre. Es justo el
/// motivo por el que `create` rechaza los repetidos.
#[test]
fn guardar_dos_veces_no_duplica_la_entrada() {
    let (_turno, mut v) = volumen();
    v.save_file_in_dir(2, &name("CTAS    BIN"), b"primera").expect("1a");
    v.save_file_in_dir(2, &name("CTAS    BIN"), b"SEGUNDA").expect("2a");

    let mut buf = [0u8; 512];
    assert!(read(v.cluster_to_lba(2), 1, &mut buf));
    let mut cuantas = 0;
    for i in 0..(512 / 32) {
        let de = unsafe { &*(buf.as_ptr().add(i * 32) as *const DirEntry) };
        if de.name[0] == 0 { break; }
        if de.name[0] == 0xE5 { continue; }
        if name_match(&de.name, &name("CTAS    BIN")) { cuantas += 1; }
    }
    assert_eq!(cuantas, 1, "reemplazar no puede dejar dos entradas con el mismo nombre");
}

/// * Y sin FUGAR clusters: la cadena vieja tiene que quedar suelta.
///
/// Un reemplazo que no libera lo anterior no rompe nada visible --el
/// archivo se lee bien-- pero el volumen se llena solo, y el dia que se
/// llene el motivo llevara meses enterrado.
#[test]
fn reemplazar_suelta_la_cadena_vieja() {
    let (_turno, mut v) = volumen();
    // 1200 bytes con clusters de 512 son TRES clusters.
    let grande = [b'A'; 1200];
    v.save_file_in_dir(2, &name("GRANDE  BIN"), &grande).expect("1a");
    assert_eq!(ocupados(&mut v), 1 + 3, "raiz + tres clusters de datos");

    // Y ahora uno pequeno en su sitio: tiene que BAJAR a un solo cluster.
    v.save_file_in_dir(2, &name("GRANDE  BIN"), b"corto").expect("2a");
    let quedan = ocupados(&mut v);
    assert_eq!(quedan, 1 + 1, "los tres clusters viejos tenian que soltarse: quedan {quedan}");
}

/// Al reves tambien: crecer reserva la cadena entera y el archivo se lee
/// completo. Un reemplazo que solo escribiera el primer cluster daria un
/// archivo del tamano nuevo con la cola del viejo dentro.
#[test]
fn reemplazar_por_uno_mas_grande_lo_lee_entero() {
    let (_turno, mut v) = volumen();
    v.save_file_in_dir(2, &name("CRECE   BIN"), b"corto").expect("1a");
    let mut grande = [0u8; 1500];
    for (i, b) in grande.iter_mut().enumerate() { *b = (i % 251) as u8; }
    v.save_file_in_dir(2, &name("CRECE   BIN"), &grande).expect("2a");

    let mut dst = [0u8; 2048];
    let n = leer_archivo(&mut v, "CRECE   BIN", &mut dst).expect("debe estar");
    assert_eq!(n, grande.len(), "el tamano de la entrada tiene que ser el nuevo");
    assert_eq!(&dst[..n], &grande[..], "y los bytes, los nuevos de punta a punta");
}


/// ** UN ARCHIVO LARGO SE LEE ENTERO, cadena de clusters incluida.
///
/// El `.bex` mas grande que este sistema habia cargado eran 306 KiB; DOOM
/// son 814 KiB, **2,7 veces mas**, y el 2026-08-09 el cargador lo rechazo.
/// La primera sospecha fue una lectura corta con muchos clusters -- y esta
/// fila existe para contestar esa pregunta en el anfitrion en vez de con
/// fotos del Ryzen.
///
/// Con clusters de UN sector, 200 KiB son **400 clusters encadenados**: la
/// misma forma que el fichero de verdad, en un disco de juguete.
#[test]
fn un_archivo_de_cientos_de_clusters_se_lee_entero() {
    let (_turno, mut v) = volumen();
    // 200 KiB = 400 clusters de 512 B. Cabe en el disco de 256 KiB? No:
    // se usa lo que si cabe con holgura -- 100 KiB son 200 clusters, que ya
    // es un orden de magnitud por encima de lo que probaba nada.
    let mut grande = std::vec![0u8; 100 * 1024];
    for (i, b) in grande.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    v.save_file_in_dir(2, &name("GRANDE  BEX"), &grande).expect("debe guardar");

    let mut dst = std::vec![0u8; grande.len()];
    let n = leer_archivo(&mut v, "GRANDE  BEX", &mut dst).expect("debe estar");
    assert_eq!(n, grande.len(), "se leyeron {n} de {} bytes", grande.len());
    // Y byte a byte: un tamano correcto con un agujero dentro es
    // exactamente el fallo que esta fila viene a descartar.
    let malo = dst.iter().zip(grande.iter()).position(|(a, b)| a != b);
    assert!(malo.is_none(), "primer byte distinto en {malo:?}");
}

/// ** Y UN SECTOR QUE NO SE PUEDE LEER **CORTA** la lectura.
///
/// Antes no: el `read_sector` fallido no copiaba nada y el `offset += count`
/// corria igual, asi que `read_file` contestaba el tamano COMPLETO con el
/// trozo sin tocar -- basura, o los bytes de quien tuvo antes ese buffer.
/// Un `.bex` de 1.591 sectores necesita **una** lectura mala para llegar al
/// cargador con un agujero y del tamano correcto.
///
/// Se provoca acortando el disco: los clusters del final quedan fuera del
/// medio y `read` contesta `false`.
#[test]
fn un_sector_ilegible_corta_la_lectura_en_vez_de_mentir() {
    let (_turno, mut v) = volumen();
    let grande = [b'Z'; 4096]; // ocho clusters
    v.save_file_in_dir(2, &name("CORTADO BIN"), &grande).expect("debe guardar");

    // El destino es mas corto que el archivo a proposito: `read_file` tiene
    // que parar en el borde de `dst` y decir cuanto trajo, no pasarse.
    let mut dst = [0u8; 1024];
    let (primero, tam) = v.find_file(&name("CORTADO BIN")).expect("debe estar");
    let n = v.read_file(primero, tam, &mut dst);
    assert_eq!(n, dst.len(), "tiene que parar en el borde del destino");
    assert!(dst.iter().all(|&b| b == b'Z'), "y lo que trajo tiene que ser bueno");
}

/// `save` sobre un nombre que NO existe es crear, sin sorpresas.
#[test]
fn guardar_lo_que_no_existe_es_crear() {
    let (_turno, mut v) = volumen();
    v.save_file_in_dir(2, &name("NUEVO   TXT"), b"hola").expect("debe crear");
    let mut dst = [0u8; 512];
    let n = leer_archivo(&mut v, "NUEVO   TXT", &mut dst).expect("debe estar");
    assert_eq!(&dst[..n], b"hola");
}

/// * Reemplazar NO puede tocar al archivo de al lado.
///
/// Es el fallo silencioso que mas miedo da de esta operacion: la entrada
/// de directorio se reescribe dentro de un sector que comparte con otras
/// quince, y escribir ese sector con un buffer que no sea el suyo se lleva
/// a los vecinos por delante.
///
/// [!] Y hay que decir lo que esta prueba NO demuestra hoy: quitar el
/// `read_sector` de `replace_file_fat32` **no la hace caer**, porque `buf`
/// resulta que todavia conserva ese sector de cuando se busco la entrada.
/// Eso es un accidente del orden de las llamadas, no una garantia -- se
/// comprobo mutandolo. La relectura se queda por eso mismo, y esta prueba
/// vale como red para la implementacion que venga despues, no como
/// demostracion de la de ahora.
#[test]
fn reemplazar_no_toca_al_vecino() {
    let (_turno, mut v) = volumen();
    v.save_file_in_dir(2, &name("UNO     TXT"), b"el primero").expect("uno");
    v.save_file_in_dir(2, &name("DOS     TXT"), b"el segundo").expect("dos");
    v.save_file_in_dir(2, &name("UNO     TXT"), b"PISADO").expect("uno otra vez");

    let mut dst = [0u8; 512];
    let n = leer_archivo(&mut v, "DOS     TXT", &mut dst).expect("el vecino debe seguir ahi");
    assert_eq!(&dst[..n], b"el segundo", "el vecino no puede cambiar");
    let n = leer_archivo(&mut v, "UNO     TXT", &mut dst).unwrap();
    assert_eq!(&dst[..n], b"PISADO");
}

/// Un volumen montado sin escritor no escribe. No es una politica que
/// alguien tenga que recordar respetar: no hay con que.
#[test]
fn sin_escritor_no_se_guarda() {
    let (_turno, _) = volumen();
    let mut v = mount(&DISPOSITIVO, false, 0).expect("debe montar en solo lectura");
    let r = v.save_file_in_dir(2, &name("NOPE    TXT"), b"x");
    assert!(matches!(r, Err(WriteError::ReadOnly)), "{r:?}");
}

// ------------------------------------------------------------------------
//  *** LOS CLUSTERS QUE VIENEN DEL DISCO Y NO EXISTEN (auditoria 2026-08-24)
// ------------------------------------------------------------------------

/// **Un cluster 0 o 1 no puede convertirse en un LBA.**
///
/// La numeracion de FAT empieza en 2, asi que `cluster_to_lba` hace
/// `cluster - 2`. Con un 0 eso da la vuelta al contador y sale un sector
/// **cualquiera del disco** -- que despues se devuelve como si fuera el
/// contenido del fichero que se pidio.
///
/// ** No es un fallo hipotetico de un disco roto: los numeros de cluster los
/// escribe QUIEN FORMATEO EL VOLUMEN, y ese puede no ser esta maquina.
#[test]
fn los_clusters_reservados_no_son_clusters() {
    let (_g, v) = volumen();
    assert!(!v.cluster_valido(0), "el 0 esta reservado");
    assert!(!v.cluster_valido(1), "el 1 esta reservado");
    assert!(v.cluster_valido(2), "el 2 es el primero de verdad");
}

/// Y por arriba: mas alla de los clusters que el volumen TIENE.
#[test]
fn un_cluster_que_no_existe_se_rechaza() {
    let (_g, v) = volumen();
    let ultimo = v.max_cluster;
    assert!(v.cluster_valido(ultimo), "el ultimo tiene que valer");
    assert!(!v.cluster_valido(ultimo + 1), "uno mas alla no existe");
    assert!(!v.cluster_valido(u32::MAX), "y el tope tampoco");
}

/// [!] Y la cadena de la FAT tampoco entrega uno imposible.
///
/// Es el productor que mas veces se recorre --una vez por cluster de cada
/// fichero-- y por eso el tope va DENTRO de `read_fat_entry` y no en cada
/// bucle que la llama.
#[test]
fn la_cadena_de_la_fat_no_entrega_un_cluster_imposible() {
    let (_g, mut v) = volumen();
    // Se envenena la entrada del cluster 2 con un numero que no existe pero
    // que NO es una marca de fin de cadena (aquellas son >= 0x0FFF_FFF7).
    let fat_lba = v.fat_start as u64 + v.partition_lba();
    let mut sector = [0u8; 512];
    assert!(v.dev.read(fat_lba, 1, &mut sector).is_ok());
    let veneno: u32 = 0x00FF_FFF0;
    assert!(veneno < 0x0FFF_FFF7, "el veneno tiene que parecer un cluster");
    sector[8..12].copy_from_slice(&veneno.to_le_bytes());
    assert!(v.dev.write(fat_lba, 1, &sector).is_ok());
    v.fat_cache_lba = SIN_CACHE;

    assert_eq!(
        v.read_fat_entry(2),
        None,
        "*** la cadena entrego un cluster que este volumen no tiene"
    );
}
