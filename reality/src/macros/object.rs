/// Returns an owned resource from a parser's storage target,
/// 
/// None; if the resource does not exist
/// 
#[macro_export]
macro_rules! object_event_handler {
    ($parser:ident, $ty:path, $key:literal)=> {
        $parser.storage().and_then(|s| {
            const KEY: &'static str = key;
            s.resource::<$ty>(Some(ResourceKey::with_label(KEY)))
                .map(|h| h.clone())
        })
    };
    ($parser:ident, $ty:path, $key:ident)=> {
        $parser.storage().and_then(|s| {
            s.resource::<$ty>(Some(ResourceKey::with_hash($key)))
                .map(|h| h.clone())
        })
    };
    ($parser:ident, $ty:path, $key:ident)=> {
        $parser.storage().and_then(|s| {
            s.resource::<$ty>(None)
                .map(|h| h.clone())
        })
    };
}