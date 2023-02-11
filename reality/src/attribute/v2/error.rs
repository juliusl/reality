use toml_edit::Item;

/// Struct for build errors,
///
#[derive(Default, Debug)]
pub struct Error {
    /// If this error is related to document state, this item will contain additional information,
    ///
    pub toml_item: Option<Item>,
}
