use std::str::FromStr;

/// Wrapper struct to include a tag w/ a parsed value,
///
#[derive(Debug, Default)]
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
