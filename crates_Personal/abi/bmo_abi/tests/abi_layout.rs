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
