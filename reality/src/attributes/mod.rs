mod attribute_type;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

pub use attribute_type::AttributeType;
pub use attribute_type::AttributeTypeParser;

mod storage_target;
pub use storage_target::prelude::*;

mod container;
pub use container::Container;

mod parser;
pub use parser::AttributeParser;

pub struct Test {
    name: String,
}

impl<S: StorageTarget<Namespace = Complex> + 'static> AttributeType<S> for Test {
    fn ident() -> &'static str {
        "test"
    }

    fn parse(parser: &mut AttributeParser<S>, _: impl AsRef<str>) {
        parser.add_parseable_with::<String>("name");

        if let Some(storage) = parser.storage() {
            if let Some(namespace) =
                storage.create_namespace(<Test as AttributeType<S>>::ident(), None)
            {
                let thread_safe = namespace.into_thread_safe();

                storage.lazy_put_resource(thread_safe, None);
            }
        }
    }
}
