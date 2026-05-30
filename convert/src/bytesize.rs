use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::{Deref, DerefMut};

const BYTESIZE_K: usize = 1024;
const BYTESIZE_M: usize = BYTESIZE_K * BYTESIZE_K;
const BYTESIZE_G: usize = BYTESIZE_K * BYTESIZE_K * BYTESIZE_K;

#[derive(Clone)]
pub struct Bytesize(usize);

impl Bytesize {
    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0 as u32
    }

    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
    }

    #[inline]
    pub fn string(&self) -> String {
        let mut v = self.0;
        let mut res = String::new();

        let g = v / BYTESIZE_G;
        if g > 0 {
            res.push_str(&format!("{}G", g));
            v %= BYTESIZE_G;
        }

        let m = v / BYTESIZE_M;
        if m > 0 {
            res.push_str(&format!("{}M", m));
            v %= BYTESIZE_M;
        }

        let k = v / BYTESIZE_K;
        if k > 0 {
            res.push_str(&format!("{}K", k));
            v %= BYTESIZE_K;
        }

        if v > 0 {
            res.push_str(&format!("{}B", v));
        }

        res
    }
}

impl Deref for Bytesize {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Bytesize {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<usize> for Bytesize {
    fn from(v: usize) -> Self {
        Bytesize(v)
    }
}

impl From<&str> for Bytesize {
    fn from(v: &str) -> Self {
        Bytesize(to_bytesize(v))
    }
}

impl fmt::Debug for Bytesize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.string())?;
        Ok(())
    }
}

impl Serialize for Bytesize {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.string())
    }
}

impl<'de> Deserialize<'de> for Bytesize {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = to_bytesize(&String::deserialize(deserializer)?);
        Ok(Bytesize(v))
    }
}

#[inline]
fn to_bytesize(text: &str) -> usize {
    let text = text
        .to_uppercase()
        .replace("GB", "G")
        .replace("MB", "M")
        .replace("KB", "K");
    text.split_inclusive(|x| x == 'G' || x == 'M' || x == 'K' || x == 'B')
        .map(|x| {
            let mut chars = x.chars();
            let u = match chars.nth_back(0) {
                None => return 0,
                Some(u) => u,
            };
            let v = match chars.as_str().parse::<usize>() {
                Err(_e) => return 0,
                Ok(v) => v,
            };
            match u {
                'B' => v,
                'K' => v * BYTESIZE_K,
                'M' => v * BYTESIZE_M,
                'G' => v * BYTESIZE_G,
                _ => 0,
            }
        })
        .sum()
}

#[cfg(test)]
#[cfg(feature = "bytesize")]
mod tests {
    use super::*;

    #[test]
    fn test_from_usize() {
        let b = Bytesize::from(0usize);
        assert_eq!(b.as_usize(), 0);

        let b = Bytesize::from(1usize);
        assert_eq!(b.as_usize(), 1);

        let b = Bytesize::from(1024usize);
        assert_eq!(b.as_usize(), 1024);

        let b = Bytesize::from(1048576usize);
        assert_eq!(b.as_usize(), 1048576);
    }

    #[test]
    fn test_from_str() {
        let b = Bytesize::from("1K");
        assert_eq!(b.as_usize(), 1024);

        let b = Bytesize::from("1M");
        assert_eq!(b.as_usize(), 1024 * 1024);

        let b = Bytesize::from("1G");
        assert_eq!(b.as_usize(), 1024 * 1024 * 1024);

        let b = Bytesize::from("1K2M");
        assert_eq!(b.as_usize(), 1024 + 2 * 1024 * 1024);

        let b = Bytesize::from("1K2M3B");
        assert_eq!(b.as_usize(), 1024 + 2 * 1024 * 1024 + 3);
    }

    #[test]
    fn test_string_output() {
        assert_eq!(Bytesize::from(1024usize).string(), "1K");
        assert_eq!(Bytesize::from(1048576usize).string(), "1M");
        assert_eq!(Bytesize::from(1073741824usize).string(), "1G");
        assert_eq!(Bytesize::from(2048usize).string(), "2K");
        assert_eq!(Bytesize::from(1025usize).string(), "1K1B");
        assert_eq!(Bytesize::from(1usize).string(), "1B");
    }

    #[test]
    fn test_as_types() {
        let b = Bytesize::from(42usize);
        assert_eq!(b.as_u32(), 42u32);
        assert_eq!(b.as_u64(), 42u64);
        assert_eq!(b.as_usize(), 42usize);

        let b = Bytesize::from(usize::MAX);
        assert_eq!(b.as_u64(), usize::MAX as u64);
        assert_eq!(b.as_usize(), usize::MAX);
    }

    #[test]
    fn test_deref() {
        let b = Bytesize::from(42usize);
        assert_eq!(*b, 42);
    }

    #[test]
    fn test_invalid_str() {
        let b = Bytesize::from("");
        assert_eq!(b.as_usize(), 0);

        let b = Bytesize::from("invalid");
        assert_eq!(b.as_usize(), 0);

        let b = Bytesize::from("123");
        assert_eq!(b.as_usize(), 0);

        let b = Bytesize::from("XYZ");
        assert_eq!(b.as_usize(), 0);
    }

    #[test]
    fn test_large_value() {
        let b = Bytesize::from(1_000_000_000_000usize);
        assert!(b.as_u64() == 1_000_000_000_000);
        let s = b.string();
        assert!(!s.is_empty());
        // Roundtrip through string
        let c = Bytesize::from(s.as_str());
        assert_eq!(b.as_usize(), c.as_usize());
    }

    #[test]
    fn test_serde_roundtrip() {
        let b = Bytesize::from(2048usize);
        let serialized = serde_json::to_string(&b).unwrap();
        assert_eq!(serialized, "\"2K\"");
        let deserialized: Bytesize = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.as_usize(), 2048);

        let b2 = Bytesize::from(1048576usize);
        let serialized2 = serde_json::to_string(&b2).unwrap();
        assert_eq!(serialized2, "\"1M\"");
        let deserialized2: Bytesize = serde_json::from_str(&serialized2).unwrap();
        assert_eq!(deserialized2.as_usize(), 1048576);
    }
}
