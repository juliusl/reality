use super::BuildRef;
use super::Listen;
use super::Properties;
use super::WorldRef;
use crate::Error;
use crate::Identifier;
use async_trait::async_trait;
use specs::Component;
use specs::Entity;
use specs::HashMapStorage;
use specs::LazyUpdate;
use specs::WorldExt;
use std::collections::BTreeMap;
use std::marker::PhantomData;

/// Log of built entities,
///
#[derive(Debug, Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct BuildLog {
    /// Index mapping identifiers into their current entities,
    ///
    pub(super) index: BTreeMap<Identifier, Entity>,
}

impl BuildLog {
    /// Returns a reference to the build log's index,
    ///
    pub fn index(&self) -> &BTreeMap<Identifier, Entity> {
        &self.index
    }

    /// Returns a mutable reference to build log's index,
    /// 
    pub fn index_mut(&mut self) -> &mut BTreeMap<Identifier, Entity> {
        &mut self.index
    }

    /// Searches for an object by identifier,
    ///
    pub fn try_get(&self, identifier: &Identifier) -> Result<Entity, Error> {
        let search = identifier.commit()?;

        self.index()
            .get(&search)
            .map(|e| Ok(e))
            .unwrap_or(Err(
                format!("Could not find object w/ {:#}", identifier).into()
            ))
            .copied()
    }

    /// Searches the index by identity w/ a reverse interpolation identity pattern,
    ///
    /// Returns an iterator over the results,
    ///
    pub fn search_index(
        &self,
        ident_pat: impl Into<String>,
    ) -> impl Iterator<Item = (&Identifier, BTreeMap<String, String>, &Entity)> {
        let pat = ident_pat.into();
        self.index().iter().filter_map(move |(ident, entity)| {
            if let Some(map) = ident.interpolate(&pat) {
                Some((ident, map, entity))
            } else {
                None
            }
        })
    }

    /// Updates a property from src properties, where the ident is the property name, and the parent of the ident
    /// is the ident of the object whose properties need to be updated,
    ///
    pub fn update_property(&self, src: &Properties, lazy_update: &LazyUpdate) {
        let property_name = src.owner().to_string();
        let property = src[&property_name].clone();
        if let Some(parent) = src
            .owner()
            .parent()
            .and_then(|p| p.commit().ok())
            .and_then(|p| self.index.get(&p))
            .cloned()
        {
            lazy_update.exec_mut(move |world| {
                if let Some(p) = world.write_component::<Properties>().get_mut(parent) {
                    p.set(property_name, property);
                }
            })
        }
    }

    /// Returns a build ref if the identifier resolves to an entity,
    /// 
    pub fn find_ref<'a, T>(&self, ident: &Identifier, world_ref: &'a mut impl WorldRef) -> Option<BuildRef<'a, T>> {
        self.try_get(ident).ok().map(|e| BuildRef {
            world_ref: Some(world_ref),
            entity: Some(e),
            error: None,
            _u: PhantomData,
        })
    }
}

#[async_trait]
impl Listen for BuildLog {
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.update_property(&properties, lazy_update);

        Ok(())
    }
}
