use std::{collections::BTreeMap, path::PathBuf, process::ExitStatus};

use async_trait::async_trait;
use bytes::Bytes;
use reality::prelude::*;

#[async_trait::async_trait]
pub trait StdExt {
    /// Find the text content of a file loaded in transient storage under `ResourceKey::with_hash(pathstr)`,
    ///
    /// **Plugins**:
    /// - `utility/loopio.ext.std.io.read_text_file`
    ///
    async fn find_file_text(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<String>;

    ///  Find the binary content of a file loaded in transient storage under `ResourceKey::with_hash(pathstr)`,
    ///
    /// **Plugins**:
    /// - `utility/loopio.ext.std.io.read_file`
    ///
    async fn find_file(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<Bytes>;
}

#[async_trait]
impl StdExt for ThunkContext {
    async fn find_file_text(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<String> {
        self.transient()
            .await
            .current_resource::<String>(path.into().to_str().map(ResourceKey::with_hash))
    }

    async fn find_file(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<Bytes> {
        self.transient()
            .await
            .current_resource::<Bytes>(path.into().to_str().map(ResourceKey::with_hash))
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
        // println!("reading file from {:?}", path);
        let result = tokio::fs::read_to_string(&path).await;
        // println!("{:?}", result);
        let result = result?;

        context
            .transient_mut()
            .await
            .put_resource(result, path.to_str().map(ResourceKey::with_hash));

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

        context.transient_mut().await.put_resource(
            Bytes::copy_from_slice(&result),
            path.to_str().map(ResourceKey::with_hash),
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

/// 
/// 
#[derive(Reality, Clone, Default)]
#[reality(plugin, call = start_process, rename = "utility/loopio.ext.std.process")]
pub struct Process {
    #[reality(derive_fromstr)]
    program: String,
    #[reality(map_of=String)]
    env: BTreeMap<String, String>,
    #[reality(vec_of=String)]
    arg: Vec<String>,
}

async fn start_process(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<Process>().await;

    let command = init.env.iter().fold(
        std::process::Command::new(init.program),
        |mut acc, (e, v)| {
            acc.env(e, v);
            acc
        },
    );

    let mut command = init.arg.iter().fold(command, |mut acc, a| {
        for arg in shlex::split(&a).unwrap_or_default() {
            acc.arg(arg);
        }
        acc
    });

    let child = command.spawn()?;

    let output = child.wait_with_output()?;
    let _ = CommandResult {
        output: output.stdout,
        error: output.stderr,
        status: output.status,
    };


    Ok(())
}

pub struct CommandResult {
    output: Vec<u8>,
    error: Vec<u8>,
    status: ExitStatus,
}