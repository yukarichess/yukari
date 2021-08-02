use std::{boxed::Box, mem::size_of};

// Entry in the table, hash and value
type Entry<T> = (u64, T);

#[derive(Clone, Debug)]
pub struct TranspositionTable<T>(Box<[Entry<T>]>);

impl<T: Default + Clone> TranspositionTable<T> {

    /// Creates a new transposition table of the given size in bytes
    /// size will be rounded down to closest power of two
    pub fn new(size: usize) -> Self {
        let entry_size = size_of::<Entry<T>>();
        // We can only store an integral number of entries
        // and we want to round down to a power of two to make key wrapping fast
        let count = (size / entry_size).next_power_of_two() >> 1;
        // Then we have to compute the number of them we can fit i
        Self({
            let empty = (0, T::default());
            vec![empty; count].into_boxed_slice()
        })
    }
}

impl<T> TranspositionTable<T> {
    pub fn get(&self, key: u64) -> Option<&T> {
        let idx = (key & (self.0.len() - 1) as u64) as usize;
        let entry = &self.0[idx];
        if key == entry.0 {
            return Some(&entry.1);
        }
        None
    }

    pub fn set(&mut self, key: u64, entry: T) {
        let idx = (key & (self.0.len() - 1) as u64) as usize;
        self.0[idx] = (key, entry);
    }

    pub fn clear(&mut self) {
        for i in 0..self.0.len() {
            self.0[i].0 = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tt::TranspositionTable;

    #[test]
    fn basic() {
        let mut tt = TranspositionTable::new(2);
        tt.set(0, "hi");
        tt.set(3, "no");
        assert_eq!(tt.0[0], (0, "hi"));
        assert_eq!(tt.0[1], (3, "no"));
        tt.set(4, "bye");
        assert_eq!(tt.0[0], (4, "bye"));
        assert_eq!(tt.get(0), None);
    }
}