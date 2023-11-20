use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;

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
        Dir(std::env::current_dir().expect("should be able to read current dir")).workspace()
    }
}

/// Constructs a workspace from a directory path,
/// 
pub struct Dir(pub PathBuf);

impl Dir {
    /// Scans the directory for .md and .runmd files and returns a workspace,
    /// 
    pub fn workspace(self) -> Workspace {
        let mut workspace = Empty.workspace();

        read_dir(
            &mut workspace,
            std::env::current_dir().expect("should be able to read current dir"),
        );

        workspace
    }
}

fn read_dir(workspace: &mut Workspace, dir: impl AsRef<Path>) {
    let read_dir = std::fs::read_dir(dir.as_ref()).expect("should be able to read dir");

    for e in read_dir {
        match e {
            Ok(ref entry) => {
                eprintln!("Scanning current directory -- {:?}", entry.path());
                match entry.path().extension().and_then(|e| e.to_str()) {
                    Some("md") | Some("runmd") => {
                        eprintln!("Adding -- {:?}", entry.path());
                        workspace.add_local(entry.path());
                    }
                    _ => {}
                }
            }
            Err(err) => {
                warn!("Couldn't enumerate - {err}");
            }
        }
    }
}

/// Returns an empty workspace,
/// 
pub struct Empty;

impl Empty {
     /// Creates a new empty workspace,
    ///
    pub fn workspace(self) -> Workspace {
        Workspace::new()
    }
}

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
        let source = Source::Local(path.into());
        debug!("Adding local path - {:?}", source);
        self.sources.push(source);
    }

    /// Adds an in-memory buffer to the list of sources,
    ///
    pub fn add_buffer(&mut self, relative: impl Into<PathBuf>, source: impl Into<String>) {
        let source = Source::TextBuffer {
            source: source.into(),
            relative: {
                let r = relative.into();
                debug!("Adding buffer at relative - {:?}", r);
                r
            },
        };
        self.sources.push(source);
    }

    /// Compiles the workspace w/ project,
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
}
