
use reality::v2::prelude::*;
use std::sync::Arc;

/// Extension that provides common utilities for writing plugins,
///
#[derive(Clone, Debug, Default)]
pub struct Plugin {
    properties: Properties,
}

impl Plugin {
    /// Returns a new plugin,
    ///
    pub const fn new() -> Self {
        Self {
            properties: Properties::empty(),
        }
    }

    /// Returns read-only properties of values found in properties,
    ///
    pub fn map(&self, _: &str, properties: &Vec<String>) -> Property {
        let mut output = Properties::empty();
        for name in properties {
            if let Some(prop) = self.properties.property(name) {
                output.set(name, prop.clone());
            }
        }

        Property::Properties(output.into())
    }

    /// Scans each line for formatting tokens and produces properties for each,
    ///
    pub fn format(&self, _: &str, lines: &Vec<String>) -> Property {
        let mut output = Properties::empty();

        for (_, line) in lines.iter().enumerate() {
            let tokens = format::parse_formatting_tokens(line);

            output.add(line, tokens);
        }

        Property::Properties(output.into())
    }

    /// Given a the property names of a property map and formatting map, apply formatting to string,
    ///
    pub fn apply_formatting(
        &self,
        map: impl AsRef<str>,
        formatting: impl AsRef<str>,
        string: &String,
    ) -> Option<String> {
        if let (Some(map), Some(formatting)) = (
            self.find_properties(map).as_ref(),
            self.find_properties(formatting).as_ref(),
        ) {
            if let Some(formatting) = formatting.property(string).and_then(|f| f.as_complex()) {
                let mut formatted_line = string.clone();
                for name in formatting {
                    if let Some(replacing) = map.property(name).and_then(|p| p.as_symbol()) {
                        formatted_line = formatted_line.replace(&format!("{{{name}}}"), &replacing);
                    }
                }

                return Some(formatted_line);
            }
        }

        None
    }

    /// Finds read-only properties located at prop,
    ///
    fn find_properties(&self, prop: impl AsRef<str>) -> Option<Arc<Properties>> {
        self.properties
            .property(prop)
            .and_then(|p| p.as_properties())
    }
}

impl AsRef<Properties> for Plugin {
    fn as_ref(&self) -> &Properties {
        &self.properties
    }
}

impl Visitor for Plugin {
    fn visit_property(&mut self, name: &str, property: &Property) {
        self.properties.visit_property(name, property);
    }
}

impl Visit for Plugin {
    fn visit(&self, _: (), _: &mut impl Visitor) -> Result<()> {
        Ok(())
    }
}

/// Module provides utilities for string formatting,
/// 
pub(super) mod format {
    use logos::Lexer;
    use logos::Logos;
    use std::collections::BTreeSet;

    /// Returns names of properties to be formatted,
    ///
    pub fn parse_formatting_tokens(line: &String) -> BTreeSet<String> {
        let mut lexer = FormattingTokens::lexer(line);
        let mut names = BTreeSet::new();

        while let Some(token) = lexer.next() {
            match token {
                FormattingTokens::Apply(name) => {
                    names.insert(name);
                }
                _ => {}
            }
        }

        names
    }

    /// Lexer for extracting property names to display,
    ///
    #[derive(Logos, Debug, PartialEq)]
    enum FormattingTokens {
        /// Regular content,
        ///
        #[regex("[^{} \n\r]+")]
        Content,
        /// Apply formatting w/ this property,
        ///
        #[regex("[{][^{}]+[}]", on_apply)]
        Apply(String),
        /// Whitespace
        ///
        #[token(" ")]
        Whitespace,
        /// Required by lexer,
        #[error]
        #[regex("[\n\r]", logos::skip)]
        Error,
    }

    fn on_apply(lexer: &mut Lexer<FormattingTokens>) -> Option<String> {
        let span = lexer.span();

        let name = &lexer.source()[span.start + 1..span.end - 1];

        Some(name.to_string())
    }

    #[allow(unused_imports)]
    mod tests {
        use super::FormattingTokens;
        use logos::Logos;

        #[test]
        fn test_formatting_tokens() {
            let mut lexer = FormattingTokens::lexer("Hello {name} my name is -- {other--NAME}");
            lexer.next().unwrap();
            lexer.next().unwrap();

            assert_eq!(
                FormattingTokens::Apply(String::from("name")),
                lexer.next().expect("should have a token")
            );
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();
            lexer.next().unwrap();

            assert_eq!(
                FormattingTokens::Apply(String::from("other--NAME")),
                lexer.next().expect("should have a token")
            );
        }
    }
}
