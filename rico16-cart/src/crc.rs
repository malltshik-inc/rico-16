//! SPEC §13.2.3, §20.6 — CRC-32 (IEEE), used by the PNG blob header.

/// SPEC §20.6 — CRC-32 (IEEE) of `bytes`.
pub fn crc32(bytes: &[u8]) -> u32 {
    crc32fast::hash(bytes)
}
