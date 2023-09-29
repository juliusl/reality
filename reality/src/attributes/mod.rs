mod attribute_type;

use std::convert::Infallible;
use std::str::FromStr;

pub use attribute_type::AttributeType;
pub use attribute_type::AttributeTypeParser;
pub use attribute_type::Callback;
pub use attribute_type::CallbackMut;
pub use attribute_type::Handler;
pub use attribute_type::OnParseField;

mod storage_target;
use reality_derive::AttributeType;
pub use storage_target::prelude::*;

mod parser;
pub use parser::AttributeParser;
pub use parser::AttributeTypePackage;

mod tag;
pub use tag::Tagged;

#[derive(AttributeType)]
#[reality(rename = "application/test", resource_label = "test_resource")]
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
    pub description: Tagged<String>,
    /// Test2 
    /// 
    #[reality(attribute_type)]
    pub test2: Test2,
    /// Ignored,
    /// 
    #[reality(ignore)]
    _value: T,
}

fn on_name<T: Send + Sync>(test: &mut Test<T>, value: String, _tag: Option<&String>) {
    test.name = value;
}

impl<T: Send + Sync + 'static> FromStr for Test<T> {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

pub struct Test2 {}

impl<S: StorageTarget + 'static> AttributeType<S> for Test2 {
    fn ident() -> &'static str {
        "test2"
    }

    fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
        // Manually implement
        todo!()
    }
}