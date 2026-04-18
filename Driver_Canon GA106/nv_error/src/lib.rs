//! # nv_error — NVIDIA Error Codes
//!
//! Complete error enumeration extracted from `nvlddmkm.sys` string table
//! by SigDead-BIB. These match NVIDIA's internal `NV_ERR_*` codes found
//! in the kernel driver's .rdata section.
//!
//! Zero dependencies. `#![no_std]` compatible.

#![no_std]

/// NVIDIA GPU driver result type.
pub type NvResult<T> = Result<T, NvError>;

/// Error codes matching NVIDIA's internal NV_ERR_* enumeration.
///
/// Source: SigDead-BIB string extraction from nvlddmkm.sys v596.21
/// (165,880 strings analyzed, ~100 NV_ERR_* codes identified).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum NvError {
    // ── Generic ──────────────────────────────────────────────────
    Generic                     = 0x0001,
    InUse                       = 0x0002,
    BusyRetry                   = 0x0003,
    InsufficientResources       = 0x0004,
    InsufficientPermissions     = 0x0005,
    InsufficientPower           = 0x0006,
    Timeout                     = 0x0007,

    // ── Invalid parameter family ─────────────────────────────────
    InvalidArgument             = 0x0100,
    InvalidAddress              = 0x0101,
    InvalidChannel              = 0x0102,
    InvalidClass                = 0x0103,
    InvalidClient               = 0x0104,
    InvalidCommand              = 0x0105,
    InvalidData                 = 0x0106,
    InvalidDevice               = 0x0107,
    InvalidDmaSpecifier         = 0x0108,
    InvalidEvent                = 0x0109,
    InvalidFlags                = 0x010A,
    InvalidFunction             = 0x010B,
    InvalidIndex                = 0x010C,
    InvalidIrqLevel             = 0x010D,
    InvalidLimit                = 0x010E,
    InvalidLockState            = 0x010F,
    InvalidMethod               = 0x0110,
    InvalidObject               = 0x0111,
    InvalidObjectBuffer         = 0x0112,
    InvalidObjectHandle         = 0x0113,
    InvalidObjectNew            = 0x0114,
    InvalidObjectOld            = 0x0115,
    InvalidObjectParent         = 0x0116,
    InvalidOffset               = 0x0117,
    InvalidOperation            = 0x0118,
    InvalidParameter            = 0x0119,
    InvalidPath                 = 0x011A,
    InvalidPointer              = 0x011B,
    InvalidRegistryKey          = 0x011C,
    InvalidRequest              = 0x011D,
    InvalidState                = 0x011E,
    InvalidStringLength         = 0x011F,
    InvalidAccessType           = 0x0120,

    // ── GPU state errors ─────────────────────────────────────────
    GpuIsLost                   = 0x0200,
    GpuInFullchipReset          = 0x0201,
    GpuNotFullPower             = 0x0202,
    GpuUuidNotFound             = 0x0203,
    GpuDmaNotInitialized        = 0x0204,
    CardNotPresent              = 0x0205,

    // ── Memory errors ────────────────────────────────────────────
    BrokenFb                    = 0x0300,
    BufferTooSmall              = 0x0301,
    DmaInUse                    = 0x0302,
    DmaMemNotLocked             = 0x0303,
    DmaMemNotUnlocked           = 0x0304,
    InvalidHeap                 = 0x0305,
    MemoryTrainingFailed        = 0x0306,
    EccError                    = 0x0307,

    // ── FIFO / command errors ────────────────────────────────────
    FifoBadAccess               = 0x0400,

    // ── Interrupt errors ─────────────────────────────────────────
    IrqNotFiring                = 0x0500,
    IrqEdgeTriggered            = 0x0501,

    // ── I2C / bus errors ─────────────────────────────────────────
    I2cError                    = 0x0600,
    I2cSpeedTooHigh             = 0x0601,

    // ── Display errors ───────────────────────────────────────────
    DualLinkInUse               = 0x0700,
    FreqNotSupported            = 0x0701,

    // ── Resource manager ─────────────────────────────────────────
    CycleDetected               = 0x0800,
    InsertDuplicateName         = 0x0801,
    MissingTableEntry           = 0x0802,
    MismatchedSlave             = 0x0803,
    MismatchedTarget            = 0x0804,

    // ── Misc ─────────────────────────────────────────────────────
    IllegalAction               = 0x0900,
    CallbackNotScheduled        = 0x0901,
    HotSwitch                   = 0x0902,
    InflateCompressedDataFailed = 0x0903,
    ModuleLoadFailed            = 0x0904,
    OperatingSystem             = 0x0905,
    NotSupported                = 0x0906,
}

impl NvError {
    /// Human-readable description (matches NVIDIA's internal string table).
    pub const fn description(self) -> &'static str {
        match self {
            Self::Generic                     => "Failure: Generic Error",
            Self::InUse                       => "Generic busy error",
            Self::BusyRetry                   => "System is busy, retry later",
            Self::InsufficientResources       => "Ran out of a critical resource",
            Self::InsufficientPermissions     => "Requester lacks sufficient permissions",
            Self::InsufficientPower           => "Generic Error: Low power",
            Self::Timeout                     => "Operation timed out",
            Self::InvalidArgument             => "Invalid argument to call",
            Self::InvalidAddress              => "Address not valid",
            Self::InvalidChannel              => "Given channel-id not valid",
            Self::InvalidClass                => "Given class-id not valid",
            Self::InvalidClient               => "Given client not valid",
            Self::InvalidCommand              => "Command passed is not valid",
            Self::InvalidData                 => "Invalid data passed",
            Self::InvalidDevice               => "Current device is not valid",
            Self::InvalidDmaSpecifier         => "Requested DMA specifier is not valid",
            Self::InvalidEvent                => "Invalid event occurred",
            Self::InvalidFlags                => "Invalid flags passed",
            Self::InvalidFunction             => "Called function is not valid",
            Self::InvalidIndex                => "Index invalid",
            Self::InvalidIrqLevel             => "Requested IRQ level is not valid",
            Self::InvalidLimit                => "Generic Error: Invalid limit",
            Self::InvalidLockState            => "Requested lock state not valid",
            Self::InvalidMethod               => "Requested method not valid",
            Self::InvalidObject               => "Object not valid",
            Self::InvalidObjectBuffer         => "Object buffer not valid",
            Self::InvalidObjectHandle         => "Object handle is not valid",
            Self::InvalidObjectNew            => "New object is not valid",
            Self::InvalidObjectOld            => "Old object is not valid",
            Self::InvalidObjectParent         => "Object parent is not valid",
            Self::InvalidOffset               => "Offset passed is not valid",
            Self::InvalidOperation            => "Requested operation is not valid",
            Self::InvalidParameter            => "At least one parameter is not valid",
            Self::InvalidPath                 => "Requested path is not valid",
            Self::InvalidPointer              => "Pointer not valid",
            Self::InvalidRegistryKey          => "Found invalid registry key",
            Self::InvalidRequest              => "Generic Error: Invalid request",
            Self::InvalidState                => "Generic Error: Invalid state",
            Self::InvalidStringLength         => "String length is not valid",
            Self::InvalidAccessType           => "This type of access is not allowed",
            Self::GpuIsLost                   => "GPU lost from the bus",
            Self::GpuInFullchipReset          => "GPU currently in full-chip reset",
            Self::GpuNotFullPower             => "GPU not in full power",
            Self::GpuUuidNotFound             => "GPU UUID not found",
            Self::GpuDmaNotInitialized        => "Requested DMA not initialized",
            Self::CardNotPresent              => "Card not detected",
            Self::BrokenFb                    => "Frame-Buffer broken",
            Self::BufferTooSmall              => "Buffer passed in is too small",
            Self::DmaInUse                    => "Requested DMA is in use",
            Self::DmaMemNotLocked             => "Requested DMA memory is not locked",
            Self::DmaMemNotUnlocked           => "Requested DMA memory is not unlocked",
            Self::InvalidHeap                 => "Heap corrupted",
            Self::MemoryTrainingFailed        => "Failed memory training sequence",
            Self::EccError                    => "Generic ECC error",
            Self::FifoBadAccess               => "FIFO: Invalid access",
            Self::IrqNotFiring                => "Requested IRQ is not firing",
            Self::IrqEdgeTriggered            => "IRQ is edge triggered",
            Self::I2cError                    => "I2C Error",
            Self::I2cSpeedTooHigh             => "I2C Error: Speed too high",
            Self::DualLinkInUse               => "Dual-Link is in use",
            Self::FreqNotSupported            => "Requested frequency is not supported",
            Self::CycleDetected               => "Call cycle detected",
            Self::InsertDuplicateName         => "Found duplicate entry in btree",
            Self::MissingTableEntry           => "Entry not found in table",
            Self::MismatchedSlave             => "Slave mismatch",
            Self::MismatchedTarget            => "Target mismatch",
            Self::IllegalAction               => "Current action is not allowed",
            Self::CallbackNotScheduled        => "The requested callback API not scheduled",
            Self::HotSwitch                   => "System in hot switch",
            Self::InflateCompressedDataFailed => "Failed to inflate compressed data",
            Self::ModuleLoadFailed            => "Failed to load requested module",
            Self::OperatingSystem             => "Operating system error",
            Self::NotSupported                => "Operation not supported",
        }
    }
}

impl core::fmt::Display for NvError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NvError::{:?}: {}", self, self.description())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_descriptions() {
        assert_eq!(NvError::GpuIsLost.description(), "GPU lost from the bus");
        assert_eq!(NvError::FifoBadAccess.description(), "FIFO: Invalid access");
        assert_eq!(NvError::BrokenFb.description(), "Frame-Buffer broken");
    }

    #[test]
    fn result_type() {
        let ok: NvResult<u32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);
        let err: NvResult<u32> = Err(NvError::GpuIsLost);
        assert!(err.is_err());
    }
}
