use crate::prelude::*;

/// Trait for types that are able to load extensions,
///
#[async_trait::async_trait]
pub trait Loader {
    /// Loads an extension, if successful returns a boxed Node which represents the extension,
    ///
    async fn load_extension(
        &self,
        extension: &str,
        tag: Option<&str>,
        input: Option<&str>,
    ) -> Option<BoxedNode>;
}
