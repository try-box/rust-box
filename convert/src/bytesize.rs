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
        serializer.serialize_str(&self.to_string())
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
