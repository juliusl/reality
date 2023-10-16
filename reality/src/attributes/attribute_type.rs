use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::str::FromStr;

use super::AttributeParser;
use super::StorageTarget;

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
pub struct AttributeTypeParser<S: StorageTarget + 'static>(
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
}

impl<S: StorageTarget + 'static> AttributeTypeParser<S> {
    /// Executes the stored parse function,
    ///
    pub fn parse(&self, parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        (self.1)(parser, content.as_ref().trim().to_string())
    }

    /// Returns an attribute parser for a parseable type field,
    ///
    pub fn parseable_field<const IDX: usize, Owner, T>() -> Self
    where
        S: Send + Sync + 'static,
        <T as FromStr>::Err: Send + Sync + 'static,
        Owner: OnParseField<IDX, T> + Send + Sync + 'static,
        T: FromStr + Send + Sync + 'static,
    {
        Self::new::<ParsableField<IDX, Owner, T>>()
    }

    /// Returns an attribute parser for a parseable attribute type field,
    /// 
    pub fn parseable_attribute_type_field<const IDX: usize, Owner, T>() -> Self 
    where
        S: Send + Sync + 'static,
        Owner: OnParseField<IDX, T> + Send + Sync + 'static,
        T: AttributeType<S> + Send + Sync + 'static,
    {
        Self::new::<ParsableAttributeTypeField<IDX, S, Owner, T>>()
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
pub struct ParsableField<const FIELD_OFFSET: usize, Owner, T>
where
    Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
    T: FromStr + Send + Sync + 'static,
{
    /// Optional, label for use w/ resource keys
    ///
    label: Option<&'static str>,
    /// Parsed value,
    ///
    value: Option<T>,
    /// Parsing error,
    ///
    error: Option<<T as FromStr>::Err>,
    /// Called when this field is parsed successfully and the owner exists,
    ///
    _owner: PhantomData<Owner>,
}

/// Parseable AttributeType,
///
/// Applies the attribute type's parse fn, and then queues removal and transfer to the owning type,
///
#[derive(Default)]
pub struct ParsableAttributeTypeField<const FIELD_OFFSET: usize, S, Owner, T>
where
    S: StorageTarget,
    Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
    T: AttributeType<S> + Send + Sync + 'static,
{
    _inner: PhantomData<T>,
    _owner: PhantomData<Owner>,
    _storage: PhantomData<S>,
}

impl<const FIELD_OFFSET: usize, Owner, S, T> AttributeType<S>
    for ParsableField<FIELD_OFFSET, Owner, T>
where
    <T as FromStr>::Err: Send + Sync + 'static,
    Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
    S: StorageTarget + Send + Sync + 'static,
    T: FromStr + Send + Sync + 'static,
{
    fn ident() -> &'static str {
        Owner::field_name()
    }

    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        let label = Some(<Self as AttributeType<S>>::ident());

        let parsed = match content.as_ref().parse::<T>() {
            Ok(value) => ParsableField {
                label,
                value: Some(value),
                error: None::<<T as FromStr>::Err>,
                _owner: PhantomData::<Owner>,
            },
            Err(err) => ParsableField {
                label,
                value: None::<T>,
                error: Some(err),
                _owner: PhantomData,
            },
        };

        let tag = parser.tag().cloned();
        let key = parser.attributes.last().map(|a| a.transmute::<Owner>());

        match (parser.storage(), parsed) {
            (
                Some(storage),
                ParsableField {
                    value: Some(value),
                    error: None,
                    ..
                },
            ) => {
                storage.lazy_dispatch_mut(move |s| {
                    borrow_mut!(s, Owner, key, |owner| => {
                        owner.on_parse(value, tag.as_ref());
                    });
                });
            }
            (
                Some(storage),
                ParsableField {
                    value: None,
                    error: Some(error),
                    label,
                    ..
                },
            ) => {
                type ParserError<T> = <T as FromStr>::Err;

                if let Some(cb) = storage.callback_mut::<ParserError<T>>(label.try_into().ok()) {
                    storage.lazy_callback_mut(cb, error)
                } else if let Some(cb) = storage.callback::<ParserError<T>>(label.try_into().ok()) {
                    storage.lazy_callback(cb, error)
                }
            }
            _ => {}
        }
    }
}

impl<const FIELD_OFFSET: usize, Owner, S, T> AttributeType<S>
    for ParsableAttributeTypeField<FIELD_OFFSET, S, Owner, T>
where
    Owner: OnParseField<FIELD_OFFSET, T> + Send + Sync + 'static,
    S: StorageTarget + Send + Sync + 'static,
    T: AttributeType<S> + Send + Sync + 'static,
{
    fn ident() -> &'static str {
        Owner::field_name()
    }

    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        // If the parse method did not initialize T, then it won't be able to found by the subsequent dispatch,
        T::parse(parser, content);

        // Get the current tag setting,
        let tag = parser.tag().cloned();
        let key = parser.attributes.last().map(|a| a.transmute::<Owner>());

        if let Some(storage) = parser.storage() {
            storage.lazy_dispatch_mut(move |s| {
                // If set by parse, it must be set w/ a resource key set to None
                let resource = { s.take_resource::<T>(None) };

                if let Some(resource) = resource {
                    borrow_mut!(s, Owner, key, |owner| => {
                        owner.on_parse(*resource, tag.as_ref());
                    });
                }
            })
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

/// Trait to allow for deriving an AttributeType implementation w/ each callback as a seperate resource,
///
pub trait OnParseField<const FIELD_OFFSET: usize, T: Send + Sync + 'static>
where
    Self: Send + Sync + Sized + 'static,
{
    /// Name of the field,
    ///
    fn field_name() -> &'static str;

    /// Function called when a value is parsed correctly,
    ///
    fn on_parse(&mut self, value: T, tag: Option<&String>);
}

/// Struct wrapping a handler that can be treated as a resource,
///
pub struct CallbackMut<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
    handler: fn(&mut S, Arg),
}

impl<S, Arg> CallbackMut<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
    /// Creates a new callback,
    ///
    pub fn new<H: Handler<S, Arg>>() -> Self {
        Self {
            handler: H::handle_mut,
        }
    }

    /// Calls the inner handler,
    ///
    pub fn handle(&self, s: &mut S, arg: Arg) {
        (self.handler)(s, arg)
    }
}

impl<S, Arg> Clone for CallbackMut<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}

/// Struct wrapping a handler that can be treated as a resource,
///
pub struct Callback<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
    handler: fn(&S, Arg),
}

impl<S, Arg> Callback<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
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

impl<S, Arg> Clone for Callback<S, Arg>
where
    S: StorageTarget + 'static,
    Arg: Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
        }
    }
}
