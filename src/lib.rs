//! An efficient [Least Frequently Used Cache](https://en.wikipedia.org/wiki/Least_frequently_used) implementation.
//!
//! It supports insertions and retrievals, both of which are performed in constant time. In the event of tie between
//! two least frequently used entries, the least *recently* used entry is evicted.
//!
//!
//!
//! # Examples
//!
//! ```
//! extern crate lfu;
//! use lfu::LFUCache;
//!
//! # fn main() {
//! let mut lfu = LFUCache::with_capacity(2); //initialize an lfu with a maximum capacity of 2 entries
//! lfu.set(2, 2);
//! lfu.set(3, 3);
//! lfu.set(3, 30);
//! lfu.set(4,4); //We're at fully capacity. First purge (2,2) since it's the least-frequently-used entry, then insert the current entry

//! assert_eq!(lfu.get(&2), None);
//! assert_eq!(lfu.get(&3), Some(&30));
//!
//! # }
//! ```

use linked_hash_set::LinkedHashSet;
use std::collections::hash_map::{IntoIter, Iter};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Index;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct LFUCache<K: Hash + Eq + Clone, V> {
    values: HashMap<K, ValueCounter<V>>,
    frequency_bin: HashMap<usize, LinkedHashSet<K>>,
    capacity: usize,
    min_frequency: usize,
}

#[derive(Clone, Debug)]
struct ValueCounter<V> {
    value: V,
    count: usize,
}

impl<V> ValueCounter<V> {
    fn inc(&mut self) {
        self.count += 1;
    }
}

impl<K: Hash + Eq + Clone, V> LFUCache<K, V> {
    pub fn with_capacity(capacity: usize) -> LFUCache<K, V> {
        if capacity == 0 {
            panic!("Unable to create cache: capacity is {:?}", capacity);
        }
        LFUCache {
            values: HashMap::new(),
            frequency_bin: HashMap::new(),
            capacity,
            min_frequency: 0,
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn remove(&mut self, key: K) -> Option<V> {
        if let Some(value_counter) = self.values.get(&key) {
            let count = value_counter.count;
            self.frequency_bin.entry(count).or_default().remove(&key);
            self.values.remove(&key).map(|x| x.value)
        } else {
            None
        }
    }

    /// Returns the value associated with the given key (if it still exists)
    /// Method marked as mutable because it internally updates the frequency of the accessed key
    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.update_frequency_bin(key);
        self.values.get(&key).map(|x| &x.value)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.update_frequency_bin(key);
        self.values.get_mut(&key).map(|x| &mut x.value)
    }

    fn update_frequency_bin(&mut self, key: &K) {
        if let Some(value_counter) = self.values.get_mut(&key) {
            let bin = self.frequency_bin.get_mut(&value_counter.count).unwrap();
            bin.remove(&key);
            let count = value_counter.count;
            value_counter.inc();
            if count == self.min_frequency && bin.is_empty() {
                self.min_frequency += 1;
            }
            self.frequency_bin
                .entry(count + 1)
                .or_default()
                .insert(key.clone());
        }
    }

    fn evict(&mut self) {
        let least_frequently_used_keys = self.frequency_bin.get_mut(&self.min_frequency).unwrap();
        let least_recently_used = least_frequently_used_keys.pop_front().unwrap();
        self.values.remove(&least_recently_used);
    }

    pub fn peek_lfu_key(&mut self) -> Option<K> {
        let least_frequently_used_keys = self.frequency_bin.get_mut(&self.min_frequency).unwrap();
        least_frequently_used_keys.front().map(|x| x.clone())
    }

    pub fn iter(&self) -> LfuIterator<K, V> {
        LfuIterator {
            values: self.values.iter(),
        }
    }

    pub fn set(&mut self, key: K, value: V) {
        if let Some(value_counter) = self.values.get_mut(&key) {
            value_counter.value = value;
            self.update_frequency_bin(&key);
            return;
        }
        if self.len() >= self.capacity {
            self.evict();
        }
        self.values
            .insert(key.clone(), ValueCounter { value, count: 1 });
        self.min_frequency = 1;
        self.frequency_bin
            .entry(self.min_frequency)
            .or_default()
            .insert(key);
    }
}

pub struct LfuIterator<'a, K, V> {
    values: Iter<'a, K, ValueCounter<V>>,
}

pub struct LfuConsumer<K, V> {
    values: IntoIter<K, ValueCounter<V>>,
}

impl<K, V> Iterator for LfuConsumer<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|(k, v)| (k, v.value))
    }
}

impl<K: Eq + Hash + Clone, V> IntoIterator for LFUCache<K, V> {
    type Item = (K, V);
    type IntoIter = LfuConsumer<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        LfuConsumer {
            values: self.values.into_iter(),
        }
    }
}

impl<'a, K: Hash + Eq + Clone, V> Iterator for LfuIterator<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|(key, vc)| (key, &vc.value))
    }
}

impl<'a, K: Hash + Eq + Clone, V> IntoIterator for &'a LFUCache<K, V> {
    type Item = (&'a K, &'a V);

    type IntoIter = LfuIterator<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        return self.iter();
    }
}

impl<K: Hash + Eq + Clone, V> Index<K> for LFUCache<K, V> {
    type Output = V;
    fn index(&self, index: K) -> &Self::Output {
        return self.values.get(&Rc::new(index)).map(|x| &x.value).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut lfu = LFUCache::with_capacity(20);
        lfu.set(10, 10);
        lfu.set(20, 30);
        assert_eq!(lfu.get(&10).unwrap(), &10);
        assert_eq!(lfu.get(&30), None);
    }

    #[test]
    fn test_lru_eviction() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(2, 2);
        lfu.set(3, 3);
        assert_eq!(lfu.get(&1), None)
    }

    #[test]
    fn test_key_frequency_update() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(2, 2);
        lfu.set(1, 3);
        lfu.set(10, 10);
        assert_eq!(lfu.get(&2), None);
        assert_eq!(lfu[10], 10);
    }

    #[test]
    fn test_lfu_indexing() {
        let mut lfu: LFUCache<i32, i32> = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        assert_eq!(lfu[1], 1);
    }

    #[test]
    fn test_lfu_deletion() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(2, 2);
        lfu.remove(1);
        assert_eq!(lfu.get(&1), None);
        lfu.set(3, 3);
        lfu.set(4, 4);
        assert_eq!(lfu.get(&2), None);
        assert_eq!(lfu.get(&3), Some(&3));
    }

    #[test]
    fn test_duplicates() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(1, 2);
        lfu.set(1, 3);
        {
            lfu.set(5, 20);
        }

        assert_eq!(lfu[1], 3);
    }

    #[test]
    fn test_lfu_consumption() {
        let mut lfu = LFUCache::with_capacity(1);
        lfu.set(&1, 1);
        for (_, v) in lfu {
            assert_eq!(v, 1);
        }
    }

    #[test]
    fn test_lfu_iter() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(&1, 1);
        lfu.set(&2, 2);
        for (key, v) in lfu.iter() {
            match *key {
                1 => {
                    assert_eq!(v, &1);
                }
                2 => {
                    assert_eq!(v, &2);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn clone_test() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(2, 2);
        lfu.set(3, 3);

        let lfu2 = lfu.clone();
        for (key, vc) in lfu.values.iter() {
            let (_key2, vc2) = lfu2.values.get_key_value(key).unwrap();
            assert_eq!(vc.count, vc2.count);
            assert_eq!(vc.value, vc2.value);
        }
    }

    #[test]
    fn peek_test() {
        let mut lfu = LFUCache::with_capacity(2);
        lfu.set(1, 1);
        lfu.set(2, 2);

        let _ = lfu.get(&1);
        let peek = lfu.peek_lfu_key();
        assert_eq!(peek, Some(2));

        lfu.set(3, 3);
        assert_eq!(lfu.get(&2), None);
    }
}
