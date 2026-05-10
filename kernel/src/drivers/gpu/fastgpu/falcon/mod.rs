//! Generic Falcon Engine Abstraction
//! All engines based on the Falcon architecture (SEC2, PMU, FECS, GPCCS) implement this.

pub trait FalconEngine {
    /// Resets the engine via PMC.
    fn reset(&mut self) -> Result<(), &'static str>;

    /// Loads authenticated microcode into IMEM.
    fn load_imem(&mut self, data: &[u8]) -> Result<(), &'static str>;

    /// Loads configuration and signature data into DMEM.
    fn load_dmem(&mut self, data: &[u8]) -> Result<(), &'static str>;

    /// Sets the BOOTVEC register.
    fn set_bootvec(&mut self, vec: u32) -> Result<(), &'static str>;

    /// Starts the CPU via CPUCTL.
    fn start_cpu(&mut self) -> Result<(), &'static str>;

    /// Validates if the engine has entered High Secure (HS) mode.
    fn validate_hs_mode(&self) -> Result<bool, &'static str>;

    /// Handles an IRQ from the engine.
    fn handle_irq(&mut self) -> Result<(), &'static str>;
}
