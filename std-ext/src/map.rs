use std::borrow::Borrow;
use std::cmp::Ord;
use std::collections::btree_map;
use std::collections::hash_map;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::time::{Duration, Instant};

pub trait CacheMapExt<K, V> {
    fn get_with_timeout<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Hash + Eq + Ord;

    fn get_with_timeout_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Ord,
        Q: Hash + Eq + Ord;

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V>;

    fn remove_expired_values(&mut self);
}

pub trait EntryExt<'a, K, V> {
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V;
    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V;
    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V;
    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V);
}

impl<K, V, S> CacheMapExt<K, V> for HashMap<K, TimedValue<V>, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get_with_timeout<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V> EntryExt<'a, K, V> for hash_map::Entry<'a, K, TimedValue<V>>
where
    K: Eq + Hash,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            hash_map::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            hash_map::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

impl<K: Ord, V> CacheMapExt<K, V> for BTreeMap<K, TimedValue<V>> {
    fn get_with_timeout<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.get(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value())
            }
        })
    }

    fn get_with_timeout_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.get_mut(k).and_then(|tv| {
            if tv.is_expired() {
                None
            } else {
                Some(tv.value_mut())
            }
        })
    }

    fn insert_with_timeout(&mut self, k: K, v: V, timeout: Option<Duration>) -> Option<V> {
        self.insert(k, TimedValue::new(v, timeout))
            .map(|tv| tv.into_value())
    }

    fn remove_expired_values(&mut self) {
        self.retain(|_, tv| !tv.is_expired());
    }
}

impl<'a, K, V> EntryExt<'a, K, V> for btree_map::Entry<'a, K, TimedValue<V>>
where
    K: Ord,
{
    fn or_insert_with_timeout(self, default: V, timeout: Option<Duration>) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default, timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => {
                entry.insert(TimedValue::new(default, timeout)).value_mut()
            }
        }
    }

    fn or_insert_with_timeout_f<F: FnOnce() -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let v = entry.into_mut();
                if v.is_expired() {
                    *v = TimedValue::new(default(), timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => entry
                .insert(TimedValue::new(default(), timeout))
                .value_mut(),
        }
    }

    fn or_insert_with_timeout_key_f<F: FnOnce(&K) -> V>(
        self,
        default: F,
        timeout: Option<Duration>,
    ) -> &'a mut V {
        match self {
            btree_map::Entry::Occupied(entry) => {
                let value = if entry.get().is_expired() {
                    Some(default(entry.key()))
                } else {
                    None
                };
                let v = entry.into_mut();
                if let Some(value) = value {
                    *v = TimedValue::new(value, timeout);
                }
                v.value_mut()
            }
            btree_map::Entry::Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(TimedValue::new(value, timeout)).value_mut()
            }
        }
    }

    fn and_modify_with_timeout<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        self.and_modify(|v| f(v.value_mut()))
    }
}

#[derive(Clone, Debug)]
pub struct TimedValue<V>(V, Option<Instant>);

impl<V> TimedValue<V> {
    pub fn new(value: V, timeout_duration: Option<Duration>) -> Self {
        TimedValue(value, timeout_duration.map(|t| Instant::now() + t))
    }

    pub fn value(&self) -> &V {
        &self.0
    }

    pub fn value_mut(&mut self) -> &mut V {
        &mut self.0
    }

    pub fn into_value(self) -> V {
        self.0
    }

    pub fn is_expired(&self) -> bool {
        self.1.map(|e| Instant::now() >= e).unwrap_or(false)
    }
}

impl<V> PartialEq for TimedValue<V>
where
    V: PartialEq,
{
    fn eq(&self, other: &TimedValue<V>) -> bool {
        self.value() == other.value()
    }
}

//impl<V> Eq for TimedValue<V> where V: Eq {}

#[test]
fn test_cache_map_ext() {
    use std::collections::hash_map::RandomState;

    let mut m: HashMap<_, _, RandomState> = HashMap::default();
    let old1 = m.insert("k1", TimedValue::new(1, None));
    let old2 = m.insert("k2", TimedValue::new(2, Some(Duration::from_millis(50))));
    let old3 = m.insert("k3", TimedValue::new(3, Some(Duration::from_millis(80))));
    let old4 = m.insert("k4", TimedValue::new(4, Some(Duration::from_millis(130))));
    let old44 = m.insert("k4", TimedValue::new(44, None));

    let old6 = m.insert_with_timeout("k6", 6, Some(Duration::from_secs(150)));
    let old7 = m.insert_with_timeout("k7", 7, None);

    let old66 = m.insert_with_timeout("k6", 66, Some(Duration::from_secs(60)));

    assert_eq!(old1, None);
    assert_eq!(old2, None);
    assert_eq!(old3, None);
    assert_eq!(old4, None);
    assert_eq!(old6, None);
    assert_eq!(old7, None);

    println!("old44: {:?}", old44);
    assert_eq!(old44, Some(TimedValue::new(4, None)));

    assert_eq!(old66, Some(6));
    println!("old66: {:?}", old66);

    let v6 = m.get("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&TimedValue::new(66, None)));

    m.get_with_timeout_mut("k6").map(|v| *v = 666);
    let v6 = m.get_with_timeout("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&666));

    for i in 0..20 {
        m.remove_expired_values();
        println!(
            "{} map len: {},  map k1: {:?}, k2: {:?}",
            i,
            m.len(),
            m.get("k1"),
            m.get_with_timeout("k2")
        );
        //        println!("{} map len: {},  map: {:?}", i, m.len(), m);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

#[test]
fn test_btree_map_ext() {
    let mut m: BTreeMap<_, _> = BTreeMap::default();
    let old1 = m.insert_with_timeout("k1", 1, None);
    let old2 = m.insert_with_timeout("k2", 2, Some(Duration::from_millis(50)));
    let old3 = m.insert_with_timeout("k3", 3, Some(Duration::from_millis(80)));
    let old4 = m.insert_with_timeout("k4", 4, Some(Duration::from_millis(130)));
    let old44 = m.insert_with_timeout("k4", 44, None);

    let old6 = m.insert_with_timeout("k6", 6, Some(Duration::from_secs(150)));
    let old7 = m.insert_with_timeout("k7", 7, None);
    let old66 = m.insert_with_timeout("k6", 66, Some(Duration::from_secs(60)));

    assert_eq!(old1, None);
    assert_eq!(old2, None);
    assert_eq!(old3, None);
    assert_eq!(old4, None);
    assert_eq!(old6, None);
    assert_eq!(old7, None);

    println!("old44: {:?}", old44);
    assert_eq!(old44, Some(4));
    assert_eq!(m.get_with_timeout("k4"), Some(&44));

    assert_eq!(old66, Some(6));
    println!("old66: {:?}", old66);

    let v6 = m.get("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&TimedValue::new(66, None)));

    m.get_with_timeout_mut("k6").map(|v| *v = 666);
    let v6 = m.get_with_timeout("k6");
    println!("v6: {:?}", v6);
    assert_eq!(v6, Some(&666));

    m.entry("kk1").or_insert_with_timeout_f(|| 10, None);
    assert_eq!(m.get_with_timeout("kk1"), Some(&10));
    m.entry("kk1").and_modify_with_timeout(|v| {
        *v = 100;
    });
    assert_eq!(m.get_with_timeout("kk1"), Some(&100));
    println!("kk1: {:?}", m.get_with_timeout("kk1"));
}
