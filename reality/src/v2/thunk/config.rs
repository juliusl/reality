use crate::{v2::Property, Identifier, Error};

/// Implement to configure w/ identifier & property,
/// 
pub trait Config {
    /// Configures self w/ an identifier and property,
    /// 
    fn config(&mut self, ident: &Identifier, property: &Property);
}

#[allow(unused_imports)]
#[allow(dead_code)]
mod tests {
    use reality_derive::Config;

    use crate::{Identifier, v2::{Property, property_value}};

    use super::Config;

    #[derive(Config)]
    struct Test {
        name: String,
        is_test: bool,
        n: usize,
    }

    impl Test {
        const fn new() -> Self {
            Self { name: String::new(), is_test: false, n: 0 }
        }
    }

    #[test]
    fn test_config() {
        let mut test = Test::new();
        
        let ident = "test.a.b.name".parse::<Identifier>().unwrap();
        let property = property_value("test_name");
        test.config(&ident, &property);

        let ident = "test.a.b.is_test".parse::<Identifier>().unwrap();
        let property = property_value(true);
        test.config(&ident, &property);

        let ident = "test.a.b.n".parse::<Identifier>().unwrap();
        let property = property_value(100);
        test.config(&ident, &property);

        assert_eq!("test_name", test.name.as_str());
        assert_eq!(true, test.is_test);
        assert_eq!(100, test.n);
    }
}