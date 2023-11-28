mod attribute;
mod attribute_type;
mod decorated;
mod decoration;
mod fields;
mod parser;
mod storage_target;
mod visit;

pub mod prelude {
    pub(super) use std::str::FromStr;

    pub use super::attribute::Attribute;
    pub use super::attribute::Node;
    pub use super::attribute::Property;
    pub use super::attribute_type::*;
    pub use super::decorated::CommaSeperatedStrings;
    pub use super::decorated::Decorated;
    pub use super::decorated::Delimitted;
    pub use super::decoration::Decoration;
    pub use super::fields::*;
    pub use super::parser::AttributeParser;
    pub use super::parser::Comments;
    pub use super::parser::HostedResource;
    pub use super::parser::ParsedAttributes;
    pub use super::parser::ParsedBlock;
    pub use super::parser::Properties;
    pub use super::parser::Tag;
    pub use super::storage_target::prelude::*;
    pub use super::visit::Field;
    pub use super::visit::FieldMut;
    pub use super::visit::FieldOwned;
    pub use super::visit::FieldPacket;
    pub use super::visit::FieldPacketType;
    pub use super::visit::Frame;
    pub use super::visit::FrameUpdates;
    pub use super::visit::SetField;
    pub use super::visit::ToFrame;
    pub use super::visit::Visit;
    pub use super::visit::VisitMut;

    /// Returns fields for an attribute type,
    ///
    pub fn visitor<S: StorageTarget, T>(
        attr_ty: &(impl AttributeType<S> + Visit<T>),
    ) -> Vec<Field<'_, T>> {
        attr_ty.visitor::<T>()
    }

    /// Returns mutable fields for an attribute type,
    ///
    pub fn visitor_mut<'a: 'b, 'b, S: StorageTarget, T>(
        attr_ty: &'a mut (impl AttributeType<S> + VisitMut<T>),
    ) -> Vec<FieldMut<'b, T>> {
        attr_ty.visitor_mut::<T>()
    }

    pub trait FindField<T> {
        type Output;

        fn find_field<Owner>(&self, name: impl AsRef<str>) -> Option<&Self::Output>;
    }

    impl<'a, T> FindField<T> for Vec<Field<'a, T>> {
        type Output = Field<'a, T>;

        fn find_field<Owner>(&self, name: impl AsRef<str>) -> Option<&Self::Output> {
            self.iter().find(|f| {
                f.name == name.as_ref()
                    && if std::any::type_name::<Owner>() != std::any::type_name::<()>() {
                        std::any::type_name::<Owner>() == f.owner
                    } else {
                        true
                    }
            })
        }
    }

    impl<'a, T> FindField<T> for Vec<FieldMut<'a, T>> {
        type Output = FieldMut<'a, T>;

        fn find_field<Owner>(&self, name: impl AsRef<str>) -> Option<&Self::Output> {
            self.iter().find(|f| {
                f.name == name.as_ref()
                    && if f.owner != std::any::type_name::<()>() {
                        std::any::type_name::<Owner>() == f.owner
                    } else {
                        true
                    }
            })
        }
    }
}

pub use prelude::*;

mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::prelude::*;
    use crate::BlockObject;
    use async_trait::async_trait;
    use reality_derive::Reality;
    use serde::{Deserialize, Serialize};

    /// Tests derive macro expansion
    ///
    #[derive(Reality, Clone, Serialize, Deserialize, Debug, Default)]
    #[reality(
        rename = "test",
        load=on_load,
    )]
    pub struct Test {
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
        _description: Decorated<String>,
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
    }

    /// Called when loading this object,
    ///
    #[allow(dead_code)]
    async fn on_load<S>(storage: AsyncStorageTarget<S>)
    where
        S: StorageTarget + Send + Sync + 'static,
    {
        storage
            .maybe_intialize_dispatcher::<u64>(ResourceKey::root())
            .await;
    }

    #[allow(dead_code)]
    fn on_name(
        test: &mut Test,
        value: String,
        _tag: Option<&String>,
    ) {
        test.name = value;
        test._description = Decorated::default();
        test._test2 = Test2 {};
    }

    impl FromStr for Test {
        type Err = anyhow::Error;

        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            todo!()
        }
    }

    #[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Default, Debug)]
    pub struct Test2 {}

    impl<S: StorageTarget + Send + Sync + 'static> AttributeType<S> for Test2 {
        fn symbol() -> &'static str {
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

        parser.set_storage(std::sync::Arc::new(RwLock::new(Shared::default())));

        let ns = parser
            .namespace("test_namespace")
            .expect("should be able to create");

        let mut disp = ns.dispatcher::<Test>(ResourceKey::root()).await;

        disp.queue_dispatch_mut(|t| {
            t.author = format!("test_v2_parser");
        });
    }

    #[test]
    fn test_visit() {
        let mut test = Test::default();

        let fields = <Test as VisitMut<BTreeMap<String, String>>>::visit_mut(&mut test);
        println!("{:#?}", fields);
        test.set_field(FieldOwned {
            owner: std::any::type_name::<Test>().to_string(),
            name: "name".to_string(),
            offset: 0,
            value: String::from("hello-set-field"),
        });

        assert_eq!("hello-set-field", test.name.as_str());
        let frames = test.to_frame(ResourceKey::root());

        for frame in frames.fields {
            println!("{:?}", frame);
        }

        let vtest = VirtualTest::new(test);

        let _listener = vtest.listen();

        let tx = vtest.name.start_tx();

        let tx_result = tx.next(|f| {
            Ok(f)
        }).finish();

        match tx_result {
            Ok(_next) => {
            },
            Err(_) => todo!(),
        }

        // listener.changed().await.unwrap();
        // let _vtest = listener.borrow_and_update();
    }
}
