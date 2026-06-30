#![allow(dead_code)]

#[repr(C)]
pub struct ImageImportDescriptor {
    pub original_first_thunk: u32,
    pub time_date_stamp: u32,
    pub forwarder_chain: u32,
    pub name: u32,
    pub first_thunk: u32,
}

impl ImageImportDescriptor {
    pub fn is_terminator(&self) -> bool {
        self.original_first_thunk == 0 && self.time_date_stamp == 0
            && self.forwarder_chain == 0 && self.name == 0 && self.first_thunk == 0
    }
}

#[repr(C)]
pub struct ImageThunk(pub u64);

impl ImageThunk {
    pub fn is_terminator(&self) -> bool {
        self.0 == 0
    }

    pub fn name_rva(&self) -> Option<u32> {
        if self.0 & 0x8000_0000_0000_0000 != 0 { None } else { Some(self.0 as u32) }
    }

    pub fn ordinal(&self) -> Option<u16> {
        if self.0 & 0x8000_0000_0000_0000 != 0 { Some((self.0 & 0xFFFF) as u16) } else { None }
    }
}

pub fn read_cstr(bytes: &[u8], offset: usize, max_len: usize) -> Option<&str> {
    let end = offset + max_len;
    if end > bytes.len() { return None; }
    let null_pos = bytes[offset..end].iter().position(|&b| b == 0).unwrap_or(max_len);
    core::str::from_utf8(&bytes[offset..offset + null_pos]).ok()
}
