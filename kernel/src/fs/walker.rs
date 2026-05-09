//! Recursive directory traversal for NTFS.

use crate::fs::ntfs::NtfsWrapper;
use crate::fs::DiskReader;
use ntfs::{Ntfs, NtfsFile};
use alloc::string::String;
use alloc::string::ToString;

pub struct FileWalker<'a, D: DiskReader> {
    ntfs: &'a Ntfs,
    disk: &'a mut NtfsWrapper<D>,
}

impl<'a, D: DiskReader> FileWalker<'a, D> {
    pub fn new(ntfs: &'a Ntfs, disk: &'a mut NtfsWrapper<D>) -> Self {
        Self { ntfs, disk }
    }

    pub fn walk<F>(&mut self, mut callback: F)
    where
        F: FnMut(&str, &NtfsFile, &mut NtfsWrapper<D>),
    {
        if let Ok(root) = self.ntfs.root_directory(self.disk) {
            self.walk_recursive("", root, &mut callback);
        }
    }

    fn walk_recursive<F>(&mut self, current_path: &str, dir: NtfsFile, callback: &mut F)
    where
        F: FnMut(&str, &NtfsFile, &mut NtfsWrapper<D>),
    {
        let index = match dir.directory_index(self.disk) {
            Ok(idx) => idx,
            Err(_) => return,
        };

        let mut iter = index.entries();
        while let Some(entry_res) = iter.next(self.disk) {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            let name_res = entry.key();
            let name = match name_res {
                Some(Ok(n)) => match n.name().to_string() {
                    Ok(s) => s,
                    Err(_) => continue,
                },
                _ => continue,
            };

            if name == "." || name == ".." {
                continue;
            }

            let mut full_path = String::from(current_path);
            if !full_path.is_empty() {
                full_path.push('\\');
            }
            full_path.push_str(&name);

            let file = match entry.to_file(self.ntfs, self.disk) {
                Ok(f) => f,
                Err(_) => continue,
            };

            callback(&full_path, &file, self.disk);

            if file.is_directory() {
                self.walk_recursive(&full_path, file, callback);
            }
        }
    }
}
