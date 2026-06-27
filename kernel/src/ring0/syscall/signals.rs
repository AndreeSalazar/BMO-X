//! Signal Syscalls (Ring 0 HAL).
//!
//! Provides signal delivery services for Ring 3 processes:
//!   - kill: Send a signal to a process
//!   - signal: Register a signal handler
//!   - sigaction: Advanced signal handler configuration
//!
//! Architecture:
//!   - Each process has a signal disposition table (64 signals)
//!   - When a signal is delivered, the kernel saves the current context
//!   - The kernel pushes a signal frame onto the user stack
//!   - The user-mode signal handler runs, then calls sigreturn
//!   - The kernel restores the original context
//!
//! These are Ring 0 service stubs — BMO Core calls them
//! when handling Ring 3 syscalls.

/// Standard signal numbers (POSIX).
pub mod signals {
    pub const SIGHUP: u32 = 1;
    pub const SIGINT: u32 = 2;   // Interrupt (Ctrl+C)
    pub const SIGQUIT: u32 = 3;
    pub const SIGILL: u32 = 4;   // Illegal instruction
    pub const SIGTRAP: u32 = 5;
    pub const SIGABRT: u32 = 6;
    pub const SIGBUS: u32 = 7;
    pub const SIGFPE: u32 = 8;
    pub const SIGKILL: u32 = 9;  // Cannot be caught
    pub const SIGUSR1: u32 = 10;
    pub const SIGSEGV: u32 = 11; // Segmentation fault
    pub const SIGUSR2: u32 = 12;
    pub const SIGPIPE: u32 = 13;
    pub const SIGALRM: u32 = 14;
    pub const SIGTERM: u32 = 15;
    pub const SIGCHLD: u32 = 17;
    pub const SIGCONT: u32 = 18;
    pub const SIGSTOP: u32 = 19; // Cannot be caught
    pub const SIGTSTP: u32 = 20;
    pub const SIGTTIN: u32 = 21;
    pub const SIGTTOU: u32 = 22;
    pub const SIGURG: u32 = 23;
    pub const SIGXCPU: u32 = 24;
    pub const SIGXFSZ: u32 = 25;
    pub const SIGVTALRM: u32 = 26;
    pub const SIGPROF: u32 = 27;
    pub const SIGWINCH: u32 = 28;
    pub const SIGIO: u32 = 29;
    pub const SIGPWR: u32 = 30;
    pub const SIGSYS: u32 = 31;
}

/// Signal action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    /// Default action for this signal.
    Default,
    /// Ignore the signal.
    Ignore,
    /// Call a user-mode handler function.
    Handler(u64), // function pointer
    /// Special handling (e.g., stop/continue).
    Special,
}

/// Per-process signal disposition table.
#[derive(Debug)]
pub struct SignalTable {
    pub dispositions: [SignalAction; 64],
    pub pending: u64,    // Bitmask of pending signals
    pub blocked: u64,    // Bitmask of blocked signals
    pub running: bool,   // Is a signal handler currently executing?
}

impl SignalTable {
    pub const fn new() -> Self {
        Self {
            dispositions: [SignalAction::Default; 64],
            pending: 0,
            blocked: 0,
            running: false,
        }
    }
}

/// Send a signal to a process.
pub fn kill(pid: u32, signal: u32) -> Result<(), SignalError> {
    if signal == 0 || signal > 64 {
        return Err(SignalError::InvalidSignal);
    }

    // TODO: Look up process by PID
    // TODO: Check permissions (can sender send to receiver?)
    // TODO: Set pending bit in signal table
    // TODO: If target is sleeping, wake it up

    crate::dev::console::serial_write("[signal] kill stub: pid=");
    crate::dev::console::serial_write_u64(pid as u64, 10);
    crate::dev::console::serial_write(" sig=");
    crate::dev::console::serial_write_u64(signal as u64, 10);
    crate::dev::console::serial_write("\n");

    Ok(())
}

/// Register a signal handler for the current process.
pub fn signal(sig: u32, handler: u64) -> Result<SignalAction, SignalError> {
    if sig == 0 || sig > 64 {
        return Err(SignalError::InvalidSignal);
    }

    // SIGKILL and SIGSTOP cannot be caught
    if sig == signals::SIGKILL || sig == signals::SIGSTOP {
        return Err(SignalError::CannotCatch);
    }

    // TODO: Update disposition in current process's signal table
    let action = if handler == 0 {
        SignalAction::Default
    } else if handler == 1 {
        SignalAction::Ignore
    } else {
        SignalAction::Handler(handler)
    };

    crate::dev::console::serial_write("[signal] signal stub: sig=");
    crate::dev::console::serial_write_u64(sig as u64, 10);
    crate::dev::console::serial_write(" handler=0x");
    crate::dev::console::serial_write_u64(handler, 16);
    crate::dev::console::serial_write("\n");

    Ok(action)
}

/// Check for pending signals and deliver them.
/// Called from the return-to-user-mode path.
pub fn check_pending() {
    // TODO: Check pending & ~blocked signals
    // TODO: For each pending signal:
    //   1. Save current context
    //   2. Set up signal frame on user stack
    //   3. Jump to signal handler
}

/// Signal error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalError {
    InvalidSignal,
    ProcessNotFound,
    PermissionDenied,
    CannotCatch,
}
