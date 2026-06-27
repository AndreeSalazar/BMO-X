//! File Descriptor Syscalls (Ring 0 HAL).
//!
//! Provides file I/O services for Ring 3 processes:
//!   - open: Open a file by path
//!   - close: Close a file descriptor
//!   - read: Read bytes from a file
//!   - write: Write bytes to a file
//!   - seek: Move file position
//!
//! Architecture:
//!   - Each process has a file descriptor table (max 256 FDs)
//!   - FDs point to "file objects" (VFS nodes or device nodes)
//!   - Standard FDs: 0=stdin, 1=stdout, 2=stderr
//!
//! These are Ring 0 service stubs — BMO Core calls them
//! when handling Ring 3 syscalls.

/// File descriptor table size per process.
const MAX_FDS: usize = 256;

/// File descriptor flags.
#[derive(Debug, Clone, Copy)]
pub struct FdFlags {
    pub close_on_exec: bool,
    pub non_blocking: bool,
}

/// An open file descriptor.
#[derive(Debug, Clone, Copy)]
pub struct FileDescriptor {
    pub id: u32,
    pub file_type: FileType,
    pub offset: u64,
    pub flags: FdFlags,
    pub in_use: bool,
}

/// Type of file descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file on a filesystem
    File { fs_id: u32, inode: u32 },
    /// Device (keyboard, mouse, serial, framebuffer)
    Device { device_id: u32 },
    /// Pipe (for IPC)
    Pipe { pipe_id: u32 },
    /// Socket (for networking)
    Socket { socket_id: u32 },
    /// Stdin/Stdout/Stderr
    Standard { fd_type: StdFdType },
}

/// Standard file descriptor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdFdType {
    Stdin,
    Stdout,
    Stderr,
}

/// Per-process file descriptor table.
#[derive(Debug)]
pub struct FdTable {
    pub fds: [FileDescriptor; MAX_FDS],
}

impl FdTable {
    pub const fn new() -> Self {
        const EMPTY: FileDescriptor = FileDescriptor {
            id: 0,
            file_type: FileType::Standard { fd_type: StdFdType::Stdin },
            offset: 0,
            flags: FdFlags { close_on_exec: false, non_blocking: false },
            in_use: false,
        };
        Self { fds: [EMPTY; MAX_FDS] }
    }

    /// Allocate a new file descriptor.
    pub fn alloc(&mut self, ft: FileType) -> Option<u32> {
        for i in 3..MAX_FDS { // Skip stdin/stdout/stderr
            if !self.fds[i].in_use {
                self.fds[i] = FileDescriptor {
                    id: i as u32,
                    file_type: ft,
                    offset: 0,
                    flags: FdFlags { close_on_exec: false, non_blocking: false },
                    in_use: true,
                };
                return Some(i as u32);
            }
        }
        None
    }

    /// Get a file descriptor by ID.
    pub fn get(&self, fd: u32) -> Option<&FileDescriptor> {
        if fd as usize >= MAX_FDS { return None; }
        if self.fds[fd as usize].in_use {
            Some(&self.fds[fd as usize])
        } else {
            None
        }
    }

    /// Close a file descriptor.
    pub fn close(&mut self, fd: u32) -> bool {
        if fd as usize >= MAX_FDS { return false; }
        if self.fds[fd as usize].in_use {
            self.fds[fd as usize].in_use = false;
            true
        } else {
            false
        }
    }
}

/// Open a file. Returns file descriptor or error.
pub fn open(path: &str, flags: u32) -> Result<u32, FileError> {
    // TODO: Resolve path via VFS
    // TODO: Check permissions
    // TODO: Create file object
    // TODO: Allocate FD in current process

    crate::dev::console::serial_write("[file] open stub: ");
    crate::dev::console::serial_write(path);
    crate::dev::console::serial_write("\n");

    Err(FileError::NotFound)
}

/// Close a file descriptor.
pub fn close(fd: u32) -> Result<(), FileError> {
    // TODO: Look up FD in current process
    // TODO: Flush pending writes
    // TODO: Release file object reference
    crate::dev::console::serial_write("[file] close stub: fd=");
    crate::dev::console::serial_write_u64(fd as u64, 10);
    crate::dev::console::serial_write("\n");
    Ok(())
}

/// Read bytes from a file descriptor.
pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, FileError> {
    // TODO: Look up FD, dispatch to file/device read
    crate::dev::console::serial_write("[file] read stub: fd=");
    crate::dev::console::serial_write_u64(fd as u64, 10);
    crate::dev::console::serial_write("\n");
    Err(FileError::NotImplemented)
}

/// Write bytes to a file descriptor.
pub fn write(fd: u32, buf: &[u8]) -> Result<usize, FileError> {
    // TODO: Look up FD, dispatch to file/device write
    // For stdout/stderr, write to serial or framebuffer
    if fd <= 2 {
        // Standard output — write to serial
        for &b in buf {
            crate::dev::console::serial_write_byte(b);
        }
        return Ok(buf.len());
    }

    crate::dev::console::serial_write("[file] write stub: fd=");
    crate::dev::console::serial_write_u64(fd as u64, 10);
    crate::dev::console::serial_write("\n");
    Err(FileError::NotImplemented)
}

/// Move file position.
pub fn seek(fd: u32, offset: u64, whence: u32) -> Result<u64, FileError> {
    // TODO: Look up FD, update offset based on whence
    crate::dev::console::serial_write("[file] seek stub\n");
    Err(FileError::NotImplemented)
}

/// File operation error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    NotFound,
    PermissionDenied,
    BadFileDescriptor,
    NotImplemented,
    IOError,
    IsDirectory,
    TooManyOpenFiles,
}
