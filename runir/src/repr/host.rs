use std::sync::Arc;

use crate::{prelude::*, define_intern_table, repr::Tag, interner::{InternResult, LevelFlags}, push_tag};

// Intern table for address values
define_intern_table!(ADDRESS: String);

/// Host level is the upper most level of representation,
///
/// Host level assigns addresses defined by the document structure to the
/// actual resource.
///
pub struct HostLevel {
    /// The address is derived by the documentation hierarchy from runmd and
    /// is some human-readable string associated to some resource.
    ///
    address: Tag<String, Arc<String>>,
}

impl HostLevel {
    /// Creates a new host level representation,
    ///
    #[inline]
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: Tag::new(&ADDRESS, Arc::new(address.into())),
        }
    }
}

impl Level for HostLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(dyn interner, &self.address);

        interner.set_level_flags(LevelFlags::LEVEL_3);

        interner.interner()
    }

    type Mount = Arc<String>;

    fn mount(&self) -> Self::Mount {
        self.address.create_value.clone()
    }
}

/// Wrapper struct with access to host tags,
///
pub struct HostRepr(pub(crate) InternHandle);

impl HostRepr {
    /// Returns the address provided by the host,
    ///
    #[inline]
    pub async fn address(&self) -> Option<Arc<String>> {
        self.0.address().await
    }

    /// Returns the address provided by the host,
    ///
    #[inline]
    pub fn try_address(&self) -> Option<Arc<String>> {
        self.0.try_address()
    }
}
