use std::sync::Arc;

use super::Properties;
use crate::Error;
use async_trait::async_trait;
use specs::Component;
use specs::LazyUpdate;
use specs::VecStorage;

mod call;
pub use call::Call;

mod build;
pub use build::Build;

mod update;
pub use update::Update;

/// Auto thunk trait implementations for common types,
/// 
pub mod auto {
    use crate::Error;
    use specs::Component;
    use specs::Entity;
    use specs::LazyUpdate;
    use specs::WorldExt;
    use std::fmt::Debug;
    use tracing::debug;
    use tracing::error;

    use self::existing_impl::UPDATE;

    use super::Update;

    /// Flags to indicate the behavior in cases when the type being automated already implements some thunks,
    /// 
    #[allow(dead_code)]
    pub(super) mod existing_impl {
        /// Existing Update trait implementation,
        /// 
        pub const UPDATE: usize = 0x01;
        
        /// Existing Build trait implementation,
        /// 
        pub const BUILD: usize = 0x02;
    }

    /// Pointer struct for providing overloaded thunk implementations for certain trait types,
    /// 
    /// By default, the overloading behavior assumes that the type does not already implement any thunk traits,
    /// 
    pub struct Auto<const FLAGS: usize = 0>;

    /// The type being automated has an existing implementation,
    /// 
    pub type AutoWithExistingUpdateImpl = Auto<UPDATE>;

    /// Update implementation for Components that do not implement Update,
    /// 
    /// Will ensure the component is registered and add the component to the entity being updated,
    /// 
    impl<T> Update<Auto> for T
    where
        T: Component + Clone + Debug + Send + Sync,
        <Self as Component>::Storage: Default,
    {
        fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<(), Error> {
            let next = self.clone();
            lazy_update.exec_mut(move |w| {
                w.register::<T>();

                match w.write_component::<T>().insert(updating, next) {
                    Ok(last) => {
                        last.map(|l| debug!("Component updated, last: {:?}", l));
                    }
                    Err(err) => error!("Error inserting component, {err}"),
                }
            });

            Ok(())
        }
    }

    /// Update implementation for Components that implement Update,
    /// 
    /// Will ensure the component is registered, and if the self.update is successful, will update the entity being updated,
    /// w/ a clone of self.
    /// 
    impl<T> Update<AutoWithExistingUpdateImpl> for T
    where
        T: Update + Component + Clone + Debug + Send + Sync,
        <Self as Component>::Storage: Default,
    {
        fn update(&self, updating: Entity, lazy_update: &LazyUpdate) -> Result<(), Error> {
            self.update(updating, lazy_update)?;

            Update::<Auto>::update(self, updating, lazy_update)
        }
    }
}

mod listen;
pub use listen::Accept;
pub use listen::Listen;
pub use listen::Listener;
pub use listen::ERROR_NOT_ACCEPTED;

/// Wrapper struct Component for storing a reference to a dyn Trait reference to be called later,
///
/// Before the thunk is called, it will be cloned
///
#[derive(Default, Component, Clone)]
#[storage(VecStorage)]
pub struct Thunk<T: Send + Sync + 'static> {
    /// Thunk type,
    ///
    thunk: T,
}

/// Type-alias for a thunk call component,
///
pub type ThunkCall = Thunk<Arc<dyn Call>>;

/// Type-alias for a thunk build component,
///
pub type ThunkBuild = Thunk<Arc<dyn Build>>;

/// Type-alias for a thunk update component,
///
pub type ThunkUpdate = Thunk<Arc<dyn Update>>;

/// Type-alias for a thunk listen component,
///
pub type ThunkListen = Thunk<Arc<dyn Listen>>;

/// Creates a thunk call from a type that implements Call,
///
pub fn thunk_call(call: impl Call + 'static) -> ThunkCall {
    Thunk {
        thunk: Arc::new(call),
    }
}

/// Creates a thunk build from a type that implements Build,
///
pub fn thunk_build(build: impl Build + 'static) -> ThunkBuild {
    Thunk {
        thunk: Arc::new(build),
    }
}

/// Creates a thunk update from a type that implements Update,
///
pub fn thunk_update(update: impl Update + 'static) -> ThunkUpdate {
    Thunk {
        thunk: Arc::new(update),
    }
}

/// Creates a thunk listen from a type that implements Listen,
///
pub fn thunk_listen(listen: impl Listen + 'static) -> ThunkListen {
    Thunk {
        thunk: Arc::new(listen),
    }
}

#[async_trait]
impl<T: Call + Send + Sync> Call for Thunk<T> {
    async fn call(&self) -> Result<Properties, Error> {
        self.thunk.call().await
    }
}

#[async_trait]
impl<T: Listen + Send + Sync> Listen for Thunk<T> {
    async fn listen(&self, properties: Properties, lazy_update: &LazyUpdate) -> Result<(), Error> {
        self.thunk.listen(properties, lazy_update).await
    }
}

impl<T: Build + Send + Sync> Build for Thunk<T> {
    fn build(&self, lazy_builder: specs::world::LazyBuilder) -> Result<specs::Entity, Error> {
        self.thunk.build(lazy_builder)
    }
}

impl<T: Update + Send + Sync> Update for Thunk<T> {
    fn update(
        &self,
        updating: specs::Entity,
        lazy_update: &specs::LazyUpdate,
    ) -> Result<(), Error> {
        self.thunk.update(updating, lazy_update)
    }
}

#[allow(unused_imports)]
mod tests {
    use std::ops::Deref;

    use super::thunk_build;
    use super::Build;
    use crate::v2::property_value;
    use crate::v2::thunk_call;
    use crate::v2::thunk_listen;
    use crate::v2::Call;
    use crate::v2::Listen;
    use crate::v2::Properties;
    use specs::world::LazyBuilder;
    use specs::Builder;
    use specs::LazyUpdate;
    use specs::World;
    use specs::WorldExt;

    #[test]
    fn test_build_thunk() {
        let t = thunk_build(|lb: LazyBuilder| Ok(lb.build()));

        let world = World::new();
        let lu = world.fetch::<LazyUpdate>();
        let lb = lu.create_entity(world.entities().deref());

        let e = t.build(lb).expect("should build successfully");
        assert_eq!(0, e.id());

        let t = thunk_build(|_: LazyBuilder| Err("build error".into()));

        let lb = lu.create_entity(world.entities().deref());
        let err = t.build(lb).expect_err("should be an error");
        assert_eq!("build error", err.to_string());
    }

    #[tokio::test]
    async fn test_call_thunk() {
        let t = thunk_call(|| async {
            let mut props = Properties::default();
            props["result"] = property_value("ok");
            Ok(props)
        });

        let result = t.call().await.expect("should be successful");
        assert_eq!(Some("ok"), result["result"].as_symbol_str());

        let t = thunk_call(|| async { Err("test_error".into()) });

        let result = t.call().await.expect_err("should be an error");
        assert_eq!("test_error", result.to_string());
    }

    #[tokio::test]
    async fn test_listen_thunk() {
        let t = thunk_listen(|_: Properties, _: &LazyUpdate| async { Ok(()) });

        let world = World::new();
        let lu = world.fetch::<LazyUpdate>();

        t.listen(Properties::default(), lu.deref())
            .await
            .expect("should be successful");

        let t = thunk_listen(|_: Properties, _: &LazyUpdate| async { Err("test_error".into()) });

        let result = t
            .listen(Properties::default(), lu.deref())
            .await
            .expect_err("should return an error");
        assert_eq!("test_error", result.to_string());
    }
}
