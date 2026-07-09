//! `defense::scanner` — Análisis estático de un BEF.

#![allow(dead_code)]

/// Resultado de escanear un BEF.
pub struct ScanResult {
    /// Magic correcto.
    pub magic_ok: bool,
    /// Versión de BEF.
    pub version: (u8, u8),
    /// # de secciones.
    pub section_count: u16,
    /// # de relocalizaciones.
    pub reloc_count: u32,
    /// ¿Tiene sección W+X?
    pub has_wx: bool,
    /// # de imports ABI.
    pub import_count: u16,
}

impl ScanResult {
    pub fn is_well_formed(&self) -> bool {
        self.magic_ok
            && self.section_count > 0
            && self.section_count < 1024
    }
}

/// Escanea un BEF y devuelve el resultado.
pub fn scan(name: &str, bytes: &[u8]) -> ScanResult {
    if bytes.len() < 48 {
        return ScanResult {
            magic_ok: false, version: (0, 0), section_count: 0,
            reloc_count: 0, has_wx: false, import_count: 0,
        };
    }
    let magic_ok = &bytes[0..4] == b"BEF1";
    let version_major = bytes[4];
    let version_minor = bytes[5];
    let section_count = u16::from_le_bytes([bytes[12], bytes[13]]);
    let reloc_count = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let import_count = u16::from_le_bytes([bytes[16], bytes[17]]);
    let _ = name;
    let has_wx = has_wx_section(bytes);
    ScanResult {
        magic_ok, version: (version_major, version_minor),
        section_count, reloc_count, has_wx, import_count,
    }
}

/// ¿El BEF tiene alguna sección con permisos W+X?
pub fn has_wx_section(bytes: &[u8]) -> bool {
    if bytes.len() < 48 { return false; }
    let section_count = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let section_off = 48usize;
    let entry_size = 32usize;
    for i in 0..section_count {
        let off = section_off + i * entry_size;
        if off + entry_size > bytes.len() { break; }
        // flags en offset 8 dentro de cada entry (heurística).
        let flags = u32::from_le_bytes([
            bytes[off + 8], bytes[off + 9], bytes[off + 10], bytes[off + 11],
        ]);
        // Bit 1 = WRITE, Bit 2 = EXEC.
        if (flags & 0x06) == 0x06 { return true; }
    }
    false
}
