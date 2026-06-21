//! Mark-and-Sweep Garbage Collector
//!
//! Traditional tracing GC that marks reachable objects and sweeps unreachable ones.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::super::traits::{GcType, GcPlugin, GcStats};

/// Object header for mark-and-sweep
#[repr(C)]
struct GcObjectHeader {
    marked: bool,
    size: usize,
    type_id: u32,
}

/// Mark-and-Sweep garbage collector
pub struct MarkSweepGc {
    heap: Vec<u8>,
    objects: Vec<usize>,  // Offsets to object headers
    stats: GcStats,
    mark_stack: Vec<usize>,
}

impl MarkSweepGc {
    pub fn new(heap_size: usize) -> Self {
        Self {
            heap: Vec::with_capacity(heap_size),
            objects: Vec::new(),
            stats: GcStats {
                total_allocated: 0,
                total_freed: 0,
                live_objects: 0,
                collections: 0,
                pause_time_us: 0,
            },
            mark_stack: Vec::new(),
        }
    }

    fn mark_object(&mut self, offset: usize) {
        if offset >= self.heap.len() {
            return;
        }
        let header = unsafe { &mut *(self.heap.as_mut_ptr().add(offset) as *mut GcObjectHeader) };
        if !header.marked {
            header.marked = true;
            self.mark_stack.push(offset);
        }
    }

    fn trace_root(&mut self, roots: &[usize]) {
        for &root in roots {
            self.mark_object(root);
        }
    }

    fn trace(&mut self) {
        while let Some(offset) = self.mark_stack.pop() {
            let header = unsafe { &*(self.heap.as_ptr().add(offset) as *const GcObjectHeader) };
            let size = header.size;

            // Trace pointers within object (simplified)
            let obj_start = offset + core::mem::size_of::<GcObjectHeader>();
            let obj_end = obj_start + size;

            for i in (obj_start..obj_end).step_by(8) {
                if i + 8 <= self.heap.len() {
                    let ptr_val = unsafe {
                        *(self.heap.as_ptr().add(i) as *const usize)
                    };
                    // Check if pointer points to heap
                    if ptr_val >= self.heap.as_ptr() as usize
                        && ptr_val < self.heap.as_ptr() as usize + self.heap.len()
                    {
                        let ptr_offset = ptr_val - self.heap.as_ptr() as usize;
                        self.mark_object(ptr_offset);
                    }
                }
            }
        }
    }

    fn do_sweep(&mut self) -> usize {
        let mut freed = 0;
        let mut i = 0;

        while i < self.objects.len() {
            let offset = self.objects[i];
            let header = unsafe { &*(self.heap.as_ptr().add(offset) as *const GcObjectHeader) };

            if !header.marked {
                // Free object
                freed += header.size + core::mem::size_of::<GcObjectHeader>();
                self.objects.swap_remove(i);
                self.stats.total_freed += header.size;
                self.stats.live_objects -= 1;
            } else {
                // Unmark for next cycle
                let header_mut = unsafe { &mut *(self.heap.as_mut_ptr().add(offset) as *mut GcObjectHeader) };
                header_mut.marked = false;
                i += 1;
            }
        }

        freed
    }
}

impl GcPlugin for MarkSweepGc {
    fn gc_type(&self) -> GcType {
        GcType::MarkSweep
    }

    fn init(&mut self, heap_size: usize) -> BxResult<()> {
        self.heap = Vec::with_capacity(heap_size);
        self.objects.clear();
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
        if self.heap.len() + total_size > self.heap.capacity() {
            self.collect()?;
        }

        // If still no space, error
        if self.heap.len() + total_size > self.heap.capacity() {
            return Err(crate::bmo_gpu::BxError::OutOfMemory);
        }

        let offset = self.heap.len();
        self.heap.resize(offset + total_size, 0);

        let header = unsafe {
            &mut *(self.heap.as_mut_ptr().add(offset) as *mut GcObjectHeader)
        };
        header.marked = false;
        header.size = size;
        header.type_id = 0;

        self.objects.push(offset);
        self.stats.total_allocated += total_size;
        self.stats.live_objects += 1;

        Ok(unsafe { self.heap.as_mut_ptr().add(offset + core::mem::size_of::<GcObjectHeader>()) })
    }

    fn mark(&mut self, ptr: *mut u8) -> BxResult<()> {
        let offset = ptr as usize - self.heap.as_ptr() as usize;
        if offset < self.heap.len() {
            self.mark_object(offset);
        }
        Ok(())
    }

    fn sweep(&mut self) -> BxResult<usize> {
        Ok(self.do_sweep())
    }

    fn stats(&self) -> GcStats {
        self.stats.clone()
    }

    fn needs_gc(&self) -> bool {
        self.heap.len() > self.heap.capacity() * 80 / 100
    }

    fn collect(&mut self) -> BxResult<usize> {
        let start_us = self.get_time_us();

        // Mark phase
        let roots: Vec<usize> = self.objects.clone();
        self.trace_root(&roots);
        self.trace();

        // Sweep phase
        let freed = self.do_sweep();

        let end_us = self.get_time_us();
        self.stats.collections += 1;
        self.stats.pause_time_us = end_us - start_us;

        Ok(freed)
    }
}

impl MarkSweepGc {
    fn get_time_us(&self) -> u64 {
        crate::bmo_core::lang::bmo::runtime::time::now_ns() / 1000
    }
}
