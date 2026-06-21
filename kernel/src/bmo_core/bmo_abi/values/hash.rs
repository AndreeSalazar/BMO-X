//! `hash` — Funciones de hash del BMO ABI.
//!
//! FNV-1a (64-bit) para hash maps y strings.
//! CRC32 para checksums de archivos y datos de red.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u64;

const FNV_OFFSET: bx_u64 = 0xcbf29ce484222325;
const FNV_PRIME:  bx_u64 = 0x100000001b3;

pub fn fnv1a_hash(data: &[u8]) -> bx_u64 {
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as bx_u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn fnv1a_hash_str(s: &str) -> bx_u64 {
    fnv1a_hash(s.as_bytes())
}

const CRC32_POLY: u32 = 0xEDB88320;

static CRC32_TABLE: [u32; 256] = build_crc32_table();

const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ CRC32_POLY;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

pub fn crc32_combine(crc1: u32, crc2: u32, len2: u32) -> u32 {
    let mut crc = crc1;
    let mut i = 0;
    while i < len2 {
        crc = CRC32_TABLE[((crc ^ (crc2 >> (i * 8))) & 0xFF) as usize] ^ (crc >> 8);
        i += 1;
    }
    crc
}

pub fn hash_combine(seed: bx_u64, value: bx_u64) -> bx_u64 {
    seed ^ (value.wrapping_mul(0x9e3779b97f4a7c15))
}
