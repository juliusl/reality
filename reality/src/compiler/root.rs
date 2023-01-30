use specs::{Component, VecStorage};

use super::{extension::ExtensionCompileFunc};

/// Struct that represents a block root,
/// 
#[derive(Component, Clone)]
#[storage(VecStorage)]
pub struct Root {
    /// Name of the stable attribute that represents this root,
    /// 
    ident: String,
    /// Extensions that have been initialized under this root,
    /// 
    extensions: Vec<ExtensionCompileFunc>,
}

impl Root {
    /// Creates a new root,
    /// 
    pub fn new(ident: impl Into<String>) -> Self {
        Self { ident: ident.into(), extensions: vec![] }
    }

    /// Returns the identifer for this root,
    /// 
    pub fn ident(&self) -> &String {
        &self.ident
    }

    /// Add's an extension compile function to this root,
    /// 
    pub fn add_extension_compile(&mut self, func: ExtensionCompileFunc)
    {
        self.extensions.push(func);
    }
}
