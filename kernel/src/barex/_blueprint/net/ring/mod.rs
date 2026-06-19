//! Submission/Completion Queues estilo io_uring para red.
//!
//! Reemplaza:
//!   - Win32 IOCP / `WSAOVERLAPPED` callbacks
//!   - Linux epoll, kqueue, /dev/poll
//!   - libuv, libevent, libev (event loop libs userspace)
//!
//! Una sola pareja SQ/CQ por proceso. App empuja SQEs (envía/recibe/conecta);
//! kernel completa y empuja CQEs. Cero syscalls por op (batch).

pub mod sqe;
pub mod cqe;
pub mod queue;

