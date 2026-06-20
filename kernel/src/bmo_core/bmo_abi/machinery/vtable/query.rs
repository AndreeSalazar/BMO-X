//! Sustituto de `IUnknown::QueryInterface` — sin COM, sin GUIDs, sin `AddRef`.

use crate::bmo_core::barex::{BxError, BxResult};
use crate::bmo_core::bmo_abi::type_system::TypeId;
use super::fat_ptr::BmoFatPtr;
use super::table::BmoVTable;
use crate::bmo_core::bmo_abi::vtable::entry::EntryKind;

/// `InterfaceId` es un `TypeId` (la interfaz es un tipo BMO más).
pub type InterfaceId = TypeId;

/// Pregunta si `obj` implementa la interfaz `id`. Si sí, devuelve un nuevo
/// `BmoFatPtr` apuntando a la vtable correspondiente. Si no, `BadHandle`.
///
/// Recorre los `EntryKind::InterfaceLink` de la vtable actual.
pub fn query_interface(obj: BmoFatPtr, vtable: &BmoVTable<'_>, id: InterfaceId) -> BxResult<BmoFatPtr> {
    if obj.is_null() { return Err(BxError::BadHandle); }
    if vtable.header.interface_type == id {
        return Ok(obj);
    }
    for e in vtable.entries.iter() {
        if e.kind == EntryKind::InterfaceLink && e.name_hash == (id.raw() as u32) {
            // El fn_ptr del slot apunta a la vtable hermana.
            return Ok(BmoFatPtr::new(obj.data, e.fn_ptr));
        }
    }
    Err(BxError::Unsupported)
}
