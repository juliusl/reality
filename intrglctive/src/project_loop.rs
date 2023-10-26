use reality::Project;
use reality::StorageTarget;
use std::path::PathBuf;

/// Implemented by interaction types to generalize the steps before compiling the project,
///
pub trait InteractionLoop<S, A>
where
    S: StorageTarget + 'static,
    A: AppType<S>,
{
    /// Called when the interaction loop should takeover,
    ///
    fn take_control(self, project_loop: ProjectLoop<S>);
}

/// Trait to act as an entrypoint for an app into a specific interaction loop implementation,
///
pub trait AppType<S>
where
    Self: Sized,
    S: StorageTarget + 'static,
{
    /// Creates a new storage target,
    /// 
    fn initialize_storage() -> S;

    /// Create an instance of the app type from a project_loop,
    ///
    fn create(project_loop: ProjectLoop<S>) -> Self;

    /// Starts the interaction process,
    ///
    fn start_interaction(
        project_loop: ProjectLoop<S>,
        interaction_loop: impl InteractionLoop<S, Self>,
    ) {
        interaction_loop.take_control(project_loop)
    }
}

/// Enumeration of runmd Source,
///
/// A source must have a unique relative path name.
///
pub enum Source {
    /// Path to a local file,
    ///
    /// The name of the file will be used to identify this source.
    ///
    Local(PathBuf),
    /// Textbuffer in memory w/ a relative name formatted as a Path,
    ///
    TextBuffer {
        /// Relative path identifier for this buffer,
        ///
        relative: PathBuf,
        /// Source content of this buffer,
        ///
        source: String,
    },
}

/// Struct containing data to managed during the lifetime of the project loop before giving full control over to the interaction loop,
///
pub struct ProjectLoop<S: StorageTarget + 'static> {
    /// Source of runmd content,
    ///
    pub source: Vec<Source>,
    /// Project used to build the source,
    ///
    pub project: Project<S>,
}

impl<S: StorageTarget + 'static> ProjectLoop<S> {
    /// Creates a new project loop,
    ///
    pub fn new(project: Project<S>) -> Self {
        Self {
            source: vec![],
            project,
        }
    }

    /// Set sources on the project loop,
    ///
    pub fn set_sources(&mut self, sources: Vec<Source>) {
        self.source = sources;
    }

    /// Adds a local file to list of sources,
    ///
    pub fn add_local(&mut self, path: impl Into<PathBuf>) {
        self.source.push(Source::Local(path.into()));
    }

    /// Adds an in-memory buffer to the list of sources,
    ///
    pub fn add_buffer(&mut self, relative: impl Into<PathBuf>, source: impl Into<String>) {
        self.source.push(Source::TextBuffer {
            source: source.into(),
            relative: relative.into(),
        });
    }
}
