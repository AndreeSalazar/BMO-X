//! ÑEXO Runtime — Gestión de memoria.
//!
//! Pool allocator sobre el bump allocator del kernel.
//! El bump allocator (16MB) no libera, así que el pool
//! reutiliza bloques de tamaño fijo.

#![allow(dead_code)]

extern crate alloc;
use alloc::alloc::{alloc, Layout};

use super::error::{Error, Result};

/// Pool block size: 64 bytes.
const BLOCK_SIZE: usize = 64;
/// Maximum pool blocks.
const MAX_BLOCKS: usize = 4096;

/// Pool entry state.
#[derive(Debug, Clone, Copy)]
enum BlockState {
    Free,
    Used,
}

/// A fixed-size block pool for efficient small allocations.
pub struct PoolAllocator {
    base: *mut u8,
    states: [BlockState; MAX_BLOCKS],
    total_blocks: usize,
    used_blocks: usize,
}

impl PoolAllocator {
    /// Create a new pool allocator using kernel heap.
    pub fn new(num_blocks: usize) -> Result<Self> {
        let n = if num_blocks > MAX_BLOCKS { MAX_BLOCKS } else { num_blocks };
        let size = n * BLOCK_SIZE;
        let layout = Layout::from_size_align(size, 16).map_err(|_| Error::OutOfMemory)?;
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(Error::OutOfMemory);
        }
        Ok(Self {
            base: ptr,
            states: [BlockState::Free; MAX_BLOCKS],
            total_blocks: n,
            used_blocks: 0,
        })
    }

    /// Allocate a block from the pool.
    pub fn alloc(&mut self) -> Result<*mut u8> {
        for i in 0..self.total_blocks {
            if let BlockState::Free = self.states[i] {
                self.states[i] = BlockState::Used;
                self.used_blocks += 1;
                let ptr = unsafe { self.base.add(i * BLOCK_SIZE) };
                return Ok(ptr);
            }
        }
        Err(Error::OutOfMemory)
    }

    /// Free a block back to the pool.
    pub fn free(&mut self, ptr: *mut u8) {
        let offset = (ptr as usize).wrapping_sub(self.base as usize);
        if offset < self.total_blocks * BLOCK_SIZE && offset % BLOCK_SIZE == 0 {
            let idx = offset / BLOCK_SIZE;
            if let BlockState::Used = self.states[idx] {
                self.states[idx] = BlockState::Free;
                self.used_blocks -= 1;
            }
        }
    }

    /// Number of free blocks.
    pub fn free_count(&self) -> usize {
        self.total_blocks - self.used_blocks
    }

    /// Number of used blocks.
    pub fn used_count(&self) -> usize {
        self.used_blocks
    }

    /// Total blocks.
    pub fn total_blocks(&self) -> usize {
        self.total_blocks
    }
}

/// Simple arena allocator — bump-allocate without individual free.
pub struct Arena {
    base: *mut u8,
    offset: usize,
    size: usize,
}

impl Arena {
    /// Create a new arena with given size on kernel heap.
    pub fn new(size: usize) -> Result<Self> {
        let layout = Layout::from_size_align(size, 16).map_err(|_| Error::OutOfMemory)?;
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(Error::OutOfMemory);
        }
        Ok(Self { base: ptr, offset: 0, size })
    }

    /// Allocate `size` bytes with `align` from the arena.
    pub fn alloc(&mut self, size: usize, align: usize) -> Result<*mut u8> {
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        if aligned_offset + size > self.size {
            return Err(Error::OutOfMemory);
        }
        let ptr = unsafe { self.base.add(aligned_offset) };
        self.offset = aligned_offset + size;
        Ok(ptr)
    }

    /// Reset the arena (all memory becomes free again).
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Bytes used.
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Bytes remaining.
    pub fn remaining(&self) -> usize {
        self.size - self.offset
    }
}

/// Initialize the memory subsystem.
pub fn init() {
    crate::diag::info("nexo_mem", "Memory subsystem initialized");
}
