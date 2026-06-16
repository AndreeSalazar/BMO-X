//! Copying (Semispace) Garbage Collector
//!
//! Copies live objects to a new space, compacting memory in the process.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::super::traits::{GcType, GcPlugin, GcStats};

/// Object header for copying GC
#[repr(C)]
struct GcObjectHeader {
    forwarded: bool,
    size: usize,
    type_id: u32,
}

/// Copying garbage collector (semispace)
pub struct CopyingGc {
    from_space: Vec<u8>,
    to_space: Vec<u8>,
    from_offset: usize,
    to_offset: usize,
    stats: GcStats,
    roots: Vec<usize>,
}

impl CopyingGc {
    pub fn new(heap_size: usize) -> Self {
        Self {
            from_space: Vec::with_capacity(heap_size),
            to_space: Vec::with_capacity(heap_size),
            from_offset: 0,
            to_offset: 0,
            stats: GcStats {
                total_allocated: 0,
                total_freed: 0,
                live_objects: 0,
                collections: 0,
                pause_time_us: 0,
            },
            roots: Vec::new(),
        }
    }

    fn copy_object(&mut self, offset: usize) -> usize {
        if offset >= self.from_space.len() {
            return 0;
        }

        let header = unsafe {
            &*(self.from_space.as_ptr().add(offset) as *const GcObjectHeader)
        };

        // Check if already forwarded
        if header.forwarded {
            // Return new location (stored in first word of object)
            return unsafe {
                *(self.from_space.as_ptr().add(offset + core::mem::size_of::<GcObjectHeader>()) as *const usize)
            };
        }

        let total_size = core::mem::size_of::<GcObjectHeader>() + header.size;

        // Copy to to_space
        let new_offset = self.to_offset;
        if new_offset + total_size > self.to_space.len() {
            return 0; // Out of space
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                self.from_space.as_ptr().add(offset),
                self.to_space.as_mut_ptr().add(new_offset),
                total_size,
            );
        }

        // Update forwarded flag and store new location
        let new_header = unsafe {
            &mut *(self.to_space.as_mut_ptr().add(new_offset) as *mut GcObjectHeader)
        };
        new_header.forwarded = false;

        // Store new location in old object for forwarding pointer
        let old_header = unsafe {
            &mut *(self.from_space.as_mut_ptr().add(offset) as *mut GcObjectHeader)
        };
        old_header.forwarded = true;
        unsafe {
            *(self.from_space.as_mut_ptr().add(offset + core::mem::size_of::<GcObjectHeader>()) as *mut usize) = new_offset;
        }

        self.to_offset = new_offset + total_size;
        new_offset
    }

    fn update_references(&mut self) {
        // Update all pointers in copied objects
        for i in 0..self.to_offset {
            if i + 8 <= self.to_offset {
                let ptr_val = unsafe {
                    *(self.to_space.as_ptr().add(i) as *const usize)
                };

                // Check if pointer points to from_space
                if ptr_val >= self.from_space.as_ptr() as usize
                    && ptr_val < self.from_space.as_ptr() as usize + self.from_space.len()
                {
                    let old_offset = ptr_val - self.from_space.as_ptr() as usize;
                    let new_offset = self.copy_object(old_offset);
                    unsafe {
                        *(self.to_space.as_mut_ptr().add(i) as *mut usize) = new_offset;
                    }
                }
            }
        }
    }

    fn swap_spaces(&mut self) {
        core::mem::swap(&mut self.from_space, &mut self.to_space);
        core::mem::swap(&mut self.from_offset, &mut self.to_offset);
        self.to_offset = 0;
    }
}

impl GcPlugin for CopyingGc {
    fn gc_type(&self) -> GcType {
        GcType::Copying
    }

    fn init(&mut self, heap_size: usize) -> BxResult<()> {
        self.from_space = Vec::with_capacity(heap_size);
        self.to_space = Vec::with_capacity(heap_size);
        self.from_offset = 0;
        self.to_offset = 0;
        self.stats = GcStats {
            total_allocated: 0,
            total_freed: 0,
            live_objects: 0,
            collections: 0,
            pause_time_us: 0,
        };
        Ok(())
    }

    fn alloc(&mut self, size: usize) -> BxResult<*mut u8> {
        let total_size = core::mem::size_of::<GcObjectHeader>() + size;

        // Check if we need GC
        if self.from_offset + total_size > self.from_space.len() / 2 {
            self.collect()?;
        }

        // If still no space, error
        if self.from_offset + total_size > self.from_space.capacity() {
            return Err(crate::barex::BxError::OutOfMemory);
        }

        let offset = self.from_offset;
        self.from_space.resize(offset + total_size, 0);

        let header = unsafe {
            &mut *(self.from_space.as_mut_ptr().add(offset) as *mut GcObjectHeader)
        };
        header.forwarded = false;
        header.size = size;
        header.type_id = 0;

        self.from_offset += total_size;
        self.stats.total_allocated += total_size;
        self.stats.live_objects += 1;

        Ok(unsafe { self.from_space.as_mut_ptr().add(offset + core::mem::size_of::<GcObjectHeader>()) })
    }

    fn mark(&mut self, ptr: *mut u8) -> BxResult<()> {
        let offset = ptr as usize - self.from_space.as_ptr() as usize;
        if offset < self.from_space.len() {
            self.roots.push(offset);
        }
        Ok(())
    }

    fn sweep(&mut self) -> BxResult<usize> {
        // Copying GC doesn't sweep - it copies live objects
        Ok(0)
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }

    fn needs_gc(&self) -> bool {
        self.from_offset > self.from_space.len() / 2
    }

    fn collect(&mut self) -> BxResult<usize> {
        let start_us = self.get_time_us();
        let freed = self.from_offset;

        // Copy all live objects
        for &root in &self.roots {
            self.copy_object(root);
        }

        // Update references
        self.update_references();

        // Swap spaces
        self.swap_spaces();

        let end_us = self.get_time_us();
        self.stats.collections += 1;
        self.stats.pause_time_us = end_us - start_us;
        self.stats.total_freed += freed;

        Ok(freed)
    }
}

impl CopyingGc {
    fn get_time_us(&self) -> u64 {
        0
    }
}
