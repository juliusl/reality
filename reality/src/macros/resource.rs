/// Returns an owned resource from a parser's storage target,
///
/// None; if the resource does not exist
///
/// # Examples
///
/// ```rs norun
/// if let Some(resource) = resource_owned!(parser, u64, "test") {
/// ...
/// }
///
/// if let Some(resource) = resource_owned!(parser, u64, test) {
/// ...
/// }
///
/// if let Some(resource) = resource_owned!(parser, u64) {
/// ...
/// }
/// ```
///
#[macro_export]
macro_rules! resource_owned {
    ($parser:ident, $ty:path, $key:literal) => {
        $parser.storage().and_then(|s| {
            const KEY: &'static str = key;
            s.resource::<$ty>(ResourceKey::with_hash($key))
                .map(|h| h.clone())
        })
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser.storage().and_then(|s| {
            s.resource::<$ty>(ResourceKey::with_hash($key))
                .map(|h| h.clone())
        })
    };
    ($parser:ident, $ty:path, $key:expr) => {
        $parser.storage().and_then(|s| {
            s.resource::<$ty>(ResourceKey::with_hash($key))
                .map(|h| h.clone())
        })
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser
            .storage()
            .and_then(|s| s.resource::<$ty>(None).map(|h| h.clone()))
    };
}

/// Returns borrowed access to a resource from a parser's storage target,
///
/// None, if the resource does not exist
///
#[macro_export]
macro_rules! resource {
    ($parser:ident, $ty:path, $key:literal) => {
        $parser.storage().and_then(|s| {
            const KEY: &'static str = key;
            s.resource::<$ty>(Some(ResourceKey::with_label(KEY)))
        })
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser.storage().and_then(|s| s.resource::<$ty>($key))
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser.storage().and_then(|s| s.resource::<$ty>(None))
    };
}

/// Returns mutable borrowed access to a resource from a parser's storage target,
///
/// None, if the resource does not exist
///
#[macro_export]
macro_rules! resource_mut {
    ($parser:ident, $ty:path, $key:literal) => {
        $parser.storage_mut().and_then(|s| {
            const KEY: &'static str = key;
            s.resource_mut::<$ty>(Some(ResourceKey::with_label(KEY)))
        })
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser
            .storage_mut()
            .and_then(|s| s.resource_mut::<$ty>($key))
    };
    ($parser:ident, $ty:path, $key:ident) => {
        $parser
            .storage_mut()
            .and_then(|s| s.resource_mut::<$ty>(None))
    };
}
