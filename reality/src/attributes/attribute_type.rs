use std::fmt::Debug;
use std::ops::Deref;
use std::str::FromStr;

use super::StorageTarget;
use super::AttributeParser;

/// Trait to implement a type as an AttributeType,
/// 
pub trait AttributeType<S: StorageTarget> {
    /// Identifier of the attribute type,
    /// 
    /// This identifier will be used by runmd to load this type
    /// 
    fn ident() -> &'static str;

    /// Parse content received by the runmd node into state w/ an attribute parser,
    ///
    /// The attribute parser will be given access to the storage target for the block this
    /// attribute declaration belongs to.
    /// 
    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>);
}

/// Struct for a concrete conversion of a type that implements AttributeType,
/// 
/// Allows the parse function to be stored and recalled
/// 
pub struct AttributeTypeParser<S: StorageTarget>(
    /// Identifier
    String,
    /// Parse function
    fn(&mut AttributeParser<S>, String),
);

impl<S: StorageTarget> AttributeTypeParser<S> {
    /// Creates a new parser 
    /// 
    pub fn new<A>() -> Self
    where
        A: AttributeType<S>,
    {
        Self(A::ident().to_string(), A::parse)
    }

    pub fn new_with(ident: impl AsRef<str>, parse: fn(&mut AttributeParser<S>, String)) -> Self {
        Self(ident.as_ref().to_string(), parse)
    }

    /// Returns a reference to this ident,
    ///
    pub fn ident(&self) -> &str {
        self.0.as_str()
    }

    /// Executes the stored parse function,
    /// 
    pub fn parse(&self, parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        (self.1)(parser, content.as_ref().trim().to_string())
    }

    /// Returns an attribute parser for a parseable type,
    /// 
    pub fn parseable<T: FromStr + Send + Sync + 'static>() -> Self 
    where
        S: 'static,
        <T as FromStr>::Err: Send + Sync + 'static
    {
        Self::new::<Parsable<T>>()
    }
}

impl<S: StorageTarget> Clone for AttributeTypeParser<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<S: StorageTarget> Debug for AttributeTypeParser<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AttributeTypeParser").field(&self.0).finish()
    }
}

/// Adapter for types that implement FromStr into an AttributeType,
/// 
pub struct Parsable<T: FromStr + Send + Sync + 'static> {
    /// Optional, label for use w/ resource keys
    /// 
    label: Option<&'static str>,
    /// Parsed value,
    /// 
    value: Option<T>,
    /// Parsing error,
    /// 
    error: Option<<T as FromStr>::Err>,
}

impl<S: StorageTarget + 'static, T: FromStr + Send + Sync + 'static> AttributeType<S> for Parsable<T> 
where
    <T as FromStr>::Err: Send + Sync + 'static
{
    fn ident() -> &'static str {
        std::any::type_name::<Self>()
    }

    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        let parsed = match content.as_ref().parse::<T>() {
            Ok(value) => {
                Parsable { label: None, value: Some(value), error: None::<<T as FromStr>::Err> }
            },
            Err(err) => {
                Parsable { label: None, value: None::<T>, error: Some(err) }
            },
        };

        match (parser.storage(), parsed) {
            (Some(storage), Parsable { value: Some(value), error: None, label}) => {
                storage.lazy_dispatch_mut(move |s| {
                    s.put_resource(value, label.try_into().ok());
                });
            },
            (Some(storage), Parsable { value: None, error: Some(error), label }) => {
                if let Some(callback) = storage.resource::<CallbackMut<S, <T as FromStr>::Err>>(label.try_into().ok()) {
                    let callback = callback.deref().clone();
                    storage.lazy_dispatch_mut(move |s| {
                        callback.handle(s, error);
                    })
                } else if let Some(callback) = storage.resource::<Callback<S, <T as FromStr>::Err>>(label.try_into().ok()) {
                    let callback = callback.deref().clone();
                    storage.lazy_dispatch(move |s| {
                        callback.handle(s, error);
                    })
                }
            }
            _ => {

            }
        }
    }
}

/// Helper trait for constructing concrete callback types,
/// 
pub trait Handler<S: StorageTarget + 'static, Arg: Send + Sync + 'static> {
    /// Handler function w/ a mutable reference to storage,
    /// 
    fn handle_mut(storage: &mut S, arg: Arg);

    /// Handler function w/ borrowed access to storage,
    /// 
    fn handle(storage: &S, arg: Arg);
}

/// Struct wrapping a handler that can be treated as a resource,
/// 
pub struct CallbackMut<S: StorageTarget + 'static, Arg: Send + Sync + 'static> {
    handler: fn(&mut S, Arg)
}

impl<S: StorageTarget + 'static, Arg: Send + Sync + 'static> CallbackMut<S, Arg> {
    /// Creates a new callback,
    /// 
    pub fn new<H: Handler<S, Arg>>() -> Self {
        Self { handler: H::handle_mut }
    }

    /// Calls the inner handler,
    /// 
    pub fn handle(&self, s: &mut S, arg: Arg) {
        (self.handler)(s, arg)
    }
}

impl<S: StorageTarget + 'static, Arg: Send + Sync + 'static> Clone for CallbackMut<S, Arg> {
    fn clone(&self) -> Self {
        Self { handler: self.handler.clone() }
    }
}

/// Struct wrapping a handler that can be treated as a resource,
/// 
pub struct Callback<S: StorageTarget + 'static, Arg: Send + Sync + 'static> {
    handler: fn(&S, Arg)
}

impl<S: StorageTarget + 'static, Arg: Send + Sync + 'static> Callback<S, Arg> {
    /// Creates a new callback,
    /// 
    pub fn new<H: Handler<S, Arg>>() -> Self {
        Self { handler: H::handle }
    }

    /// Calls the inner handler,
    /// 
    pub fn handle(&self, s: &S, arg: Arg) {
        (self.handler)(s, arg)
    }
}

impl<S: StorageTarget + 'static, Arg: Send + Sync + 'static> Clone for Callback<S, Arg> {
    fn clone(&self) -> Self {
        Self { handler: self.handler.clone() }
    }
}

#[test]
fn test_err_type() {
    struct Test; 

    impl<S: StorageTarget + 'static> Handler<S, <u64 as FromStr>::Err> for Test {
        fn handle_mut(storage: &mut S, arg: <u64 as FromStr>::Err) {
            todo!()
        }

        fn handle(storage: &S, arg: <u64 as FromStr>::Err) {
            todo!()
        }
    }

    let callback = CallbackMut::<crate::Simple, std::num::ParseIntError>::new::<Test>();
}