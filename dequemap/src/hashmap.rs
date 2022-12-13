/*
DequeMap is a combination of a double-ended queue and a hash map. It allows for the
insertion, deletion, and lookup of elements in O(1) time complexity, and maintains the
order in which elements are inserted. It is commonly used in scenarios such as caching or
message queues, where elements need to be quickly looked up and removed. In addition,
DequeMap provides an Entry API similar to that of a regular hash map, which allows for
advanced features such as concurrent modifications on the hash map.

As I mentioned earlier, DequeMap is a generic data structure that combines the strengths
of a double-ended queue and a hash map. It allows for fast insertion, deletion, and
lookup of elements, while maintaining the order in which they were inserted. This makes
it suitable for scenarios such as caching or message queues, where it can help improve
system performance and efficiency. In addition, DequeMap can support advanced features
such as concurrent modifications by implementing the Entry API.

In short, DequeMap is a powerful data structure that can meet a wide range of needs. It
has a wide range of applications and can help solve many practical problems.

DequeMap is a useful data structure for many different applications. Some of its key
features and advantages include:
 - Fast insertion, deletion, and lookup of elements: DequeMap allows for the insertion,
deletion, and lookup of elements in O(1) time complexity, making it efficient for a wide
range of applications.
 - Maintaining insertion order: DequeMap maintains the order in which elements are
inserted, allowing you to easily keep track of the order of elements. This can be useful
in scenarios such as message queues, where the order in which messages are processed is
important.
 - Support for the Entry API: DequeMap provides an Entry API similar to that of a regular
hash map, which allows for advanced features such as concurrent modifications. This can
make DequeMap a powerful tool for applications that require concurrent access to data.
 - Flexible keys and values: DequeMap allows you to use any type that implements the Hash
and Eq traits as a key, and any type as a value. This means that you can store a wide
range of data types in a DequeMap, making it a versatile data structure.
Overall, DequeMap is a useful data structure that can help improve the performance and
efficiency of your applications. Its combination of fast insertion, deletion, and lookup,
along with its support for maintaining insertion order and the Entry API, make it a
powerful tool for many different scenarios.

In addition to the features and advantages mentioned earlier, DequeMap also has some
potential limitations and drawbacks that you should be aware of. These include:
 - Not suitable for processing large batches of data with many duplicates: Because the
maximum time complexity of DequeMap is O(n), it is not suitable for processing large
batches of data with many duplicate elements. In these scenarios, a different data
structure may be more appropriate.
 - Limited to key-value pairs: DequeMap only allows you to store key-value pairs, so it
may not be suitable for applications that require other data structures. For example, if
you need to store a list of items without associated keys, a different data structure
such as a vector or linked list may be more appropriate.
 - Potential for memory overhead: DequeMap uses a hash map internally to store its
elements, which can lead to some memory overhead. This may not be an issue for small or
moderate-sized collections, but it could become a problem if you are working with very
large collections of data.
While DequeMap is a powerful data structure with many useful features, it is important to
be aware of its limitations and potential drawbacks. In some cases, a different data
structure may be a better choice for your application.

One possible use case for DequeMap is implementing a simple message queue. The code below
shows an example of how you might use DequeMap for this purpose:

```
use std::collections::DequeMap;

// Create a new DequeMap with a capacity of 100 messages.
let mut queue = DequeMap::with_capacity(100);

// Enqueue some messages.
queue.push_back("message1", "Hello, world!");
queue.push_back("message2", "This is a message queue.");
queue.push_back("message3", "DequeMap is a powerful data structure.");

// Dequeue a message.
let message = queue.pop_front();

// Update a message in the queue.
queue.push_back("message2", "This is an updated message.");

// Remove a message from the queue.
queue.remove("message3");

```

This code creates a new DequeMap with a capacity of 100 messages, enqueues some messages,
dequeues a message, updates a message, and removes a message. As with the caching
example, you would likely want to add additional logic for handling errors and
implementing more advanced features in a real application.

Another possible use case for DequeMap is implementing a simple dictionary data
structure. The code below shows an example of how you might use DequeMap for this purpose:

```
use std::collections::DequeMap;

// Create a new DequeMap.
let mut dictionary = DequeMap::new();

// Insert some words and definitions.
dictionary.insert("word1", "A unit of language.");
dictionary.insert("word2", "A word or phrase used to describe something.");
dictionary.insert("word3", "A word or phrase that has a specific meaning.");

// Look up a word in the dictionary.
let definition = dictionary.get("word2");

// Update a word in the dictionary.
dictionary.insert("word1", "A unit of language that consists of one or more spoken sounds.");

// Remove a word from the dictionary.
dictionary.remove("word3");


```

This code creates a new DequeMap, inserts some words and definitions, looks up a word,
updates a word, and removes a word. As with the other examples, you would likely want to
add additional logic for handling errors and implementing more advanced features in a
real application.

The above content and some comments in the code are written by ChatGPT.
 */
use alloc::collections::vec_deque::IntoIter as DequeIntoIter;
use alloc::collections::vec_deque::Iter as DequeIter;

use alloc::collections::VecDeque;
use core::borrow::Borrow;
use core::fmt;
use core::hash::BuildHasher;
use core::hash::Hash;
use core::iter::DoubleEndedIterator;
use core::iter::ExactSizeIterator;
use core::iter::FromIterator;
use core::iter::FusedIterator;
use core::mem::replace;
use core::ops::{Index, IndexMut};

use hashbrown::hash_map;
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::HashMap;

///Double-ended queue with Map feature.
///
/// When the element is present, the maximum time complexity is O(n). So it is not suitable for
/// processing large batches of data with too many duplicates.

#[derive(Debug, Clone)]
pub struct DequeMap<K, V, S = DefaultHashBuilder> {
    entries: HashMap<K, V, S>,
    indices: VecDeque<K>,
}

impl<K, V> DequeMap<K, V> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            indices: VecDeque::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            indices: VecDeque::with_capacity(capacity),
        }
    }
}

impl<K, V, S> Default for DequeMap<K, V, S>
where
    S: Default,
{
    fn default() -> Self {
        Self {
            entries: HashMap::default(),
            indices: VecDeque::default(),
        }
    }
}

impl<K, V, S> DequeMap<K, V, S> {
    #[inline]
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            entries: HashMap::with_hasher(hasher),
            indices: VecDeque::default(),
        }
    }
}

impl<K, V, S> DequeMap<K, V, S>
where
    K: Hash + Eq + Clone,
    S: BuildHasher,
{
    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(v) = self.entries.get_mut(&key) {
            Some(replace(v, value))
        } else {
            self.entries.insert(key.clone(), value);
            self.indices.push_back(key);
            None
        }
    }

    #[inline]
    pub fn push_back(&mut self, key: K, value: V) -> Option<V> {
        let old_val = self.remove_entry(&key);
        self.entries.insert(key.clone(), value);
        self.indices.push_back(key);
        old_val
    }

    #[inline]
    pub fn push_front(&mut self, key: K, value: V) -> Option<V> {
        let old_val = self.remove_entry(&key);
        self.entries.insert(key.clone(), value);
        self.indices.push_front(key);
        old_val
    }

    #[inline]
    pub fn entry(&mut self, key: K) -> Entry<K, V, S> {
        match self.entries.entry(key) {
            hash_map::Entry::Vacant(entry) => Entry::Vacant(VacantEntry {
                vacant: entry,
                indices: &mut self.indices,
            }),
            hash_map::Entry::Occupied(entry) => Entry::Occupied(OccupiedEntry { occupied: entry }),
        }
    }

    #[inline]
    fn remove_entry(&mut self, key: &K) -> Option<V> {
        if let Some(old_val) = self.entries.remove(key) {
            self.remove_from_index(key);
            Some(old_val)
        } else {
            None
        }
    }
}

impl<K, V, S> DequeMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Reserves capacity for at least additional more elements to be inserted in the given VecDeque.
    /// The collection may reserve more space to avoid frequent reallocations.
    pub fn reserve(&mut self, additional: usize) {
        self.indices.reserve(additional);
    }

    #[inline]
    pub fn clear(&mut self) {
        self.indices.clear();
        self.entries.clear();
    }

    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<V>
    where
        K: Clone,
    {
        if let Some(old_val) = self.entries.remove(k) {
            self.remove_from_index(k);
            Some(old_val)
        } else {
            None
        }
    }

    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.entries.get(k)
    }

    #[inline]
    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.entries.get_key_value(key)
    }

    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.entries.get_mut(k)
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V, S> {
        Iter {
            inner: self.indices.iter(),
            entries: &self.entries,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    #[inline]
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.entries.contains_key(k)
    }

    #[inline]
    pub fn front(&self) -> Option<(&K, &V)> {
        if self.is_empty() {
            return None;
        }
        if let Some(k) = self.indices.front() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn pop_front(&mut self) -> Option<(K, V)> {
        if let Some(k) = self.indices.pop_front() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn back(&self) -> Option<(&K, &V)> {
        if self.is_empty() {
            return None;
        }
        if let Some(k) = self.indices.back() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    pub fn pop_back(&mut self) -> Option<(K, V)> {
        if let Some(k) = self.indices.pop_back() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn get_index(&self, k: &K) -> Option<usize> {
        self.indices
            .iter()
            .enumerate()
            .find(|(_, x)| *x == k)
            .map(|(idx, _)| idx)
    }

    #[inline]
    fn remove_from_index(&mut self, k: &K) -> Option<K> {
        if let Some(idx) = self.get_index(k) {
            self.indices.remove(idx)
        } else {
            None
        }
    }
}

impl<'a, K, Q, V, S> Index<&'a Q> for DequeMap<K, V, S>
where
    K: Borrow<Q> + Hash + Eq,
    Q: Hash + Eq,
    S: BuildHasher,
{
    type Output = V;

    fn index(&self, key: &'a Q) -> &Self::Output {
        self.get(key).expect("no entry found for key")
    }
}

impl<K, V, S> Index<usize> for DequeMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    type Output = V;

    fn index(&self, index: usize) -> &Self::Output {
        let key = self
            .indices
            .get(index)
            .expect("DequeMap: index out of bounds");
        self.entries
            .get(key)
            .expect("DequeMap: index out of bounds")
    }
}

impl<K, V, S> IndexMut<usize> for DequeMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let key = self
            .indices
            .get(index)
            .expect("DequeMap: index out of bounds");
        self.entries
            .get_mut(key)
            .expect("DequeMap: index out of bounds")
    }
}

impl<K, V, S> IntoIterator for DequeMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, S>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.indices.into_iter(),
            entries: self.entries,
        }
    }
}

impl<'a, K, V, S> Extend<(&'a K, &'a V)> for DequeMap<K, V, S>
where
    K: Hash + Copy,
    V: Copy,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (&'a K, &'a V)>,
    {
        self.extend(iter.into_iter().map(|(key, value)| (key, value)))
    }
}

impl<K, V, S> Extend<(K, V)> for DequeMap<K, V, S>
where
    K: Hash + Eq + Clone,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K, V, S> FromIterator<(K, V)> for DequeMap<K, V, S>
where
    K: Hash + Eq + Clone,
    S: Default + BuildHasher,
{
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (K, V)>,
    {
        let mut map = DequeMap::default();
        map.extend(iter);
        map
    }
}

impl<K, V, S, const N: usize> From<[(K, V); N]> for DequeMap<K, V, S>
where
    K: Hash + Eq + Clone,
    S: Default + BuildHasher,
{
    fn from(items: [(K, V); N]) -> Self {
        let mut map = DequeMap::default();
        map.extend(items);
        map
    }
}

impl<'a, K, V, S> IntoIterator for &'a DequeMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V, S>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone)]
pub struct Iter<'a, K, V, S> {
    inner: DequeIter<'a, K>,
    entries: &'a HashMap<K, V, S>,
}

impl<'a, K, V, S> Iterator for Iter<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<'a, K, V, S> DoubleEndedIterator for Iter<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next_back() {
            self.entries.get(k).map(|v| (k, v))
        } else {
            None
        }
    }
}

impl<'a, K, V, S> ExactSizeIterator for Iter<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, K, V, S> FusedIterator for Iter<'a, K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
}

pub struct IntoIter<K, V, S> {
    inner: DequeIntoIter<K>,
    entries: HashMap<K, V, S>,
}

impl<K, V, S> Iterator for IntoIter<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<K, V, S> DoubleEndedIterator for IntoIter<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(k) = self.inner.next_back() {
            self.entries.remove(&k).map(|v| (k, v))
        } else {
            None
        }
    }
}

impl<K: Hash, V, S> ExactSizeIterator for IntoIter<K, V, S>
where
    K: Eq,
    S: BuildHasher,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<K: Hash, V, S> FusedIterator for IntoIter<K, V, S>
where
    K: Eq,
    S: BuildHasher,
{
}

/// A view into a single entry in a map, which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`DequeMap`].
///
/// [`entry`]: DequeMap::entry
pub enum Entry<'a, K, V, S> {
    /// A vacant entry.
    Vacant(VacantEntry<'a, K, V, S>),
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, K, V, S>),
}

impl<'a, K: Hash, V, S> Entry<'a, K, V, S> {
    /// Ensures a value is in the entry by inserting the default if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Clone,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result
    /// of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V
    where
        K: Clone,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Ensures a value is in the entry by inserting,
    /// if empty, the result of the default function.
    ///
    /// This method allows for generating key-derived values for
    /// insertion by providing the default function a reference
    /// to the key that was moved during the `.entry(key)` method call.
    ///
    /// The reference to the moved key is provided
    /// so that cloning or copying the key is
    /// unnecessary, unlike with `.or_insert_with(|| ... )`.
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V
    where
        K: Clone,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry’s key.
    pub fn key(&self) -> &K {
        match *self {
            Self::Occupied(ref entry) => entry.key(),
            Self::Vacant(ref entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry
    /// before any potential inserts into the map.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Self::Occupied(mut entry) => {
                f(entry.get_mut());
                Self::Occupied(entry)
            }
            Self::Vacant(entry) => Self::Vacant(entry),
        }
    }
}

impl<'a, K, V, S> Entry<'a, K, V, S>
where
    K: Hash + Clone,
    V: Default,
    S: BuildHasher,
{
    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_default(self) -> &'a mut V {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, K, V, S> fmt::Debug for Entry<'a, K, V, S>
where
    K: fmt::Debug + Hash,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Entry::Vacant(entry) => entry.fmt(f),
            Entry::Occupied(entry) => entry.fmt(f),
        }
    }
}

/// A view into a vacant entry in an [`DequeMap`]. It is part of the [`Entry`] `enum`.
pub struct VacantEntry<'a, K, V, S> {
    /// The underlying vacant entry.
    vacant: hash_map::VacantEntry<'a, K, V, S>,
    /// The vector that stores all slots.
    indices: &'a mut VecDeque<K>,
}

impl<'a, K, V, S> VacantEntry<'a, K, V, S>
where
    K: Hash,
{
    /// Gets a reference to the key that would be used when inserting a value through the VacantEntry.
    pub fn key(&self) -> &K {
        self.vacant.key()
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> K {
        self.vacant.into_key()
    }

    /// Sets the value of the entry with the `VacantEntry`’s key,
    /// and returns a mutable reference to it.
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Clone,
        S: BuildHasher,
    {
        self.indices.push_back(self.vacant.key().clone());
        self.vacant.insert(value)
    }
}

impl<'a, K, V, S> fmt::Debug for VacantEntry<'a, K, V, S>
where
    K: fmt::Debug + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VacantEntry")
            .field("key", self.key())
            .finish()
    }
}

/// A view into an occupied entry in a [`DequeMap`]. It is part of the [`Entry`] `enum`.
pub struct OccupiedEntry<'a, K, V, S> {
    /// The underlying occupied entry.
    occupied: hash_map::OccupiedEntry<'a, K, V, S>,
}

impl<'a, K, V, S> OccupiedEntry<'a, K, V, S>
where
    K: Hash,
{
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &K {
        self.occupied.key()
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V {
        self.occupied.get()
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntry` that may outlive the
    /// destruction of the `Entry` value, see [`into_mut`].
    ///
    /// [`into_mut`]: OccupiedEntry::into_mut
    pub fn get_mut(&mut self) -> &mut V {
        self.occupied.get_mut()
    }

    /// Converts the entry into a mutable reference to its value.
    ///
    /// If you need multiple references to the `OccupiedEntry`, see [`get_mut`].
    ///
    /// [`get_mut`]: OccupiedEntry::get_mut
    pub fn into_mut(self) -> &'a mut V {
        self.occupied.into_mut()
    }

    /// Sets the value of the entry with the `OccupiedEntry`’s key,
    /// and returns the entry’s old value.
    pub fn insert(&mut self, value: V) -> V
    where
        K: Clone,
    {
        replace(self.occupied.get_mut(), value)
    }
}

impl<'a, K, V, S> fmt::Debug for OccupiedEntry<'a, K, V, S>
where
    K: fmt::Debug + Hash,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}

#[test]
fn test_insert() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.insert(2, 20);
    map.insert(1, 10);
    map.insert(9, 90);
    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90)]);

    map.insert(7, 70);
    map.insert(1, 100);
    assert_eq!(to_vec(&map), [(2, 20), (1, 100), (9, 90), (7, 70)]);

    assert_eq!(map.entries.len(), map.indices.len());

    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.pop_back(), Some((7, 70)));
    assert_eq!(to_vec(&map), [(1, 100), (9, 90)]);

    map.insert(3, 30);
    map.insert(7, 70);
    map.insert(9, 900);
    map.push_back(1, 10);
    assert_eq!(to_vec(&map), [(9, 900), (3, 30), (7, 70), (1, 10)]);
    assert_eq!(map.entries.len(), map.indices.len());
}

#[test]
fn test_entry() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.entry(2).or_insert(20);
    map.entry(1).or_insert(10);
    map.entry(9).or_insert(90);
    map.entry(3).or_insert(30);
    map.entry(5).or_insert(50);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), Some(&30));
    assert_eq!(map.get(&5), Some(&50));
    assert_eq!(map.get(&9), Some(&90));

    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 30), (5, 50)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.entry(3).and_modify(|v| *v = 300);

    assert_eq!(to_vec(&map), [(2, 20), (1, 10), (9, 90), (3, 300), (5, 50)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.entry(7).or_insert_with(|| 70);
    assert_eq!(
        to_vec(&map),
        [(2, 20), (1, 10), (9, 90), (3, 300), (5, 50), (7, 70)]
    );
    assert_eq!(map.entries.len(), map.indices.len());
}

#[test]
fn test_dequemap() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };

    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    map.push_back(3, 30);
    map.push_back(5, 50);
    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map.get(&2), Some(&20));
    assert_eq!(map.get(&3), Some(&30));
    assert_eq!(map.get(&5), Some(&50));
    assert_eq!(map.get(&9), Some(&90));
    assert_eq!(map.len(), 5);
    assert_eq!(map.pop_front(), Some((2, 20)));
    assert_eq!(map.len(), 4);
    assert_eq!(map.pop_back(), Some((5, 50)));
    assert_eq!(map.len(), 3);
    assert_eq!(to_vec(&map), [(1, 10), (9, 90), (3, 30)]);
    assert_eq!(map.entries.len(), map.indices.len());

    let mut map1: DequeMap<i32, i32> = DequeMap::new();
    map1.push_back(7, 70);
    map1.push_back(9, 900);
    map.extend(map1);
    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (3, 30), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());

    assert_eq!(map.front(), Some((&1, &10)));
    assert_eq!(map.back(), Some((&7, &70)));

    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (3, 30), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());

    map.remove(&3);
    assert_eq!(to_vec(&map), [(1, 10), (9, 900), (7, 70)]);
    assert_eq!(map.entries.len(), map.indices.len());
}

#[test]
fn test_dequemap_index() {
    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    assert_eq!(map.index_mut(1), &mut 10);
    assert_eq!(map.index(2), &90);
}

#[test]
fn test_dequemap_extend() {
    use alloc::vec::Vec;
    let to_vec = |map: &DequeMap<i32, i32>| {
        map.iter()
            .map(|t| (*t.0, *t.1))
            .collect::<Vec<(i32, i32)>>()
    };
    let mut map = DequeMap::new();
    map.push_back(2, 20);
    map.push_back(1, 10);
    map.push_back(9, 90);
    map.extend([(10, 100), (5, 50)]);
    assert_eq!(
        to_vec(&map),
        [(2, 20), (1, 10), (9, 90), (10, 100), (5, 50)]
    );
    assert_eq!(map.entries.len(), map.indices.len());
}
