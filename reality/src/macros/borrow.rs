/// Borrows a resource from a storage target,
/// 
#[macro_export]
macro_rules! borrow {
    (async $storage:ident $(,)? $ty:ty, $key:literal, |$var:ident| => $body:block )=> {
        {
            const KEY: &'static str = $key;
            if let Some(resource) = $storage.storage.read().await.resource::<$ty>(Some(ResourceKey::with_label(KEY))) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
    (async $storage:ident $(,)? $ty:ty, $key:ident, |$var:ident| => $body:block )=> {
        {
            if let Some(resource) = $storage.storage.read().await.resource::<$ty>(Some(ResourceKey::with_hash($key))) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
    (async $storage:ident $(,)? $ty:ty, |$var:ident| => $body:block )=> {
        {
            if let Some(resource) = $storage.storage.read().await.resource::<$ty>(None) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
    ($storage:ident $(,)?  $ty:ty, $key:literal, |$var:ident| => $body:block )=> {
        {
            const KEY: &'static str = $key;
            if let Some(resource) = $storage.resource::<$ty>(Some(ResourceKey::with_label(KEY))) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:ident, |$var:ident| => $body:block )=> {
        {
            if let Some(resource) = $storage.resource::<$ty>(Some(ResourceKey::with_hash($key))) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
    ($storage:ident $(,)? $ty:ty, |$var:ident| => $body:block )=> {
        {
            if let Some(resource) = $storage.resource::<$ty>(None) {
                #[allow(unused_mut)]
                let mut monad = |$var: &$ty| $body;

                monad(&resource)
            }
        }
    };
}

/// Borrows mutable access to a resource from a storage target,
/// 
#[macro_export]
macro_rules! borrow_mut {
    (async $storage:ident $(,)? $ty:ty, $key:literal, |$var:ident| => $body:block)=> {
        {
            const KEY: &'static str = $key;
            if let Some(mut resource) = $storage.storage.write().await.resource_mut::<$ty>(Some(ResourceKey::with_label(KEY))) {
                let monad = |$var: &mut $ty| $body;

                monad(resource.deref_mut())
            }
        }
    };
    (async $storage:ident $(,)? $ty:ty, $key:ident, |$var:ident| => $body:block)=> {
        {
            if let Some(mut resource) = $storage.storage.write().await.resource_mut::<$ty>($key) {
                let mut monad = |$var: &mut $ty| $body;

                monad(resource.deref_mut())
            }
        }
    };
    (async $storage:ident $(,)? $ty:ty, $key:literal, |$var:ident| => $body:block)=> {
        {
            if let Some(mut resource) = $storage.storage.write().await.resource_mut::<$ty>(None) {
                let mut monad = |$var: &mut $ty| $body;

                monad(resource).deref_mut()
            }
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:literal, |$var:ident| => $body:block)=> {
        {
            const KEY: &'static str = $key;
            if let Some(mut resource) = $storage.resource_mut::<$ty>(Some(ResourceKey::with_label(KEY))) {
                let monad = |$var: &mut $ty| $body;

                monad(resource.deref_mut())
            }
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:ident, |$var:ident| => $body:block)=> {
        {
            if let Some(mut resource) = $storage.resource_mut::<$ty>($key) {
                let monad = |$var: &mut $ty| $body;

                monad(resource.deref_mut())
            }
        }
    };
    ($storage:ident $(,)? $ty:ty, |$var:ident| => $body:block)=> {
        {
            if let Some(mut resource) = $storage.resource_mut::<$ty>(None) {
                let mut monad = |$var: &mut $ty| $body;

                monad(resource.deref_mut())
            }
        }
    };
}

/// Borrows a resource from an AsyncStorageTarget wrapper,
/// 
#[macro_export]
macro_rules! take {
    (async $storage:ident $(,)? $ty:ty, $key:literal)=> {
        {
            const KEY: &'static str = $key;
            $storage.storage.write().await.take_resource::<$ty>(Some(ResourceKey::with_label(KEY)))
        }
    };
    (async $storage:ident $(,)? $ty:ty, $key:ident)=> {
        {
            $storage.storage.write().await.take_resource::<$ty>(Some(ResourceKey::with_hash($key)))
        }
    };
    (async $storage:ident $(,)? $ty:ty)=> {
        {
            $storage.storage.write().await.take_resource::<$ty>(None)
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:literal)=> {
        {
            const KEY: &'static str = $key;
            $storage.take_resource::<$ty>(Some(ResourceKey::with_label(KEY)))
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:ident)=> {
        {
            $storage.take_resource::<$ty>(Some(ResourceKey::with_hash($key)))
        }
    };
    ($storage:ident $(,)? $ty:ty, $key:literal)=> {
        {
            $storage.take_resource::<$ty>(None) 
        }
    };
}