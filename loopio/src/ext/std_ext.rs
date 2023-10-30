use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use reality::prelude::*;

pub trait StdExt {
    /// Find the text content of a file loaded in transient storage under `ResourceKey::with_hash(pathstr)`,
    ///
    /// **Plugins**:
    /// - `utility/loopio.ext.std.io.read_text_file`
    ///
    fn find_file_text(&mut self, path: impl Into<PathBuf>) -> Option<String>;

    ///  Find the binary content of a file loaded in transient storage under `ResourceKey::with_hash(pathstr)`,
    ///
    /// **Plugins**:
    /// - `utility/loopio.ext.std.io.read_file`
    ///
    fn find_file(&mut self, path: impl Into<PathBuf>) -> Option<Bytes>;
}

impl StdExt for ThunkContext {
    fn find_file_text(&mut self, path: impl Into<PathBuf>) -> Option<String> {
        self.transient
            .storage
            .try_read()
            .ok()
            .and_then(|r| {
                r.resource::<String>(path.into().to_str().map(|p| ResourceKey::with_hash(p))).as_deref().cloned()
            })
            .map(|s| s.to_string())
    }

    fn find_file(&mut self, path: impl Into<PathBuf>) -> Option<Bytes> {
        self.transient
            .storage
            .try_read()
            .ok()
            .and_then(|r| {
                r.resource::<Bytes>(path.into().to_str().map(|p| ResourceKey::with_hash(p))).as_deref().cloned()
            })
            .map(|s| s.clone())
    }
}

/// Set of plugins for std.io,
///
#[derive(Reality, Clone, Default)]
#[reality(plugin, rename = "utility/loopio.ext.std.io")]
pub struct Stdio {
    /// Version to use for this ext,
    /// (unused)
    #[reality(derive_fromstr)]
    version: String,
    /// Adds a plugin to read text files,
    ///
    #[reality(ext)]
    read_text_file: ReadTextFile,
    /// Adds a plugin to read files,
    ///
    #[reality(ext)]
    read_file: ReadFile,
    /// Adds a plugin to print lines,
    ///
    #[reality(ext)]
    print_line: Println,
}

#[async_trait]
impl CallAsync for Stdio {
    async fn call(_: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Plugin for reading a file path into transient storage,
///
#[derive(Reality, Clone, Default)]
#[reality(plugin, rename = "utility/loopio.ext.std.io.read_text_file")]
pub struct ReadTextFile {
    /// Path to read string from,
    ///
    #[reality(derive_fromstr)]
    path: PathBuf,
}

#[async_trait::async_trait]
impl CallAsync for ReadTextFile {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let initialized = context.initialized::<ReadTextFile>().await;

        let path = initialized.path;

        let result = tokio::fs::read_to_string(&path).await?;

        let mut transport = context.write_transport().await;
        transport.put_resource(result, path.to_str().map(|p| ResourceKey::with_hash(p)));

        Ok(())
    }
}

/// Plugin for reading a file path into transient storage,
///
#[derive(Reality, Clone, Default)]
#[reality(plugin, rename = "utility/loopio.ext.std.io.read_file")]
pub struct ReadFile {
    /// Path to read string from,
    ///
    #[reality(derive_fromstr)]
    path: PathBuf,
}

#[async_trait::async_trait]
impl CallAsync for ReadFile {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let initialized = context.initialized::<ReadFile>().await;

        let path = initialized.path;

        let result = tokio::fs::read(&path).await?;

        let mut transport = context.write_transport().await;
        transport.put_resource(
            Bytes::copy_from_slice(&result),
            path.to_str().map(|p| ResourceKey::with_hash(p)),
        );

        Ok(())
    }
}

/// Plugin for reading a file path into transient storage,
///
#[derive(Reality, Clone, Default)]
#[reality(plugin, rename = "utility/loopio.ext.std.io.println")]
pub struct Println {
    /// Path to read string from,
    ///
    #[reality(derive_fromstr)]
    line: String,
}

#[async_trait::async_trait]
impl CallAsync for Println {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let initialized = context.initialized::<Println>().await;
        println!("{}", initialized.line);
        Ok(())
    }
}
