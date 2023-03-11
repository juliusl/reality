use std::ops::Deref;
use std::sync::Arc;

use async_trait::async_trait;
use specs::Entity;
use specs::LazyUpdate;
use specs::world::LazyBuilder;
use crate::Error;
use crate::v2::Properties;
use crate::Identifier;

use super::Build;

/// Trait to provide an implementation for handling results of a Thunk call,
/// 
/// If the type also implements Build + Accept, it can be used w/ a system to identify identifiers of interest and build 
/// a listener aware of that identifier.
/// 
#[async_trait]
pub trait Listen 
where
    Self: Send + Sync
{   
    /// Called on properties returned from a ThunkCall,
    /// 
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error>;

    /// Called on identifiers and if accepted, lazily builds and returns an entity,
    /// 
    /// (TODO) The entity created will be associated to the owner of the identity.
    /// 
    /// Otherwise returns ERROR_NOT_ACCEPTED,
    /// 
    async fn accept<'a>(&self, identifier: &Identifier, lazy_builder: LazyBuilder<'a>) -> Result<Entity, Error>
    where
        Self: Build + Accept 
    {
        if let Ok(listener) = Accept::accept(self, identifier).await {
            listener.build(lazy_builder)
        } else {
            Err(ERROR_NOT_ACCEPTED.into())
        }
    }
}

#[async_trait]
impl Listen for Arc<dyn Listen> {
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.deref().listen(properties, lazy_update).await
    }
}

/// Trait to provide an implementation for accepting an identifier,
/// 
#[async_trait]
pub trait Accept
where
    Self: Send + Sync + Sized
{
    /// If accepting identifier, returns self that is aware of identifier,
    /// 
    /// Note: return the error ERROR_NOT_ACCEPTED to indiciate that this type does not accept the identifier,
    /// 
    async fn accept(&self, identifier: &Identifier) -> Result<Self, Error>;
}

/// Super-trait for a type that implements Listen + Accept + Build,
/// 
pub trait Listener: Listen + Accept + Build {}

/// Error returned when identifier is not accepted,
/// 
pub const ERROR_NOT_ACCEPTED: &'static str = "Listener does not accept identifier";
