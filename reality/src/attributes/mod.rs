mod attribute;
mod attribute_type;
mod parser;
mod storage_target;
mod tag;
mod visit;

pub mod prelude {
    pub(super) use std::convert::Infallible;
    pub(super) use std::str::FromStr;

    pub use super::attribute::Attribute;
    pub use super::attribute_type::AttributeType;
    pub use super::attribute_type::AttributeTypeParser;
    pub use super::attribute_type::Callback;
    pub use super::attribute_type::CallbackMut;
    pub use super::attribute_type::Handler;
    pub use super::attribute_type::OnParseField;
    pub use super::parser::AttributeParser;
    pub use super::parser::ParsedAttributes;
    pub use super::storage_target::prelude::*;
    pub use super::tag::Tagged;
    pub use super::visit::Field;
    pub use super::visit::FieldMut;
    pub use super::visit::FieldOwned;
    pub use super::visit::SetField;
    pub use super::visit::Visit;
    pub use super::visit::VisitMut;
    pub use super::visit::ToFrame;
    pub use super::visit::FromFrame;
    pub use super::visit::FieldPacket;
    pub use super::visit::FieldPacketType;
    pub use super::visit::Frame;

    /// Returns fields for an attribute type,
    ///
    pub fn visitor<'a, S: StorageTarget, T>(
        attr_ty: &'a (impl AttributeType<S> + Visit<T>),
    ) -> Vec<Field<'a, T>> {
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
                    && if f.owner != std::any::type_name::<()>() {
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
    use crate::BlockObject;
    use reality_derive::Reality;
    use serde::{Serialize, Deserialize};

    pub mod reality {
        pub use crate::prelude;
        pub use crate::runmd;
    }

    /// Tests derive macro expansion
    ///
    #[derive(Reality, Debug, Default)]
    #[reality(
        rename = "application/test",
        load=on_load
    )]
    pub struct Test<T: Send + Sync + 'static> {
        /// Name for test,
        ///
        #[reality(wire, parse=on_name)]
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
        #[reality(wire, map_of=String)]
        _test_map_of: BTreeMap<String, String>,
        /// Testing option_of parse macro,
        ///
        #[reality(wire, option_of=String)]
        _test_option_of: Option<String>,
        /// Test2
        ///
        #[reality(wire, attribute_type)]
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

    #[derive(Serialize, Deserialize, Clone, Default, Debug)]
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

    #[test]
    fn test_visit() {
        let mut test = Test::<String>::default();
        let fields = <Test<String> as VisitMut<BTreeMap<String, String>>>::visit_mut(&mut test);
        println!("{:#?}", fields);
        test.set_field(FieldOwned {
            owner: std::any::type_name::<Test<String>>(),
            name: "name",
            offset: 0,
            value: String::from("hello-set-field"),
        });
        
        assert_eq!("hello-set-field", test.name.as_str());
        let frames = test.to_frame(None); 
        // .drain(..).fold(vec![], |mut acc, v| {
        //     acc.push(v.into_wire());
        //     acc
        // });
        for frame in frames {
            println!("{:?}", frame);
        }
    }
}
