use specs::{join::JoinIter, Entities, Entity, Join};

use super::Loader;

/// Trait to provide joinable world data,
///
pub trait Provider<'a, Data>
where
    Self: Sized,
    Data: Join,
{
    /// Returns joinable Data,
    ///
    fn provide(&'a self) -> Data;

    /// Returns state for an entity,
    ///
    fn state<L>(&'a self, entity: Entity) -> Option<L>
    where
        Self: AsRef<Entities<'a>>,
        L: Loader<Data = Data> + 'a,
    {
        self.state_with(entity, self.as_ref())
    }

    /// Returns state for an entity w/ entities resource,
    ///
    fn state_with<L>(&'a self, entity: Entity, entities: &Entities<'a>) -> Option<L>
    where
        L: Loader<Data = Data> + 'a,
    {
        let data = self.provide();

        L::current(entity, entities, data)
    }

    /// Returns join iterator over entities w/ state defined by Loader impl,
    ///
    /// Does not call Loader::load(..),
    ///
    fn iter_preload_state<L>(&'a self) -> JoinIter<(&'a Entities<'a>, Data)>
    where
        Self: AsRef<Entities<'a>>,
        L: Loader<Data = Data> + 'a,
    {
        (self.as_ref(), self.provide()).join()
    }

    /// Loads all entity state returning a vector,
    ///
    fn state_vec<L>(&'a self) -> Vec<(Entity, L)>
    where
        Self: AsRef<Entities<'a>>,
        L: Loader<Data = Data> + 'a,
    {
        iter_state(self).collect::<Vec<_>>()
    }
}

/// Returns an iterator that loads state from provider,
///
pub fn iter_state<'a, L, P>(provider: &'a P) -> impl Iterator<Item = (Entity, L)> + 'a
where
    P: Provider<'a, L::Data> + AsRef<Entities<'a>>,
    L: Loader + 'a,
{
    provider
        .iter_preload_state::<L>()
        .map(|(e, d)| (e, L::load(d)))
}
