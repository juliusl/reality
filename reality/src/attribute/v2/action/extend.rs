use crate::attribute::v2::{Attribute, Error};

use super::Action;

/// Trait for implementing an extend action,
///
pub trait Extend
where
    Self: Send + Sync,
{
    /// Extends an attribute by returning a stack of actions to integrate,
    /// 
    fn extend(&self, attribute: &Attribute) -> Result<Vec<Action>, Error>;
}
