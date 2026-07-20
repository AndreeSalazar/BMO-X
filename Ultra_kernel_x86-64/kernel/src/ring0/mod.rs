pub mod bex;
pub mod core {
    pub mod entry;
    pub mod phase;
    pub mod splash;
}
pub mod cpu;
pub mod cpu_vendor;
pub mod channel;
pub mod dev {
    pub mod console;
    pub mod framebuffer;
}
pub mod mm;
pub mod percpu;
pub mod proc;
pub mod scheduler;
pub mod spin;
pub mod syscall;
pub mod timer;
pub mod trap;
