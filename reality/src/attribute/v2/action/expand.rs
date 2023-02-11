use std::sync::Arc;

use crate::attribute::v2::{Attribute, Error};

use super::Action;

/// Trait for implementing an expand action,
///
pub trait Expand
where
    Self: Send + Sync + 'static,
{
    /// Expand the current action into a stack of actions to apply,
    ///
    fn expand(self: Arc<Self>, attribute: &Attribute) -> Result<Vec<Action>, Error>;
}
