//! PE Authenticode Signature Parser — no_std, bare-metal.
//!
//! Parses PE32+ headers to extract the Security Directory (Authenticode)
//! and reads signer/issuer information from embedded PKCS#7 certificates.

use alloc::string::String;
use alloc::format;

/// Extracted signature information from a PE binary.
pub struct PeSignatureInfo {
    pub pe_timestamp: u32,
    pub machine: u16,
    pub signature_offset: u32,
    pub signature_size: u32,
    pub signer_name: String,
    pub issuer_name: String,
    pub cert_serial_hex: String,
}

/// Try to parse PE signature info from raw file bytes.
/// Only reads the first ~4KB (headers) + the security directory.
pub fn parse_pe_signature(data: &[u8]) -> Option<PeSignatureInfo> {
    // ── 1. DOS Header ──
    if data.len() < 64 {
        return None;
    }
    // MZ magic
    if data[0] != b'M' || data[1] != b'Z' {
        return None;
    }
    let e_lfanew = read_u32(data, 0x3C) as usize;
    if e_lfanew + 4 > data.len() {
        return None;
    }

    // ── 2. PE Signature ──
    if &data[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }

    // ── 3. COFF Header (20 bytes after PE sig) ──
    let coff_offset = e_lfanew + 4;
    if coff_offset + 20 > data.len() {
        return None;
    }
    let machine = read_u16(data, coff_offset);
    let pe_timestamp = read_u32(data, coff_offset + 4);
    let optional_hdr_size = read_u16(data, coff_offset + 16) as usize;

    // ── 4. Optional Header ──
    let opt_offset = coff_offset + 20;
    if opt_offset + optional_hdr_size > data.len() {
        return None;
    }
    let magic = read_u16(data, opt_offset);
    // PE32+ (64-bit) = 0x20B, PE32 (32-bit) = 0x10B
    let is_pe32plus = magic == 0x20B;

    // Data directories start at different offsets for PE32 vs PE32+
    // PE32+: opt_offset + 112 (after 108 bytes of fixed fields + 4 for NumberOfRvaAndSizes)
    // PE32:  opt_offset + 96
    let dd_offset = if is_pe32plus {
        opt_offset + 112
    } else {
        opt_offset + 96
    };

    // Check NumberOfRvaAndSizes — we need at least 5 entries (index 4 = Security)
    let num_dd = read_u32(data, dd_offset - 4) as usize;
    if num_dd < 5 {
        return None; // No security directory
    }

    // ── 5. Security Directory (DataDirectory[4]) ──
    // Each DD entry is 8 bytes (RVA + Size)
    let sec_dd_offset = dd_offset + 4 * 8; // index 4, each entry 8 bytes
    if sec_dd_offset + 8 > data.len() {
        return None;
    }
    let sec_rva = read_u32(data, sec_dd_offset);
    let sec_size = read_u32(data, sec_dd_offset + 4);

    if sec_rva == 0 || sec_size == 0 {
        return None; // Not signed
    }

    // ── 6. WIN_CERTIFICATE struct ──
    // Security directory uses raw file offset (not RVA!)
    let cert_offset = sec_rva as usize;
    if cert_offset + 8 > data.len() {
        // We might not have enough data — return what we have
        return Some(PeSignatureInfo {
            pe_timestamp,
            machine,
            signature_offset: sec_rva,
            signature_size: sec_size,
            signer_name: String::from("[signature beyond read range]"),
            issuer_name: String::from("[signature beyond read range]"),
            cert_serial_hex: String::new(),
        });
    }

    let _cert_len = read_u32(data, cert_offset);
    let cert_rev = read_u16(data, cert_offset + 4);
    let cert_type = read_u16(data, cert_offset + 6);

    // Revision 0x0200 = WIN_CERT_REVISION_2_0, Type 0x0002 = WIN_CERT_TYPE_PKCS_SIGNED_DATA
    let _is_pkcs7 = cert_rev == 0x0200 && cert_type == 0x0002;

    // ── 7. Parse PKCS#7 / X.509 from DER-encoded data ──
    let pkcs7_start = cert_offset + 8;
    let pkcs7_end = (cert_offset + sec_size as usize).min(data.len());

    let (signer, issuer, serial) = if pkcs7_start < pkcs7_end {
        extract_x509_names(&data[pkcs7_start..pkcs7_end])
    } else {
        (String::from("[no cert data]"), String::from("[no cert data]"), String::new())
    };

    Some(PeSignatureInfo {
        pe_timestamp,
        machine,
        signature_offset: sec_rva,
        signature_size: sec_size,
        signer_name: signer,
        issuer_name: issuer,
        cert_serial_hex: serial,
    })
}

/// Scan DER-encoded PKCS#7 SignedData for X.509 certificate names.
/// This is a best-effort parser — looks for common OID patterns in the ASN.1 stream.
fn extract_x509_names(der: &[u8]) -> (String, String, String) {
    // Strategy: find the OID for CommonName (2.5.4.3 = 55 04 03),
    // then read the following UTF8String / PrintableString value.
    // In a typical Authenticode PKCS#7, the first CN is the signer,
    // and the second CN is the issuer.

    let cn_oid: &[u8] = &[0x55, 0x04, 0x03]; // OID 2.5.4.3 (CommonName)
    let mut names: alloc::vec::Vec<String> = alloc::vec::Vec::new();
    let mut serial = String::new();

    // Also try to find the serial number (in the SignerInfo or TBSCertificate)
    // TBSCertificate serialNumber is typically right after the version field

    let mut i = 0;
    while i + cn_oid.len() + 4 < der.len() {
        // Look for CN OID
        if der[i..].starts_with(cn_oid) {
            // OID found. The value follows after the OID encoding.
            // Typical: ... 55 04 03 [tag] [len] [value bytes]
            let val_offset = i + cn_oid.len();
            if val_offset + 2 < der.len() {
                let tag = der[val_offset];
                let len = der[val_offset + 1] as usize;
                // Tags: 0x0C = UTF8String, 0x13 = PrintableString, 0x16 = IA5String
                if (tag == 0x0C || tag == 0x13 || tag == 0x16) && val_offset + 2 + len <= der.len() {
                    let name_bytes = &der[val_offset + 2..val_offset + 2 + len];
                    if let Ok(name) = core::str::from_utf8(name_bytes) {
                        names.push(String::from(name));
                    }
                }
            }
        }
        i += 1;
    }

    // Extract serial number — look for INTEGER tag (0x02) with reasonable length
    // after the certificate version marker [0] EXPLICIT
    let mut found_serial = false;
    i = 0;
    while i + 3 < der.len() && !found_serial {
        // Look for SEQUENCE > SEQUENCE > version [0] EXPLICIT > INTEGER (serial)
        if der[i] == 0x02 {
            let slen = der[i + 1] as usize;
            if slen >= 4 && slen <= 20 && i + 2 + slen <= der.len() {
                // This looks like a serial number
                for b in &der[i + 2..i + 2 + slen] {
                    serial.push_str(&format!("{:02x}", b));
                }
                found_serial = true;
            }
        }
        i += 1;
    }

    let signer = names.first().cloned().unwrap_or_else(|| String::from("[unknown signer]"));
    let issuer = names.get(1).cloned().unwrap_or_else(|| String::from("[unknown issuer]"));

    (signer, issuer, serial)
}

// ── Little-endian readers ──

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}
