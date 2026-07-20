//! Ring 0 BEX admission gate.
//!
//! This module performs the allocation-free part of loading a BEX image:
//! validate the x86-64/ABI contract and create a fixed-size mapping plan.
//! It deliberately does not execute code.  The process subsystem will later
//! consume this plan to allocate user pages, copy sections and enter Ring 3.

// Keep this parser `no_std` and allocation-free.  `bmo-abi` is the canonical
// producer/validator, but its complete API intentionally uses `alloc`, which
// is not available until the kernel has initialized a process allocator.
// These offsets are the stable BEX v1 (= BEF1) wire contract.
const BEX_MAGIC: u32 = u32::from_le_bytes(*b"BEF1");
const BEX_HEADER_SIZE: usize = 48;
const BEX_SECTION_SIZE: usize = 48;
const BEX_VERSION_MAJOR: u16 = 1;
const BEX_ARCH_X86_64: u8 = 0x01;
const BEX_FLAG_EXECUTABLE: u32 = 1 << 0;
const SECTION_CODE: u8 = 0x01;
const SECTION_BSS: u8 = 0x04;
const SECTION_FLAG_EXEC: u32 = 1 << 2;

/// The first BEX process supports a compact, auditable section table.
pub const MAX_BEX_SECTIONS: usize = 16;

#[derive(Clone, Copy)]
pub struct BexMapping {
    pub kind: u8,
    pub flags: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub alignment: u16,
}

const EMPTY_MAPPING: BexMapping = BexMapping {
    kind: 0,
    flags: 0,
    file_offset: 0,
    file_size: 0,
    mem_size: 0,
    alignment: 0,
};

/// Validated inputs required by the future Ring 3 mapper.
pub struct BexLoadPlan {
    /// Offset within the executable Code section; it is not a Ring 0 address.
    pub entry_offset: u64,
    pub sections: [BexMapping; MAX_BEX_SECTIONS],
    pub section_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BexError {
    TooSmall,
    InvalidHeader,
    UnsupportedArchitecture,
    AbiMismatch,
    NotExecutable,
    TooManySections,
    SectionTableOutOfBounds,
    InvalidSection,
    MissingCode,
    EntryOutsideCode,
}

/// Validate an untrusted BEX image and produce a fixed-size mapping plan.
///
/// No `alloc`, file access, relocation, page-table mutation or control transfer
/// happens here.  This boundary is therefore safe to call before a process is
/// admitted to the kernel.
pub fn inspect(bytes: &[u8]) -> Result<BexLoadPlan, BexError> {
    if bytes.len() < BEX_HEADER_SIZE {
        return Err(BexError::TooSmall);
    }
    let magic = read_u32(bytes, 0).ok_or(BexError::TooSmall)?;
    let version_major = read_u16(bytes, 4).ok_or(BexError::TooSmall)?;
    let flags = read_u32(bytes, 8).ok_or(BexError::TooSmall)?;
    let arch = *bytes.get(12).ok_or(BexError::TooSmall)?;
    let abi_major = *bytes.get(16).ok_or(BexError::TooSmall)?;
    let abi_minor = *bytes.get(17).ok_or(BexError::TooSmall)?;
    let entry_offset = read_u64(bytes, 24).ok_or(BexError::TooSmall)?;
    let section_table_offset = read_u64(bytes, 32).ok_or(BexError::TooSmall)?;
    let section_count = read_u32(bytes, 40).ok_or(BexError::TooSmall)? as usize;
    if magic != BEX_MAGIC || version_major != BEX_VERSION_MAJOR || section_count == 0 {
        return Err(BexError::InvalidHeader);
    }
    if arch != BEX_ARCH_X86_64 {
        return Err(BexError::UnsupportedArchitecture);
    }
    let supported_abi = (abi_major == 1 && abi_minor == 0)
        || (abi_major == 2 && abi_minor == 0);
    if !supported_abi {
        return Err(BexError::AbiMismatch);
    }
    if flags & BEX_FLAG_EXECUTABLE == 0 {
        return Err(BexError::NotExecutable);
    }

    let count = section_count;
    if count > MAX_BEX_SECTIONS {
        return Err(BexError::TooManySections);
    }
    let table_size = count.checked_mul(BEX_SECTION_SIZE).ok_or(BexError::SectionTableOutOfBounds)?;
    let table_start = section_table_offset as usize;
    let table_end = table_start.checked_add(table_size).ok_or(BexError::SectionTableOutOfBounds)?;
    if table_end > bytes.len() {
        return Err(BexError::SectionTableOutOfBounds);
    }

    let mut plan = BexLoadPlan {
        entry_offset,
        sections: [EMPTY_MAPPING; MAX_BEX_SECTIONS],
        section_count: count,
    };
    let mut code_size = None;

    for index in 0..count {
        let offset = table_start + index * BEX_SECTION_SIZE;
        let kind = *bytes.get(offset).ok_or(BexError::InvalidSection)?;
        let section_flags = read_u32(bytes, offset + 4).ok_or(BexError::InvalidSection)?;
        let file_offset = read_u64(bytes, offset + 8).ok_or(BexError::InvalidSection)?;
        let file_size = read_u64(bytes, offset + 16).ok_or(BexError::InvalidSection)?;
        let mem_size = read_u64(bytes, offset + 24).ok_or(BexError::InvalidSection)?;
        let alignment_raw = read_u16(bytes, offset + 40).ok_or(BexError::InvalidSection)?;
        if kind == 0 || file_size > mem_size || (kind != SECTION_BSS && file_size == 0) {
            return Err(BexError::InvalidSection);
        }
        if kind != SECTION_BSS {
            let end = file_offset.checked_add(file_size).ok_or(BexError::InvalidSection)?;
            if end as usize > bytes.len() {
                return Err(BexError::InvalidSection);
            }
        }
        let alignment = if alignment_raw == 0 { 8 } else { alignment_raw };
        if !alignment.is_power_of_two() {
            return Err(BexError::InvalidSection);
        }
        if kind == SECTION_CODE {
            if section_flags & SECTION_FLAG_EXEC == 0 {
                return Err(BexError::InvalidSection);
            }
            code_size = Some(mem_size);
        }
        plan.sections[index] = BexMapping {
            kind,
            flags: section_flags,
            file_offset,
            file_size,
            mem_size,
            alignment,
        };
    }

    let code_size = code_size.ok_or(BexError::MissingCode)?;
    if entry_offset >= code_size {
        return Err(BexError::EntryOutsideCode);
    }
    Ok(plan)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// Report the currently available BEX admission capability over serial.
pub fn announce() {
    crate::ring0::dev::console::serial_write(
        "[bex] x86-64 admission gate ready; Ring 3 mapping pending storage/process phase\n",
    );
}
