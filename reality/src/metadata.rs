use std::{fmt::Display, path::PathBuf};

/// Metadata struct,
///
#[derive(Debug, Default, Clone)]
pub struct Metadata {
    /// Runmd source
    ///
    src: Source,
    /// Approx. source position,
    ///
    pos: Position,
}

impl Metadata {
    /// Returns a new metadata struct from a source,
    ///
    pub fn new(src: Source) -> Self {
        Metadata {
            src,
            ..Default::default()
        }
    }

    /// Returns the src,
    ///
    pub fn src(&self) -> &Source {
        &self.src
    }

    /// Move the col position,
    ///
    pub fn move_col(&mut self, col: usize) {
        self.pos.col += col;
    }

    /// Updates source position for a new line,
    ///
    pub fn new_line(&mut self) {
        self.pos.line += 1;
        self.pos.col = 0;
    }
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.src, self.pos)
    }
}

/// Struct w/ source position
///
#[derive(Debug, Default, Clone)]
pub struct Position {
    /// Source line position,
    ///
    line: usize,
    /// Source col position,
    ///
    col: usize,
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ln: {} col: {}", self.line, self.col)
    }
}

/// Enumeration of runmd sources,
///
#[derive(Debug, Default, Clone)]
pub enum Source {
    /// Static source, such as static string declared in code,
    ///
    #[default]
    Static,
    /// Source from a file path,
    ///
    File(PathBuf),
}

impl Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Static => {
                write!(f, "")
            }
            Source::File(src) => {
                if let Some(file_name) = src.file_name() {
                    write!(f, "file: {}", format!("{:?}", file_name).trim_matches('"'))
                } else {
                    write!(f, "{:?}", src)
                }
            }
        }
    }
}

impl Source {
    /// Creates a new file source,
    ///
    pub fn file(src: PathBuf) -> Self {
        Source::File(src)
    }
}
