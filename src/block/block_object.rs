use crate::{BlockProperties, CustomAttribute};

/// Types implement this trait to provide query settings at runtime,
/// 
/// When interpreting runmd, the complex value type can be used to serve the same purpose,
/// but for usage during runtime its better to declare the contract inside of code.
/// 
pub trait BlockObject {
    /// Returns block properties to use when querying for this object from state,
    ///
    fn query(&self) -> BlockProperties;

    /// Returns a custom attribute parser if implemented,
    /// 
    fn parser(&self) -> Option<CustomAttribute>;
}
