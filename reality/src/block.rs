use crate::AttributeType;
use crate::ResourceKey;
use crate::AttributeTypePackage;
use crate::StorageTarget;

/// Struct containing all attributes,
///
pub struct Block {}

pub trait BlockPackage<S: StorageTarget + 'static> {
    /// Resource key for the block package, 
    /// 
    fn resource_key() -> ResourceKey<AttributeTypePackage<S>>;

    /// Initialized package,
    /// 
    fn package() -> AttributeTypePackage<S>;
}

/// Object type that lives inside of a runmd block,
/// 
/// Initiated w/ the `+` keyword,
/// 
pub trait BlockObject<Storage: StorageTarget + 'static> : AttributeType<Storage> 
where
    Self: Sized + Send + Sync + 'static
{
}
