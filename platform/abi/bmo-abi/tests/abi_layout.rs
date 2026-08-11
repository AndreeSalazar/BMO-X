#![no_std]
#![allow(dead_code)]
extern crate alloc;
use alloc::vec;

#[test]
fn bef_build_validate_load_roundtrip() {
    let mut b = bmo_abi::bef::BefBuilder::new();
    b.add_section(bmo_abi::bef::BefSection::code(vec![0xC3; 16]));
    b.add_section(bmo_abi::bef::BefSection::rodata(b"hello\0".to_vec()));
    let bytes = b.build().unwrap();
    let result = bmo_abi::bef::validate(&bytes);
    assert!(result.is_valid, "BEF should be valid: {:?}", result.issues);
    let loaded = bmo_abi::bef::load(&bytes, 0, |_, _| Err("no imports")).unwrap();
    assert!(loaded.entry_point > 0);
    // Cuatro, y las dos ultimas las pone `build` por su cuenta:
    //
    // - `code` y `rodata`, que son las que se anadieron aqui.
    // - `Signature` (2026-08-09): el BLAKE3 de cada una de las otras. Los
    //   hashes ya se calculaban y se escribian al final del fichero sin entrada
    //   que los nombrara, o sea invisibles para cualquier lector.
    // - `Requisitos` (2026-08-10): lo que la imagen necesita para arrancar. Se
    //   emite sola por el mismo motivo que la firma --el dato solo lo tiene el
    //   escritor-- y existe para que el kernel deje de DEDUCIRLO. Ver
    //   `docs/EL_CONTRATO_DE_CARGA.md`.
    //
    // ** Que este numero suba es una noticia, no un fallo: significa que el
    // escritor emite algo nuevo, y entonces hay que decir QUE y por que.
    assert_eq!(loaded.sections.len(), 4);
}

#[test]
fn type_registry_basic() {
    let mut reg = bmo_abi::runtime::types::TypeRegistry::new();
    let meta = bmo_abi::runtime::types::TypeMeta {
        name_hash: 0xABCD,
        size: 16,
        align: 8,
        kind: 0,
        field_count: 2,
    };
    let idx = reg.register(meta).unwrap();
    assert_eq!(idx, 0);
    let looked_up = reg.lookup(0xABCD).unwrap();
    assert_eq!(looked_up.meta.size, 16);
}

#[test]
fn reflect_query_type() {
    let mut reg = bmo_abi::runtime::types::TypeRegistry::new();
    reg.register(bmo_abi::runtime::types::TypeMeta {
        name_hash: 42, size: 8, align: 4, kind: 4, field_count: 0,
    });
    let info = bmo_abi::values::reflect::ReflectQuery::resolve_type(&reg, 42);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.size, 8);
    assert_eq!(info.field_count, 0);
}

#[test]
fn bmo_status_layout() {
    use bmo_abi::fundamentals::status::BmoStatus;
    assert_eq!(core::mem::size_of::<BmoStatus>(), 16);
    assert_eq!(core::mem::align_of::<BmoStatus>(), 8);
    let ok = BmoStatus::OK;
    assert!(ok.code == 0);
    let err = BmoStatus::err(5);
    assert!(err.code == 5);

    let status = BmoStatus { code: 7, flags: 0xA5A5_5A5A, value: 0x1122_3344_5566_7788 };
    let (rax, rdx) = status.into_registers();
    assert_eq!(rax, 0xA5A5_5A5A_0000_0007);
    assert_eq!(rdx, status.value);
    assert_eq!(BmoStatus::from_registers(rax, rdx), status);
}

#[test]
fn syscall_contract_is_distinct_from_native_calls() {
    use bmo_abi::types::convention::*;
    assert_eq!(GPR_ARG_COUNT, 7);
    assert_eq!(SYSCALL_GPR_ARG_COUNT, 6);
    assert_eq!(X86_64_SYSCALL_ARG_REGISTERS, &["rdi", "rsi", "rdx", "r10", "r8", "r9"]);

    let result = bmo_abi::syscalls::SyscallResult(1_u64 << 32, 42);
    assert!(result.is_ok());
    assert_eq!(result.flags(), 1);
    assert_eq!(result.status().value, 42);
}

#[test]
fn abi_v2_keeps_v1_as_migration_input() {
    assert_eq!(bmo_abi::BMO_ABI_VERSION, (2, 0));
    assert!(bmo_abi::supports_abi((2, 0)));
    assert!(bmo_abi::supports_abi((1, 0)));
    assert!(!bmo_abi::supports_abi((3, 0)));
}

#[test]
fn bmo_string_create() {
    use bmo_abi::fundamentals::string::BmoString;
    let s = BmoString::from_string(alloc::string::String::from("hello"));
    assert_eq!(s.len(), 5);
    assert!(s.capacity() >= 5);
}

#[test]
fn beffer_import_resolve_callback() {
    let mut b = bmo_abi::bef::BefBuilder::new();
    b.add_section(bmo_abi::bef::BefSection::code(vec![0xC3; 64]));
    let bytes = b.build().unwrap();
    let loaded = bmo_abi::bef::load(&bytes, 0, |lib, sym| {
        if lib == "test.lib" && sym == "test_fn" { Ok(0x1000) } else { Err("not found") }
    }).unwrap();
    assert!(loaded.entry_point > 0);
}

#[test]
fn static_assert_sizes() {
    assert_eq!(core::mem::size_of::<bmo_abi::bef::header::BefHeader>(), 48);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::sections::SectionEntry>(), 48);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::relocations::Relocation>(), 24);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::imports::ImportEntry>(), 24);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::exports::ExportEntry>(), 32);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::signing::SectionHash>(), 40);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::signing::SignatureHeader>(), 8);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::tls::TlsTemplate>(), 24);
    assert_eq!(core::mem::size_of::<bmo_abi::bef::symbols::Symbol>(), 32);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::status::BmoStatus>(), 16);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::error::BmoError>(), 16);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::memory::BmoSlice>(), 16);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::buffer::BmoBuffer>(), 32);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::string::BmoStr>(), 16);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::string::BmoString>(), 24);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::allocator::BmoAllocResult>(), 24);
    assert_eq!(core::mem::size_of::<bmo_abi::fundamentals::capability::BmoCap>(), 8);
    assert_eq!(core::mem::size_of::<bmo_abi::values::version::BmoVersion>(), 12);
    assert_eq!(core::mem::size_of::<bmo_abi::values::uuid::BmoUuid>(), 16);
    assert_eq!(core::mem::size_of::<bmo_abi::values::net::BmoIpv4Addr>(), 4);
    assert_eq!(core::mem::size_of::<bmo_abi::values::net::BmoIpv6Addr>(), 16);
}

// -- El contrato de bytes de una `SeccionAbs64` ------------------------
//
// * POR QUE ESTE TEST EXISTE, y no es redundante con `size_of == 24`.
//
// El cargador de Ring 0 **no importa este crate**: `bmo-abi` es el contrato y el
// kernel lo implementa leyendo bytes a mano (`ring0/task/bex.rs::leer_reloc`),
// igual que hace con la tabla de secciones. Asi que hay dos lectores del mismo
// formato en dos sitios, y **el compilador no puede comprobar que coincidan**.
//
// Si alguien reordena los campos de `Relocation`, este crate seguiria
// compilando, el `.bex` saldria con otra disposicion, y el kernel leeria el
// `kind` donde esta el `addend`. El sintoma: direcciones inventadas escritas en
// la memoria de un proceso, o sea el bug que las relocations existen para matar.
//
// Este test fija los OFFSETS que el kernel usa. Si cambian, se pone rojo aqui y
// no en el Ryzen.

/// Los mismos offsets que `ring0/task/bex.rs::leer_reloc`. Copiados a mano a
/// proposito: si se importaran, no se estaria probando nada.
const OFF_DONDE: usize = 0; //  u64  <- Relocation::offset
const OFF_DESTINO_SEC: usize = 8; //  u32  <- Relocation::symbol_idx
const OFF_KIND: usize = 12; //  u8
const OFF_DONDE_SEC: usize = 13; //  u8   <- Relocation::target_section
const OFF_ADDEND: usize = 16; //  i64

#[test]
fn una_seccion_abs64_se_descodifica_como_la_lee_el_kernel() {
    use bmo_abi::bef::relocations::{Relocation, RelocationKind, SEC_DATA, SEC_RODATA};

    // "en .data+8 escribe la direccion de .rodata+17" -- la reloc real de
    // `char *nombres[] = {"...", "imp", ...}`.
    let r = Relocation::seccion_abs64(SEC_DATA, 8, SEC_RODATA, 17);

    let bytes: &[u8] = unsafe {
        core::slice::from_raw_parts(
            &r as *const Relocation as *const u8,
            core::mem::size_of::<Relocation>(),
        )
    };
    assert_eq!(bytes.len(), Relocation::SIZE, "la reloc tiene que medir 24");

    let donde_off = u64::from_le_bytes(bytes[OFF_DONDE..OFF_DONDE + 8].try_into().unwrap());
    let destino_sec =
        u32::from_le_bytes(bytes[OFF_DESTINO_SEC..OFF_DESTINO_SEC + 4].try_into().unwrap());
    let kind = bytes[OFF_KIND];
    let donde_sec = bytes[OFF_DONDE_SEC];
    let addend = i64::from_le_bytes(bytes[OFF_ADDEND..OFF_ADDEND + 8].try_into().unwrap());

    assert_eq!(donde_off, 8, "el offset donde se escribe");
    assert_eq!(donde_sec, SEC_DATA, "la seccion donde se escribe");
    assert_eq!(destino_sec as u8, SEC_RODATA, "la seccion del destino");
    assert_eq!(addend, 17, "el offset del destino");
    assert_eq!(kind, RelocationKind::SeccionAbs64 as u8);
    assert_eq!(kind, 0x04, "el kernel compara contra 0x04 literal");
}

/// [!] Y la trampa de las DOS numeraciones de seccion, fijada con numeros.
///
/// Los codigos de una reloc no son los de `SectionKind`: `data` y `rodata`
/// estan cambiados. Es la parte del formato mas facil de cruzar, asi que el
/// numero esta escrito y no deducido.
#[test]
fn los_codigos_de_seccion_de_una_reloc_no_son_los_de_sectionkind() {
    use bmo_abi::bef::relocations::{SEC_CODE, SEC_DATA, SEC_RODATA};
    use bmo_abi::bef::sections::SectionKind;

    assert_eq!(SEC_CODE, 0);
    assert_eq!(SEC_DATA, 1);
    assert_eq!(SEC_RODATA, 2);

    assert_eq!(SectionKind::Code as u8, 1);
    assert_eq!(SectionKind::RoData as u8, 2);
    assert_eq!(SectionKind::Data as u8, 3);

    // Lo que hace que esto sea una trampa y no una curiosidad: rodata COINCIDE
    // en 2, asi que un cruce de las dos tablas acierta en rodata y falla en las
    // otras dos -- el peor caso posible, porque parece funcionar a medias.
    assert_eq!(SEC_RODATA, SectionKind::RoData as u8);
    assert_ne!(SEC_DATA, SectionKind::Data as u8);
    assert_ne!(SEC_CODE, SectionKind::Code as u8);
}

// =============== LA FIRMA: integridad que VIAJA CON EL FICHERO ===============
//
// El kernel NO importa `bmo-abi` a proposito: lee los bytes a mano. Asi que hay
// dos lectores del mismo formato y el compilador no puede comprobar que
// coincidan. Estas filas lo comprueban aqui, con los MISMOS offsets que usa
// `ring0/task/bex.rs::verificar_hashes`, copiados a mano igual que se copiaron
// alli. Si divergen, esto se pone rojo antes de que ningun `.bex` deje de
// arrancar en el Ryzen.

const CAB_FIRMA: usize = 8; // hash_count (u32) + sig_algo (u32)
const ENTRADA: usize = 40; // section_index (u16) + pad(6) + digest(32)
const SECTION_SIGNATURE: u8 = 0x0F;
const BEX_SECTION_SIZE: usize = 48;

fn tabla_de(b: &[u8]) -> (usize, usize) {
    let mut t = [0u8; 8];
    t.copy_from_slice(&b[32..40]);
    let tabla = u64::from_le_bytes(t) as usize;
    let count = u32::from_le_bytes([b[40], b[41], b[42], b[43]]) as usize;
    (tabla, count)
}

/// La copia exacta de lo que hace el kernel. `true` = todas cuadran.
fn verificar_como_el_kernel(b: &[u8]) -> bool {
    let (tabla, count) = tabla_de(b);
    let mut sig_off = 0usize;
    let mut sig_len = 0usize;
    let mut sig_idx = usize::MAX;
    for i in 0..count {
        let e = tabla + i * BEX_SECTION_SIZE;
        if b[e] == SECTION_SIGNATURE {
            let mut v = [0u8; 8];
            v.copy_from_slice(&b[e + 8..e + 16]);
            sig_off = u64::from_le_bytes(v) as usize;
            v.copy_from_slice(&b[e + 16..e + 24]);
            sig_len = u64::from_le_bytes(v) as usize;
            sig_idx = i;
            break;
        }
    }
    if sig_len < CAB_FIRMA {
        return true; // sin firma no hay nada que comprobar
    }
    let cuantos =
        u32::from_le_bytes([b[sig_off], b[sig_off + 1], b[sig_off + 2], b[sig_off + 3]]) as usize;
    for k in 0..cuantos {
        let h = sig_off + CAB_FIRMA + k * ENTRADA;
        let idx = u16::from_le_bytes([b[h], b[h + 1]]) as usize;
        if idx == sig_idx || idx >= count {
            continue;
        }
        let e = tabla + idx * BEX_SECTION_SIZE;
        let mut v = [0u8; 8];
        v.copy_from_slice(&b[e + 8..e + 16]);
        let off = u64::from_le_bytes(v) as usize;
        v.copy_from_slice(&b[e + 16..e + 24]);
        let len = u64::from_le_bytes(v) as usize;
        let datos = if len == 0 { &b[0..0] } else { &b[off..off + len] };
        if bmo_abi::bef::signing::blake3_256(datos)[..] != b[h + 8..h + 40] {
            return false;
        }
    }
    true
}

fn una_imagen() -> alloc::vec::Vec<u8> {
    let mut b = bmo_abi::bef::BefBuilder::new();
    b.add_section(bmo_abi::bef::BefSection::code(vec![0xC3; 4096]));
    b.add_section(bmo_abi::bef::BefSection::rodata(b"hola mundo\0".to_vec()));
    b.add_section(bmo_abi::bef::BefSection::data(vec![7u8; 512]));
    b.build().unwrap()
}

/// Un `.bex` recien escrito verifica. Es la fila que hace utiles a las demas:
/// un verificador que dijera "no" siempre tambien cazaria la corrupcion.
#[test]
fn una_imagen_recien_escrita_cuadra_con_sus_hashes() {
    assert!(
        verificar_como_el_kernel(&una_imagen()),
        "una imagen sin tocar tiene que cuadrar"
    );
}

/// ** UN SOLO BYTE CAMBIADO Y NO CUADRA.
///
/// Es el caso que ningun contador de bytes ve: el fichero mide exactamente lo
/// que debe y por dentro no es el mismo. Un sector que se lee sin error y trae
/// datos de otro sitio produce justo esto.
#[test]
fn un_byte_cambiado_en_el_codigo_rompe_su_hash() {
    let mut img = una_imagen();
    let antes = img.len();
    // ** EL OFFSET SE LEE DE LA TABLA, que es de donde lo lee el kernel.
    //
    // Aqui ponia `tabla + count * BEX_SECTION_SIZE` -- *"la seccion empieza tras
    // cabecera + tabla"*. Era cierto y **dejo de serlo el 2026-08-10**: lo que
    // se carga se alinea ahora a sector, asi que delante del codigo hay relleno.
    //
    // Y el sintoma fue el peor posible: la prueba **paso a tocar un byte de
    // relleno**, que no pertenece a ninguna seccion y por tanto no esta bajo
    // ningun hash. O sea que dejo de comprobar lo que dice comprobar sin dejar
    // de compilar. Una prueba que calcula por su cuenta un offset que el fichero
    // ya declara es una prueba que un dia mira a otro sitio.
    let (tabla, count) = tabla_de(&img);
    let mut off = 0usize;
    for i in 0..count {
        let e = tabla + i * BEX_SECTION_SIZE;
        if img[e] == bmo_abi::bef::sections::SectionKind::Code as u8 {
            let mut v = [0u8; 8];
            v.copy_from_slice(&img[e + 8..e + 16]);
            off = u64::from_le_bytes(v) as usize;
            break;
        }
    }
    assert!(off != 0, "la imagen de prueba tiene seccion code");
    img[off] ^= 0xFF;
    assert_eq!(img.len(), antes, "el tamano NO cambia: por eso hace falta el hash");
    assert!(
        !verificar_como_el_kernel(&img),
        "un byte distinto en .code tiene que romper su hash"
    );
}

/// Y en los DATOS tambien -- no solo en el codigo. Una tabla de constantes
/// corrupta da numeros plausibles, que es peor que un fallo.
#[test]
fn un_byte_cambiado_en_los_datos_rompe_su_hash() {
    let mut img = una_imagen();
    let (tabla, count) = tabla_de(&img);
    // La seccion `data` es la tercera que se anadio.
    let mut tocado = false;
    for i in 0..count {
        let e = tabla + i * BEX_SECTION_SIZE;
        if img[e] == bmo_abi::bef::sections::SectionKind::Data as u8 {
            let mut v = [0u8; 8];
            v.copy_from_slice(&img[e + 8..e + 16]);
            let off = u64::from_le_bytes(v) as usize;
            img[off + 3] ^= 0x01;
            tocado = true;
            break;
        }
    }
    assert!(tocado, "la imagen de prueba tiene seccion data");
    assert!(!verificar_como_el_kernel(&img), "un byte de .data tiene que romper su hash");
}

/// ** Y EMPAQUETAR REGENERA LA FIRMA.
///
/// Meter recursos recoloca las secciones, asi que los hashes viejos describen
/// una disposicion que ya no existe. Conservarlos daria un fichero que declara
/// integridad y no la cumple -- peor que uno sin firma, porque el segundo al
/// menos no promete nada.
#[test]
fn empaquetar_regenera_la_firma_y_el_paquete_verifica() {
    let img = una_imagen();
    let paquete = bmo_abi::bef::empaquetar(&img, &[("icono", &[1u8, 2, 3][..])]).unwrap();
    assert!(paquete.len() > img.len(), "el paquete lleva algo mas dentro");
    assert!(
        verificar_como_el_kernel(&paquete),
        "tras empaquetar, los hashes tienen que describir la disposicion NUEVA"
    );
}
