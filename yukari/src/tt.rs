use std::{boxed::Box, cell::Cell, fmt::{Display, Debug}, mem::size_of};

// Entry in the table, hash and value
type Entry<T> = (u64, T);

/// Implements a "histogram" for recording hits and misses in the table
/// has do-nothing default implementations
trait Histogram {
    fn miss(&self) {}
    fn hit(&self) {}
    fn misses(&self) -> u64 { 0 }
    fn hits(&self) -> u64 { 0 }
    fn total(&self) -> u64 {
        self.hits() + self.misses()
    }
}
/// NullHistogram does nothing and takes no space so it's intentionally supposed to optimize out
struct NullHistogram;
impl Histogram for NullHistogram {}

/// BasicHistogram implements the usual support
/// Needs to use cell because we don't want to make get on the hashtable mutable
#[derive(Clone)]
struct BasicHistogram {
    hits: Cell<u64>,
    misses: Cell<u64>
}

impl BasicHistogram {
    fn new() -> Self {
        Self {
            hits: Cell::new(0),
            misses: Cell::new(0)
        }
    }
}

impl Histogram for BasicHistogram {
    #[inline(always)]
    fn miss(&self) {
        self.misses.set(self.misses.get() + 1);
    }

    #[inline(always)]
    fn hit(&self) {
        self.hits.set(self.hits.get() + 1);
    }

    #[inline(always)]
    fn misses(&self) -> u64 {
        self.misses.get()
    }

    #[inline(always)]
    fn hits(&self) -> u64 {
        self.hits.get()
    }
}

impl Display for BasicHistogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Histogram")
            .field("hits", &self.hits.get())
            .field("misses", &self.misses.get())
            .field("total", &self.total())
            .finish()
    }
}

impl Debug for BasicHistogram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

#[derive(Clone)]
pub struct TranspositionTable<T> {
    table: Box<[Entry<T>]>,
    histogram: BasicHistogram
}

impl<T: Default + Clone> TranspositionTable<T> {
    /// Hack to create with a histogram until API and usefulness is determined
    pub fn with_histogram(size: usize) -> Self {
        let entry_size = size_of::<Entry<T>>();
        // We can only store an integral number of entries
        // and we want to round down to a power of two to make key wrapping fast
        let mut count = (size / entry_size).next_power_of_two();
        if count * entry_size > size {
            count >>= 1;
        }
        // Then we have to compute the number of them we can fit i
        Self {
            table: {
                let empty = (0, T::default());
                vec![empty; count].into_boxed_slice()
            },
            histogram: BasicHistogram::new()
        }
    }
    /// Creates a new transposition table of the given size in bytes
    /// size will be rounded down to closest power of two
    pub fn new(size: usize) -> Self {
        Self::with_histogram(size)
    }
}

impl<T> TranspositionTable<T> {
    pub fn get(&self, key: u64) -> Option<&T> {
        if self.table.len() > 0 {
            let idx = (key & (self.table.len() - 1) as u64) as usize;
            let entry = &self.table[idx];
            if key == entry.0 {
                self.histogram.hit();
                return Some(&entry.1);
            }
        }
        self.histogram.miss();
        None
    }

    pub fn set(&mut self, key: u64, entry: T) {
        if self.table.len() > 0 {
            let idx = (key & (self.table.len() - 1) as u64) as usize;
            self.table[idx] = (key, entry);
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.table.len() {
            self.table[i].0 = 0;
        }
    }
}

impl<T> Display for TranspositionTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Count the number of entries in the table that are not unused (hash of 0)
        let valid_count = self.table.iter()
                                    .map(|&(hash, ..)| hash)
                                    .filter(|&hash| hash != 0)
                                    .count();
        f.debug_struct("TranspositionTable")
            .field("table (valid entries)", &valid_count)
            .field("histogram", &self.histogram)
            .finish()
    }
}

#[cfg(test)]
mod tests {

    use crate::tt::{Entry, TranspositionTable};

    #[test]
    fn basic() {
        /* TODO: Fix broken test */
        let mut tt = TranspositionTable::new(2*std::mem::size_of::<Entry<&str>>());
        tt.set(0, "hi");
        tt.set(3, "no");
        //assert_eq!(tt.table[0], (0, "hi"));
        //assert_eq!(tt.table[1], (3, "no"));
        tt.set(4, "bye");
        //assert_eq!(tt.table[0], (4, "bye"));
        assert_eq!(tt.get(0), None);
        eprintln!("{}", &tt);
    }
}