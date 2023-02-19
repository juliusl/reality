use toml_edit::Document;
use toml_edit::Item;

use crate::Error;

/// Trait to extend toml item indexes,
/// 
pub trait ValueIndex<'a>
where
    Self: core::ops::Index<&'a str, Output = Item>,
{
    /// Returns a result that contains an integer, otherwise returns an error,
    ///
    fn int(&'a self, ident: &'static str) -> Result<i64, Error> {
        self.find(ident, toml_edit::Value::as_integer)
    }

    /// Returns a result that contains an integer, otherwise returns an error,
    ///
    fn float(&'a self, ident: &'static str) -> Result<f64, Error> {
        self.find(ident, toml_edit::Value::as_float)
    }

    /// Returns a result that contains a string, otherwise returns an error,
    ///
    fn string(&'a self, ident: &'static str) -> Result<&str, Error> {
        self.find(ident, toml_edit::Value::as_str)
    }

    /// Returns a result that contains a boolean, otherwise returns an error,
    ///
    fn bool(&'a self, ident: &'static str) -> Result<bool, Error> {
        self.find(ident, toml_edit::Value::as_bool)
    }

    /// Finds a value, mapping it w/ map, otherwise constructs a proper error,
    ///
    fn find<T>(
        &'a self,
        ident: &'static str,
        map: fn(&'a toml_edit::Value) -> Option<T>,
    ) -> Result<T, Error> {
        if let Some(i) = self[ident].as_value().and_then(map) {
            Ok(i)
        } else {
            Err(Error::default())
        }
    }
}

impl ValueIndex<'_> for Item {}
impl ValueIndex<'_> for Document {}

#[allow(unused_imports)]
mod tests {
    use toml_edit::{Document, value};

    use crate::v2::ValueIndex;

    #[test]
    fn test_value_provider() {
        let mut test = Document::new();

        test["test_int"] = value(10);
        test["test_float"] = value(3.14);
        test["test_str"] = value("hello world");
        test["test_bool"] = value(true);

        assert_eq!(10, test.int("test_int").unwrap());
        assert_eq!(3.14, test.float("test_float").unwrap());
        assert_eq!("hello world", test.string("test_str").unwrap());
        assert_eq!(true, test.bool("test_bool").unwrap());
    }
}
