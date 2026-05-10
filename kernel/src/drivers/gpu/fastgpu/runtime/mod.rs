//! Runtime Orchestration
//! Manages boot ordering, dependency validation, and capability staging.

pub mod payload_loader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuCapabilityStage {
    PciDetected,
    BarMapped,
    MmioAlive,
    Sec2Detected,
    FalconResetReleased,
    ImemUploaded,
    DmemUploaded,
    BootvecConfigured,
    CpuStarted,
    HsModeObserved,
    GspReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuRuntimeMode {
    ObserveOnly,
    DryRun,
    Active,
}

pub struct GpuRuntime {
    pub stage: GpuCapabilityStage,
    pub mode: GpuRuntimeMode,
}

impl GpuRuntime {
    pub const fn new(mode: GpuRuntimeMode) -> Self {
        Self {
            stage: GpuCapabilityStage::PciDetected,
            mode,
        }
    }

    pub fn advance_to(&mut self, stage: GpuCapabilityStage) {
        crate::evidence_println!("[RUNTIME] Transition: {:?} -> {:?}", self.stage, stage);
        self.stage = stage;
    }
}
