//! Storage drivers — NVMe/AHCI (future).

#[derive(Debug, Clone, Copy)]
pub enum StorageType {
    Nvme,
    Ahci,
    Unknown,
}
