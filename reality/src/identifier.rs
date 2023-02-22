use logos::Lexer;
use logos::Logos;
use specs::Component;
use specs::VecStorage;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Display;
use std::fmt::Write;
use std::str::FromStr;
use std::sync::Arc;
use tracing::trace;

use crate::Error;

/// Struct for a dot-seperated identifier,
/// 
#[derive(Component, Default, Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord)]
#[storage(VecStorage)]
pub struct Identifier {
    /// Internal buffer,
    ///
    buf: String,
    /// Set of tags to include w/ this identifier,
    ///
    tags: BTreeSet<String>,
    /// Number of segments,
    ///
    len: usize,
    /// Parent identifier,
    parent: Option<Arc<Identifier>>,
}

impl Identifier {
    /// Adds a tag to the identifier,
    ///
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.insert(tag.into());
    }

    /// Returns true if this identifier has a tag in the set,
    ///
    pub fn contains_tag(&self, tag: impl AsRef<str>) -> bool {
        self.tags.contains(tag.as_ref())
    }

    /// Returns true if this identifier contains all tags,
    ///
    pub fn contains_tags(&self, tags: &BTreeSet<String>) -> bool {
        self.tags.is_subset(tags)
    }

    /// Removes a tag,
    /// 
    pub fn remove_tag(&mut self, tag: impl AsRef<str>) {
        self.tags.remove(tag.as_ref());
    }

    /// Returns iterator over tags,
    ///
    pub fn tags(&self) -> impl Iterator<Item = &String> {
        self.tags.iter()
    }

    /// Returns the number of tags in this identifier,
    ///
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Clears current set of tags on this identifier,
    ///
    pub fn clear_tags(&mut self) {
        self.tags.clear();
    }

    /// Sets the parent identifier,
    /// 
    pub fn set_parent(&mut self, identifier: Arc<Identifier>) {
        self.parent = Some(identifier);
    }

    /// Returns the current length of this identifier,
    ///
    /// Note: the length excludes the root of the identifier
    ///
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns a new identifier w/ root,
    ///
    /// Returns an error if the root has any `.`'s
    ///
    pub fn try_create(root: impl Into<String>) -> Result<Self, Error> {
        let root = root.into();
        let root = if root.contains(".") && !root.starts_with(r#"""#) && !root.ends_with(r#"""#) {
            format!(r#""{}""#, root)
        } else {
            format!("{root}")
        };

        Ok(Self {
            buf: root,
            len: 0,
            tags: BTreeSet::default(),
            parent: Default::default(),
        })
    }

    /// Joins the next part of the identifier,
    ///
    /// If the next part contains a `.`, it will automatically be formatted w/ quotes
    ///
    pub fn join(&mut self, next: impl AsRef<str>) -> Result<&mut Self, Error> {
        let next = next.as_ref();

        if Self::should_escape_with_quotes(next) && !next.starts_with(r#"""#) && !next.ends_with(r#"""#) {
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

    /// Clones current identifier, consumes tags, and parses a new identifier,
    ///
    pub fn commit(&self) -> Result<Identifier, Error> {
        let c = self.clone();

        format!("{:#}", c).parse()
    }

    /// Returns a merged identifier,
    /// 
    /// The current identifier will be set as the parent of the other identifier (after the other identifier is committed).
    /// The result is the committed merged identifier.
    /// 
    pub fn merge(&self, other: &Identifier) -> Result<Identifier, Error> {
        let mut merged = other.commit()?;
        merged.set_parent(Arc::new(self.commit()?));
        merged.commit()
    }

    /// Truncates the identifier by count,
    ///
    pub fn truncate(&self, count: usize) -> Result<Identifier, Error> {
        if let Some(newlen) = self.len.checked_sub(count) {
            let parts = parts(&self.buf)?
                .iter()
                .take(newlen + 1)
                .cloned()
                .collect::<Vec<_>>();

            let mut next = parts.join(".").parse::<Identifier>()?;
            next.tags = self.tags.clone();
            Ok(next)
        } else {
            Err("truncating overflow".into())
        }
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
                StringInterpolationTokens::Match(match_ident)
                | StringInterpolationTokens::EscapedMatch(match_ident)
                    if sint.start.is_none() =>
                {
                    sint.start = self
                        .buf
                        .find(&match_ident)
                        .map(|s| s + match_ident.len() + 1);
                }
                StringInterpolationTokens::OptionalSuffixAssignment(_)
                    if tokens.remainder().len() > 0 =>
                {
                    panic!("Pattern error, optional suffix assignment can only be at the end")
                }
                _ => {
                    sint.tokens.push(token);
                }
            }
        }

        let mut map = BTreeMap::<String, String>::default();
        let buf = if let Some(start) = sint.start {
            let (_, rest) = self.buf.split_at(start);
            trace!("rest: {rest}");
            if let Some(ident) = rest.parse::<Identifier>().ok() {
                trace!("ident: {ident}");
                ident.buf.to_string()
            } else {
                return None;
            }
        } else {
            self.buf.to_string()
        };

        trace!("buf: {buf} sint: {:?}", sint);

        if let Some(buf) = parts(buf).ok() {
            if buf.len() < sint.tokens.len()
                && sint
                    .tokens
                    .last()
                    .filter(|t| {
                        if let StringInterpolationTokens::OptionalSuffixAssignment(_) = t {
                            true
                        } else {
                            false
                        }
                    })
                    .is_none()
            {
                return None;
            }

            for (part, token) in buf.iter().zip(sint.tokens) {
                match token {
                    StringInterpolationTokens::Match(matches)
                    | StringInterpolationTokens::EscapedMatch(matches)
                        if matches != *part =>
                    {
                        return None;
                    }
                    StringInterpolationTokens::Match(matches)
                    | StringInterpolationTokens::EscapedMatch(matches)
                        if matches == *part =>
                    {
                        continue;
                    }
                    StringInterpolationTokens::Assignment(name)
                    | StringInterpolationTokens::OptionalSuffixAssignment(name) => {
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


    /// Returns true if the input string should be escaped w/ quotes,
    /// 
    fn should_escape_with_quotes(s: &str) -> bool {
        s.contains(".") || s.contains(" ") || s.contains("\t")
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

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(root) = self.parent.as_ref().filter(|_| f.alternate()) {
            write!(f, "{:#}", root)?;

            if !self.buf.starts_with(".") && !self.buf.is_empty() {
                write!(f, ".")?;
            }
        }

        write!(f, "{}", self.buf)?;
        if f.alternate() && self.tag_count() > 0 {
            write!(f, r#".""#)?;
            let mut tags = self.tags.iter();
            if let Some(tag) = tags.next() {
                write!(f, "{tag}")?;
            }
            for t in tags {
                write!(f, ":{t}")?;
            }
            write!(f, r#"""#)?;
        }

        Ok(())
    }
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
    /// Match this token, escaped w/ quotes,
    ///
    #[regex(r#"[.]?["][^"]*["][.]?"#, on_match)]
    EscapedMatch(String),
    /// Match this token,
    ///
    #[regex("[.]?[a-zA-Z0-9]+[.]?", on_match)]
    Match(String),
    /// Assign the value from the identifier,
    ///
    #[regex("[(][a-zA-Z-0-9]+[)]", on_assignment)]
    Assignment(String),
    /// Optionally assign a suffix,
    ///
    #[regex("[(][?][a-zA-Z0-9]+[)]", on_optional_suffix_assignment)]
    OptionalSuffixAssignment(String),
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

    let end = if lex.slice().chars().last() == Some('.') {
        lex.slice().len() - 1
    } else {
        lex.slice().len()
    };

    lex.slice()[start..end].to_string()
}

fn on_assignment(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let name = lex.slice()[1..lex.slice().len() - 1].to_string();
    name
}

fn on_optional_suffix_assignment(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let name = lex.slice()[2..lex.slice().len() - 1].to_string();
    name
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = parts(s)?;
        let mut parts = parts.iter();

        if let Some(mut root) = parts.next().and_then(|p| Self::try_create(p).ok()) {
            for p in parts {
                root.join(p)?;
            }

            Ok(root)
        } else {
            Self::try_create("")
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use logos::{Lexer, Logos};
    use tracing_test::traced_test;

    use super::{Identifier, StringInterpolationTokens};

    #[test]
    fn test_identifier() {
        let mut root = Identifier::try_create("test").expect("should be a root");

        root.join("part1")
            .expect("should be part1")
            .join("part2")
            .expect("should be part2");

        assert_eq!("test", root.pos(0).expect("should have a part"));
        assert_eq!("part1", root.pos(1).expect("should have a part"));
        assert_eq!("part2", root.pos(2).expect("should have a part"));
        root.pos(3).expect_err("should be an error");

        let root = Identifier::try_create("").expect("should be able to create root");
        assert_eq!("", root.root());

        let mut root: Identifier = "test.part1.part2".parse().expect("should be able to parse");
        root.add_tag("test");
        root.add_tag("v1");
        assert_eq!("test", root.pos(0).expect("should have a part"));
        assert_eq!("part1", root.pos(1).expect("should have a part"));
        assert_eq!("part2", root.pos(2).expect("should have a part"));
        root.pos(3).expect_err("should be an error");

        let branch = root.branch("part3").expect("should be able to branch");
        assert_eq!("part3", branch.pos(3).expect("should have a part"));
        assert_eq!(2, root.len);
        assert_eq!(
            r#"test.part1.part2.part3."test:v1""#,
            format!("{:#}", branch)
        );

        let truncated = branch.truncate(2).expect("should truncate");
        assert_eq!(r#"test.part1."test:v1""#, format!("{:#}", truncated));

        let branch = branch
            .branch("testing.branch")
            .expect("should be able to branch");
        assert_eq!(
            r#""testing.branch""#,
            branch.pos(4).expect("should have a part")
        );
    }

    /// Tests string interpolation w/ identifier
    ///
    #[test]
    #[traced_test]
    fn test_string_interpolate() {
        // Test case where quotes and a . repairs parts,
        //
        let ident: Identifier =
            r#"blocks.test."https://test.test-symbol.com.".roots.op.more.stuff.at.the.end"#
                .parse()
                .expect("should parse");

        // Test escaped quotes case
        let map = ident
            .interpolate(r#""https://test.test-symbol.com.".(a).(b)"#)
            .expect("should interpolate");
        assert_eq!("roots", map["a"].as_str());
        assert_eq!("op", map["b"].as_str());

        // Test middle escaped quotes case
        let map = ident
            .interpolate(r#"test."https://test.test-symbol.com.".(a).(b)"#)
            .expect("should interpolate");
        assert_eq!("roots", map["a"].as_str());
        assert_eq!("op", map["b"].as_str());

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
        assert_eq!(r#""some spaces in the ident""#, map["symbol"].as_str());
        assert_eq!("op", map["root"].as_str());
        assert_eq!(
            None,
            ident.interpolate("blocks.(name).(symbol).notmatch.(root)")
        );

        let map = ident
            .interpolate("blocks.test")
            .expect("should interpolate");
        assert!(map.is_empty());

        let ident: Identifier = "test.optional".parse().expect("should parse");
        let map = ident
            .interpolate("(a).(b).(?c)")
            .expect("should interpolate");
        assert_eq!("test", map["a"].as_str());
        assert_eq!("optional", map["b"].as_str());

        let map = ident.interpolate("(a).(?c)").expect("should interpolate");
        assert_eq!("test", map["a"].as_str());
        assert_eq!("optional", map["c"].as_str());

        let root = Identifier::default();
        assert_eq!(
            ".test",
            root.branch("test")
                .expect("should join")
                .to_string()
                .as_str()
        );
        assert_eq!(1, root.branch("test").expect("should join").len());

        let mut root = Identifier::default();
        root.join("test").expect("should join");
        root.add_tag("test");
        root.add_tag("v1");
        assert_eq!(r#".test."test:v1""#, format!("{:#}", root).as_str());
        assert_eq!(
            "test,v1",
            root.tags().cloned().collect::<Vec<_>>().join(",")
        );

        let comitted = root.commit().expect("should be able to commit");
        assert_eq!(r#".test."test:v1""#, format!("{comitted}").as_str());

        let ident: Identifier = ".input".parse().expect("should parse");
        assert_eq!("input", ident.pos(1).expect("should exist").as_str());

        let ident = ident.branch(" .").expect("should join");
        assert_eq!(r#".input." .""#, format!("{ident}").as_str());
    }

    #[test]
    #[should_panic]
    fn test_optional_suffix_match_format() {
        let ident: Identifier = "a.b.c".parse().expect("should parse");

        ident.interpolate("(?shouldpanic).b.c");
    }

    #[test]
    #[should_panic]
    fn test_truncation_overflow_err() {
        let ident: Identifier = "a.b.c".parse().expect("should parse");

        ident.truncate(3).expect("should error");
    }

    #[test]
    fn test_merge() {
        let a = "main.id".parse::<Identifier>().expect("should parse");
        let b = "other.id".parse::<Identifier>().expect("should parse");

        let ab = a.merge(&b).expect("should merge");

        assert_eq!("main.id.other.id", ab.to_string().as_str());
    }
}
