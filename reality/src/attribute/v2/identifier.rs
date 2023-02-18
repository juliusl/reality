use std::{collections::BTreeMap, fmt::Write, str::FromStr};

use logos::{Lexer, Logos};

use super::Error;

/// Struct for a dot-seperated identifier,
///
#[derive(Clone)]
pub struct Identifier {
    buf: String,
    len: usize,
}

impl Identifier {
    /// Returns a new identifier w/ root,
    ///
    /// Returns an error if the root has any `.`'s
    ///
    pub fn try_create_root(root: impl Into<String>) -> Result<Self, Error> {
        let root = root.into();
        if root.contains(".") {
            return Err(Error::default());
        }

        Ok(Self { buf: root, len: 0 })
    }

    /// Joins the next part of the identifier,
    ///
    /// If the next part contains a `.`, it will automatically be formatted w/ quotes
    pub fn join(&mut self, next: impl AsRef<str>) -> Result<&mut Self, Error> {
        let next = next.as_ref();

        if next.contains(".") && !next.starts_with(r#"""#) && !next.ends_with(r#"""#) {
            write!(self.buf, r#"."{}""#, next)?;
        } else {
            write!(self.buf, ".{next}")?;
        }

        self.len += 1;
        Ok(self)
    }

    /// Branches from the current identifier,
    /// 
    pub fn branch(&self, next: impl AsRef<str>) -> Result<Self, Error> {
        let mut clone = self.clone();

        clone.join(next).map(|c| c.to_owned())
    }

    /// Returns the value at positioned at `index`,
    ///
    pub fn pos(&self, index: usize) -> Result<String, Error> {
        if index > self.len {
            return Err(Error::default());
        }

        for next in parts(&self.buf)?.iter().skip(index).take(1) {
            return Ok(next.to_string());
        }

        Err(Error::default())
    }

    /// Returns the root identifier,
    ///
    pub fn root(&self) -> String {
        self.pos(0).ok().unwrap_or_default()
    }

    /// Interpolates a pattern expression into a map with user-assigned keys,
    ///
    /// Example,
    ///
    /// Given an identifier, "blocks.test.object" and a pattern "blocks.{name}.{symbol}"
    ///
    /// Interpolation would return a mapping such as,
    ///
    /// name = "test"
    /// symbol = "object"
    ///
    /// Given an identifier, "blocks.test.object.test" and a pattern "blocks.{name}.{symbol}.roots"
    ///
    /// Interpolation would return None, since ".roots" would not match the end of the identifier.
    ///
    pub fn interpolate(&self, pat: impl AsRef<str>) -> Option<BTreeMap<String, String>> {
        let mut tokens = StringInterpolationTokens::lexer(pat.as_ref());
        let mut sint = StringInterpolation::default();

        while let Some(token) = tokens.next() {
            match token {
                StringInterpolationTokens::Match(match_ident) if sint.start.is_none() => {
                    sint.start = self
                        .buf
                        .find(&match_ident)
                        .map(|s| s + match_ident.len() + 1);
                }
                _ => {
                    sint.tokens.push(token);
                }
            }
        }

        let mut map = BTreeMap::<String, String>::default();
        let buf = if let Some(start) = sint.start {
            let (_, rest) = self.buf.split_at(start);

            if let Some(ident) = rest.parse::<Identifier>().ok() {
                ident.buf.to_string()
            } else {
                return None;
            }
        } else {
            self.buf.to_string()
        };

        if let Some(buf) = parts(buf).ok() {
            for (part, token) in buf.iter().zip(sint.tokens) {
                match token {
                    StringInterpolationTokens::Match(matches) if matches != *part => {
                        return None;
                    }
                    StringInterpolationTokens::Match(matches) if matches == *part => {
                        continue;
                    }
                    StringInterpolationTokens::Assignment(name) => {
                        map.insert(name, part.to_string());
                    }
                    _ => {
                        return None;
                    }
                }
            }
        } else {
            return None;
        }

        Some(map)
    }
}

/// Tries to seperate buf into parts,
///
fn parts(buf: impl AsRef<str>) -> Result<Vec<String>, Error> {
    let mut coll = vec![];
    let parts = buf.as_ref().split(".");
    let mut parts = parts.peekable();

    while let Some(part) = parts.next() {
        if part.starts_with(r#"""#) && !part.ends_with(r#"""#) {
            let mut extracted = part.to_string();
            loop {
                let peek = parts.peek();
                if peek.is_none() {
                    return Err("quoted segment does not terminate".into());
                }

                let found_terminator = peek.filter(|p| p.ends_with(r#"""#)).is_some();

                let other = parts.next();
                if let Some(other) = other {
                    write!(extracted, ".{}", other)?;
                } else {
                    return Err(Error::default());
                }

                if found_terminator {
                    break;
                }
            }

            coll.push(extracted);
        } else {
            coll.push(part.to_string());
        }
    }

    Ok(coll)
}

#[derive(Default, Debug)]
struct StringInterpolation {
    start: Option<usize>,
    tokens: Vec<StringInterpolationTokens>,
}

/// Tokens to parse an identifier and interpolate values,
///
/// Example,
///
/// Given an identifier, "blocks.test.object" and a pattern "blocks.(name).(symbol)"
///
/// String interpolation would return a mapping such as,
///
/// name = "test"
/// symbol = "object"
///
#[derive(Logos, Debug)]
enum StringInterpolationTokens {
    /// Match this token,
    ///
    #[regex("[.]?[a-zA-Z0-9]+[.]", on_match)]
    Match(String),
    /// Assign the value from the identifier,
    ///
    #[regex("[(][a-zA-Z-0-9]+[)]", on_assignment)]
    Assignment(String),
    #[error]
    #[regex("[.]", logos::skip)]
    Error,
}

fn on_match(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let start = if lex.slice().chars().nth(0) == Some('.') {
        1
    } else {
        0
    };

    lex.slice()[start..lex.slice().len() - 1].to_string()
}

fn on_assignment(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let name = lex.slice()[1..lex.slice().len() - 1].to_string();
    name
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = parts(s)?;
        let mut parts = parts.iter();

        if let Some(mut root) = parts.next().and_then(|p| Self::try_create_root(p).ok()) {
            for p in parts {
                root.join(p)?;
            }

            Ok(root)
        } else {
            Self::try_create_root("")
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use logos::{Lexer, Logos};

    use super::{Identifier, StringInterpolationTokens};

    #[test]
    fn test_identifier() {
        let mut root = Identifier::try_create_root("test").expect("should be a root");

        root.join("part1")
            .expect("should be part1")
            .join("part2")
            .expect("should be part2");

        assert_eq!("test", root.pos(0).expect("should have a part"));
        assert_eq!("part1", root.pos(1).expect("should have a part"));
        assert_eq!("part2", root.pos(2).expect("should have a part"));
        root.pos(3).expect_err("should be an error");

        let root = Identifier::try_create_root("").expect("should be able to create root");
        assert_eq!("", root.root());

        let root: Identifier = "test.part1.part2".parse().expect("should be able to parse");
        assert_eq!("test", root.pos(0).expect("should have a part"));
        assert_eq!("part1", root.pos(1).expect("should have a part"));
        assert_eq!("part2", root.pos(2).expect("should have a part"));
        root.pos(3).expect_err("should be an error");

        let branch = root.branch("part3").expect("should be able to branch");
        assert_eq!("part3", branch.pos(3).expect("should have a part"));
        assert_eq!(2, root.len);

        let branch = branch.branch("testing.branch").expect("should be able to branch");
        assert_eq!(r#""testing.branch""#, branch.pos(4).expect("should have a part"));
    }

    /// Tests string interpolation w/ identifier
    ///
    #[test]
    fn test_string_interpolate() {
        // Test case where quotes and a . repairs parts,
        //
        let ident: Identifier =
            r#"blocks.test."https://test.test-symbol.com.".roots.op.more.stuff.at.the.end"#
                .parse()
                .expect("should parse");

        let map = ident
            .interpolate("blocks.(name).(symbol).roots.(root)")
            .expect("should interpolate");
        assert_eq!("test", map["name"].as_str());
        assert_eq!(r#""https://test.test-symbol.com.""#, map["symbol"].as_str());
        assert_eq!("op", map["root"].as_str());

        // Test case where quotes preserves the whole parts,
        //
        let ident: Identifier =
            r#"blocks.test."https://test-symbolcom".roots.op.more.stuff.at.the.end"#
                .parse()
                .expect("should parse");

        let map = ident
            .interpolate("blocks.(name).(symbol).roots.(root)")
            .expect("should interpolate");
        assert_eq!("test", map["name"].as_str());
        assert_eq!(r#""https://test-symbolcom""#, map["symbol"].as_str());
        assert_eq!("op", map["root"].as_str());

        // Test case where spaces in the ident
        //
        let ident: Identifier =
            r#"blocks.test.some spaces in the ident.roots.op.more.stuff.at.the.end"#
                .parse()
                .expect("should parse");

        let map = ident
            .interpolate("blocks.(name).(symbol).roots.(root)")
            .expect("should interpolate");
        assert_eq!("test", map["name"].as_str());
        assert_eq!("some spaces in the ident", map["symbol"].as_str());
        assert_eq!("op", map["root"].as_str());

        assert_eq!(
            None,
            ident.interpolate("blocks.{name}.{symbol}.notmatch.{root}")
        );
    }
}