use std::path::PathBuf;

use tracing::info;

use crate::Project;
use crate::Shared;

use super::Source;

/// Pointer struct for creating a workspace based on the current directory,
/// 
pub struct CurrentDir;

impl CurrentDir {
    /// Creates a new workspace from the current directory,
    /// 
    pub fn workspace(self) -> Workspace {

        Workspace::new()
    }
}

// TODO: Add different Workspace initializers
// pub struct Empty;
// impl Empty {
//      /// Creates a new empty workspace,
//     /// 
//     pub fn workspace(self) -> Workspace {
//         Workspace::new()
//     }
// }

/// Struct containing a workspace of sources,
///
pub struct Workspace {
    /// Sources added to the workspace,
    ///
    pub sources: Vec<Source>,
    /// Project to compile sources with,
    ///
    pub project: Option<Project<Shared>>,
}

impl Clone for Workspace {
    fn clone(&self) -> Self {
        Self {
            sources: self.sources.clone(),
            project: None,
        }
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Creates a new project loop,
    ///
    pub fn new() -> Self {
        Self {
            sources: vec![],
            project: None,
        }
    }

    /// Set sources on the project loop,
    ///
    pub fn set_sources(&mut self, sources: Vec<Source>) {
        self.sources = sources;
    }

    /// Adds a local file to list of sources,
    ///
    pub fn add_local(&mut self, path: impl Into<PathBuf>) {
        self.sources.push(Source::Local(path.into()));
    }

    /// Adds an in-memory buffer to the list of sources,
    ///
    pub fn add_buffer(&mut self, relative: impl Into<PathBuf>, source: impl Into<String>) {
        self.sources.push(Source::TextBuffer {
            source: source.into(),
            relative: relative.into(),
        });
    }

    /// Compiles the workspace,
    ///
    pub async fn compile(&self, mut project: Project<Shared>) -> anyhow::Result<Self> {
        let mut compiled = self.clone();

        for source in self.sources.iter() {
            match source {
                Source::Local(path) => {
                    info!("Compiling {:?}", path);
                    project = project.load_file(path).await?;
                }
                Source::TextBuffer { relative, source } => {
                    info!("Compiling {:?}", relative);
                    project = project.load_content(source).await?;
                }
            }
        }

        compiled.project = Some(project);
        Ok(compiled)
    }

    // TODO: implement analyze option
    // pub async fn analyze();
}
