use std::{fmt::Write, str::FromStr};

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
    /// Returns an error if the `next` ontains a `.`,
    ///
    pub fn join(&mut self, next: impl AsRef<str>) -> Result<&mut Self, Error> {
        let next = next.as_ref();

        if next.contains(".") {
            return Err(Error::default());
        }

        write!(self.buf, ".{next}")?;
        self.len += 1;
        Ok(self)
    }

    /// Returns the value at positioned at `index`,
    ///
    pub fn pos(&self, index: usize) -> Result<&str, Error> {
        if index > self.len {
            return Err(Error::default());
        }

        for next in self.buf.split(".").skip(index).take(1) {
            return Ok(next);
        }

        Err(Error::default())
    }

    /// Returns the root identifier,
    ///
    pub fn root(&self) -> &str {
        self.pos(0).ok().unwrap_or("")
    }
}

impl FromStr for Identifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(".");

        if let Some(Ok(mut root)) = parts.next().map(|r| Identifier::try_create_root(r)) {
            for part in parts {
                root.join(part)?;
            }

            Ok(root)
        } else {
            Err(Error::default())
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use super::Identifier;

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
    }
}
