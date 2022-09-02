use std::path::Path;


/// Struct for managing the directory .world/ 
/// 
/// This directory will contain all of the world assets.
///  
#[derive(Default, Clone)]
pub struct WorldDir {
    /// The root directory, defaults to '' 
    /// 
    root: &'static str
}

impl From<&'static str> for WorldDir {
    fn from(root: &'static str) -> Self {
        Self { root }
    }
}

impl WorldDir {
    /// Returns the canonical path to the directory 
    /// 
    pub fn dir(&self) -> impl AsRef<Path> {
        Path::new(self.root)
            .join(".world")
            .canonicalize()
            .expect("should be able to canonicalize")
    }

    /// Returns true if the directory exists 
    /// 
    pub fn exists(&self) -> bool {
        self.dir().as_ref().try_exists().unwrap_or(false)
    }
}
