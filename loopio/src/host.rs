use reality::prelude::*;

/// A Host contains a broadly shared storage context,
///
#[derive(Reality, Clone)]
pub struct Host {
    /// Name for this host,
    ///
    #[reality(derive_fromstr)]
    pub name: String,
    /// (unused) Tag for this host,
    ///
    #[reality(ignore)]
    pub _tag: Option<String>,
    /// Host storage provided by this host,
    ///
    #[reality(ignore)]
    pub host_storage: Option<AsyncStorageTarget<Shared>>,
}

impl Host {
    /// Bind this host to a storage target,
    /// 
    pub fn bind(mut self, storage: AsyncStorageTarget<Shared>) -> Self {
        self.host_storage = Some(storage);
        self
    }
}

impl Default for Host {
    fn default() -> Self {
        Self {
            name: Default::default(),
            _tag: Default::default(),
            host_storage: None,
        }
    }
}
