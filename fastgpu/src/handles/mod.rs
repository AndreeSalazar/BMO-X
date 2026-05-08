

pub const MAX_HANDLES: usize = 1024;

#[derive(Copy, Clone)]
pub struct HandleEntry<T> {
    pub item: Option<T>,
    pub generation: u32,
    pub ref_count: u32,
}

pub struct HandleTable<T> {
    entries: [HandleEntry<T>; MAX_HANDLES],
}

impl<T: Copy> HandleTable<T> {
    pub const fn new() -> Self {
        Self {
            entries: [HandleEntry { item: None, generation: 0, ref_count: 0 }; MAX_HANDLES],
        }
    }

    pub fn allocate(&mut self, item: T) -> Option<u32> {
        for (i, entry) in self.entries.iter_mut().enumerate() {
            if entry.item.is_none() {
                entry.item = Some(item);
                entry.generation += 1;
                // handle = index (lower 16) | generation (upper 16)
                let handle = (i as u32) | (entry.generation << 16);
                return Some(handle);
            }
        }
        None
    }

    pub fn get(&mut self, handle: u32) -> Option<&mut T> {
        let index = (handle & 0xFFFF) as usize;
        let gen = handle >> 16;
        if index < MAX_HANDLES {
            let entry = &mut self.entries[index];
            if entry.generation == gen && entry.item.is_some() {
                return entry.item.as_mut();
            }
        }
        None
    }

    pub fn free(&mut self, handle: u32) -> bool {
        let index = (handle & 0xFFFF) as usize;
        let gen = handle >> 16;
        if index < MAX_HANDLES {
            let entry = &mut self.entries[index];
            if entry.generation == gen {
                entry.item = None;
                return true;
            }
        }
        false
    }
}
