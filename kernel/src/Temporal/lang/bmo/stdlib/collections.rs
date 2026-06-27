//! BMO std::collections — Colecciones básicas.

#![allow(dead_code)]

use alloc::vec::Vec;

pub struct BmoVec<T: Copy> {
    inner: Vec<T>,
}

impl<T: Copy> BmoVec<T> {
    pub fn new() -> Self { BmoVec { inner: Vec::new() } }

    pub fn push(&mut self, val: T) { self.inner.push(val); }

    pub fn pop(&mut self) -> Option<T> { self.inner.pop() }

    pub fn len(&self) -> usize { self.inner.len() }

    pub fn is_empty(&self) -> bool { self.inner.is_empty() }

    pub fn get(&self, idx: usize) -> Option<T> { self.inner.get(idx).copied() }

    pub fn as_slice(&self) -> &[T] { &self.inner }
}

pub struct BmoMap {
    inner: Vec<(u64, u64)>,
}

impl BmoMap {
    pub fn new() -> Self { BmoMap { inner: Vec::new() } }

    pub fn insert(&mut self, key: u64, value: u64) {
        if let Some(entry) = self.inner.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.inner.push((key, value));
        }
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    pub fn remove(&mut self, key: u64) -> Option<u64> {
        if let Some(pos) = self.inner.iter().position(|(k, _)| *k == key) {
            Some(self.inner.swap_remove(pos).1)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize { self.inner.len() }

    pub fn contains_key(&self, key: u64) -> bool {
        self.inner.iter().any(|(k, _)| *k == key)
    }
}
