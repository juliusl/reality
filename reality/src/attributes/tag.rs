use std::fmt::Debug;
use std::marker::PhantomData;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;

/// Wrapper struct to include a tag w/ a parsed value,
///
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Tagged<T: FromStr + Send + Sync + 'static> {
    /// Untagged value,
    ///
    value: Option<T>,
    /// Map of values,
    ///
    tag: Option<String>,
}

impl<T: FromStr + Clone + Send + Sync + 'static> Clone for Tagged<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            tag: self.tag.clone(),
        }
    }
}

impl<T: FromStr + Send + Sync + 'static> Tagged<T> {
    /// Returns the default value for this tag,
    ///
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the value of this tag,
    ///
    pub fn tag(&self) -> Option<&String> {
        self.tag.as_ref()
    }

    /// Returns true if this container matches the provided tagged value,
    ///
    pub fn is_tag(&self, tag: impl AsRef<str>) -> bool {
        self.tag.as_ref().filter(|t| *t == tag.as_ref()).is_some()
    }

    /// Sets a tag on this container,
    ///
    pub fn set_tag(&mut self, tag: impl Into<String>) {
        self.tag = Some(tag.into());
    }
}

impl<T: FromStr + Send + Sync + 'static> FromStr for Tagged<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = T::from_str(s)?;
        Ok(Tagged {
            value: Some(value),
            tag: None,
        })
    }
}

/// Type-alias for a Delimitted<',', String>,
///
pub type CommaSeperatedStrings = Delimitted<',', String>;

#[derive(Serialize, Deserialize)]
pub struct Delimitted<const DELIM: char, T: FromStr + Send + Sync + 'static> {
    value: String,
    cursor: usize,
    #[serde(skip)]
    _t: PhantomData<T>,
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Clone for Delimitted<DELIM, T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            cursor: self.cursor,
            _t: PhantomData,
        }
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Debug for Delimitted<DELIM, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Delimitted")
            .field("value", &self.value)
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> FromStr for Delimitted<DELIM, T> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Delimitted {
            value: s.to_string(),
            cursor: 0,
            _t: PhantomData,
        })
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Iterator for Delimitted<DELIM, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut value = self.value.split(DELIM).rev().skip(self.cursor);
        self.cursor += 1;
        value.next().and_then(|v| T::from_str(v.trim()).ok())
    }
}

// TODO: Enabling this would allow for more multi-vector use-cases.
// /// Struct that contains the parsed inner type as well as the idx 
// /// of the value.
// /// 
// /// 
// /// 
// pub struct Indexed<T>
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     idx: usize,
//     val: T,
// }

// impl<T> Indexed<T> 
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     /// Returns this struct w/ index set,
//     /// 
//     pub fn with_index(mut self, idx: usize) -> Self {
//         self.idx = idx;
//         self
//     }
// }

// impl<T> FromStr for Indexed<T> 
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     type Err = anyhow::Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(Self { val: T::from_str(s)?, idx: 0 })
//     }
// }
