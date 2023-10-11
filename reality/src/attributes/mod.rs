mod attribute_type;
mod attribute;
mod parser;
mod storage_target;
mod tag;

pub mod prelude {
    pub(super) use std::convert::Infallible;
    pub(super) use std::str::FromStr;

    pub use super::attribute_type::AttributeType;
    pub use super::attribute_type::AttributeTypeParser;
    pub use super::attribute_type::Callback;
    pub use super::attribute_type::CallbackMut;
    pub use super::attribute_type::Handler;
    pub use super::attribute_type::OnParseField;
    pub use super::attribute::Attribute;
    pub use super::parser::AttributeParser;
    pub use super::storage_target::prelude::*;
    pub use super::tag::Tagged;
}
pub use prelude::*;

mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::BlockObject;
    use reality_derive::BlockObjectType;
    
    pub mod reality {
        pub use crate::runmd;
    }

    /// Tests derive macro expansion
    /// 
    #[derive(BlockObjectType)]
    #[reality(
        rename = "application/test", 
        resource_label = "test_resource", 
        load=on_load
    )]
    pub struct Test<T: Send + Sync + 'static> {
        /// Name for test,
        ///
        #[reality(parse=on_name)]
        name: String,
        /// Author of the test,
        ///
        #[reality(ignore)]
        pub author: String,
        /// Description of the test,
        ///
        _description: Tagged<String>,
        /// Testing vec_of parse macro,
        /// 
        #[reality(vec_of=String)]
        _test_vec_of: Vec<String>,
        /// Testing map_of parse macro,
        /// 
        #[reality(map_of=String)]
        _test_map_of: BTreeMap<String, String>,
        /// Testing option_of parse macro,
        /// 
        #[reality(option_of=String)]
        _test_option_of: Option<String>,
        /// Test2
        ///
        #[reality(attribute_type)]
        _test2: Test2,
        /// Ignored,
        ///
        #[reality(ignore)]
        _value: T,
    }

    /// Called when loading this object,
    /// 
    #[allow(dead_code)]
    async fn on_load<S>(storage: AsyncStorageTarget<S>)
    where
        S: StorageTarget + Send + Sync + 'static,
    {
        storage.intialize_dispatcher::<u64>(None).await;
    }

    #[allow(dead_code)]
    fn on_name<T: Send + Sync>(test: &mut Test<T>, value: String, _tag: Option<&String>) {
        test.name = value;
        test._description = Tagged::default();
        test._test2 = Test2 {};
    }

    impl<T: Send + Sync + 'static> FromStr for Test<T> {
        type Err = Infallible;

        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            todo!()
        }
    }

    pub struct Test2 {}

    impl<S: StorageTarget + Send + Sync + 'static> AttributeType<S> for Test2 {
        fn ident() -> &'static str {
            "test2"
        }

        fn parse(parser: &mut AttributeParser<S>, _content: impl AsRef<str>) {
            if let Some(_storage) = parser.storage_mut() {}
        }
    }

    #[cfg(feature = "async_dispatcher")]
    #[tokio::test]
    async fn test_v2_parser() {
        let mut parser = AttributeParser::<crate::Shared>::default();

        let ns = parser
            .namespace("test_namespace")
            .expect("should be able to create");

        let mut disp = ns.dispatcher::<Test<String>>(None).await;

        disp.queue_dispatch_mut(|t| {
            t.author = format!("test_v2_parser");
        });
    }
}
