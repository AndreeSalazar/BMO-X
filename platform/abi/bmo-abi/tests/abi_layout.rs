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
    assert_eq!(loaded.sections.len(), 2);
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
