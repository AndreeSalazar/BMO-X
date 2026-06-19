//! Sockets BMO. Reemplazan `SOCKET` (Win32) y `int fd` (POSIX) con
//! `BmoHandle` con generación. Sin `WSAStartup`, sin `socket(AF_INET, ...)`.

pub mod tcp;
pub mod udp;
pub mod state;

