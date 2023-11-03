use super::prelude::*;

/// Struct containing extension parameters,
/// 
/// An extension is a container whose name is a media type and optional input. 
/// 
/// If a suffix is set, then the media type is formatted by applying the suffix to the name of the extension.
/// 
#[derive(Hash, Default, Debug, Clone)]
pub struct Extension<'a> {
    /// Name of this extension,
    ///
    pub(super) name: &'a str,
    /// If set, will append to the name of this extension,
    ///
    pub(super) suffix: Option<&'a str>,
    /// The input set for this extension,
    ///
    pub input: Option<Input<'a>>,
}

impl Extension<'_> {
    /// Formats and returns the type name of this extension,
    /// 
    pub fn type_name(&self) -> String {
        if let Some(suffix) = self.suffix {
            format!("{}.{}", self.name, suffix)
        } else {
            self.name.to_string()
        }
    }
}