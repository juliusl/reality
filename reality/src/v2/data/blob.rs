use std::sync::Arc;
use std::path::PathBuf;
use async_trait::async_trait;
use serde::Serialize;
use serde::Deserialize;
use crate::Identifier;
use crate::Value;
use crate::v2::property_value;
use crate::v2::Properties;
use crate::v2::Call;
use crate::Error;

/// Struct for encoding binary data,
/// 
/// # Blob Source Types
/// 
/// base64 - data is base64 encoded 
/// file - data is stored in a local file
/// url - data is stored in a remote location, https only
/// 
#[derive(Serialize, Deserialize)]
pub struct BlobInfo {
    /// Source, 
    /// 
    /// # Sources
    /// base64 - data is a base64 encoded string
    /// 
    pub(super) src: String,
    /// Data, value depends on encoding type,
    /// 
    pub(super) data: String,
    /// Optional, blob fetcher
    /// 
    #[serde(skip)]
    pub(super) fetcher: Option<Arc<dyn Fetch>>,
    /// Optional, identifier
    /// 
    #[serde(skip)]
    pub(super) ident: Option<Identifier>,
}

impl BlobInfo {
    /// Returns self w/ a fetcher,
    /// 
    pub fn with_fetcher(mut self, fetcher: impl Fetch + 'static) -> Self {
        self.fetcher = Some(Arc::new(fetcher));
        self
    }

    /// Returns self w/ an identifier,
    /// 
    /// Note: The format is ideally to be the value name as the base, and the parent set w/ the property owner,
    /// 
    pub fn with_ident(mut self, identifier: impl Into<Identifier>) -> Self {
        self.ident = Some(identifier.into());
        self
    }

    /// Returns true if this blob info is a valid remote,
    /// 
    /// A valid remote means that there is a fetcher impl and identifier present,
    /// 
    pub fn is_valid_remote(&self) -> bool {
        self.ident.is_some() && self.fetcher.is_some()
    }

    /// Returns true if this blob info is a valid local,
    /// 
    /// A valid local means that there is an identifier and the src is local,
    /// 
    pub fn is_valid_local(&self) -> bool {
        self.ident.is_some() && self.is_local()
    }

    /// Returns true if this blob info can be fetched locally,
    /// 
    fn is_local(&self) -> bool {
        match self.src.as_ref() {
            "base64" | "local" => true, 
            _ => false
        }
    }
}

#[async_trait]
impl Call for BlobInfo {
    async fn call(&self) -> Result<Properties, Error> {
        let value = match self.fetcher.as_ref() {
            Some(fetcher) if self.ident.is_some() => {
                fetcher.fetch(self).await?
            }
            None if self.is_valid_local() => {
                crate::Value::try_from(self)?
            }
            _ => {
                return Err("Not enough information to return properties".into());
            }
        };

        let ident = self.ident.as_ref().expect("should exist just checked");
        let mut properties = Properties::new(ident.clone());
        properties[&format!("{ident}")] = property_value(value);
        Ok(properties)
    }
}

/// Trait for providing a blob fetch implementation,
/// 
#[async_trait]
pub trait Fetch 
where 
    Self: Send + Sync 
{
    /// Returns a binary value from a blob_info,
    /// 
    async fn fetch(&self, blob_info: &BlobInfo) -> Result<Value, Error>;
}

impl TryFrom<&BlobInfo> for crate::Value {
    type Error = Error;

    fn try_from(value: &BlobInfo) -> Result<Self, Self::Error> {
        match value.src.as_str() {
            "base64" => match base64::decode(&value.data) {
                Ok(data) => {
                    Ok(crate::Value::BinaryVector(data))
                },
                Err(err) => Err(format!("Could not convert decode base64, {err}").into()),
            },
            "local" => match PathBuf::from(&value.data).canonicalize() {
                Ok(path) => {
                    Ok(std::fs::read(path).map(|v| crate::Value::BinaryVector(v))?)
                },
                Err(err) => Err(format!("Could not canonicalize path, {err}").into()),
            }
            _ if value.is_valid_remote() => Err(BLOB_INFO_USE_REMOTE.into()),
            _ => Err(BLOB_INFO_CONVERT_ERROR.into())
        }
    }
}

/// Error message if blob cannot be converted into a value locally,
/// 
pub const BLOB_INFO_CONVERT_ERROR: &'static str = "Could not convert blob info into value";

/// Error message if blob cannot be converted into a value locally, but a fetcher and identifier are present,
/// 
pub const BLOB_INFO_USE_REMOTE: &'static str = "Could not convert blob info, use fetcher impl w/ call fn instead"; 