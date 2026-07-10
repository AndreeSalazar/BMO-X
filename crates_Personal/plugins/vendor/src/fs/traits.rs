/// Filesystem trait — generic interface for all filesystem types.
pub trait FileSystem {
    type Error;

    /// Open a file by path.
    fn open(&mut self, path: &str) -> Result<FileHandle, Self::Error>;

    /// Create/overwrite a file.
    fn create(&mut self, path: &str) -> Result<FileHandle, Self::Error>;

    /// Delete a file.
    fn delete(&mut self, path: &str) -> Result<(), Self::Error>;

    /// Check if a file exists.
    fn exists(&mut self, path: &str) -> bool;

    /// Get file metadata.
    fn metadata(&mut self, path: &str) -> Result<FileMetadata, Self::Error>;

    /// List directory contents.
    fn read_dir(&mut self, path: &str) -> Result<alloc::vec::Vec<FileMetadata>, Self::Error>;
}

/// File handle for read/write operations.
pub struct FileHandle {
    pub id: u32,
    pub offset: u64,
    pub size: u64,
}

impl FileHandle {
    pub fn new(id: u32) -> Self {
        Self { id, offset: 0, size: 0 }
    }
}

/// File metadata.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub name: alloc::string::String,
    pub size: u64,
    pub file_type: FileType,
    pub cluster: u32,
}

/// File type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

/// Read/Write trait for file handles.
pub trait FileOps {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError>;
    fn seek(&mut self, offset: u64) -> Result<(), FsError>;
    fn tell(&self) -> u64;
    fn size(&self) -> u64;
}

use crate::fs::FsError;
