pub mod bex;
pub mod cabina;
pub mod fs;
pub mod estratos;
/// Endpoint RPC: un proceso Ring 3 puede atender a otro.
pub mod endpoint;
pub mod cap;
pub mod faults;
pub mod fb;
pub mod consola;
pub mod directorio;
pub mod input;
pub mod lanzar;
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
    pub mod disk;
    pub mod keyboard;
    pub mod pci;
    pub mod usb;
}
pub mod mm;
pub mod percpu;
pub mod proc;
pub mod reinicio;
pub mod scheduler;
pub mod spin;
pub mod svc;
pub mod syscall;
pub mod timer;
pub mod uconsole;
pub mod trap;
