use std::sync::Arc;

use crate::attribute::v2::{Attribute, Error};

use super::Action;

/// Trait for implementing an extend action,
///
pub trait Extend
where
    Self: Send + Sync + 'static,
{
    /// Extend the attribute w/ a stack of actions to apply,
    ///
    fn extend(self: Arc<Self>, attribute: &Attribute) -> Result<Vec<Action>, Error>;
}
