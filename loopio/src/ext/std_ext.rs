use std::{collections::BTreeMap, path::PathBuf, process::ExitStatus};

use async_trait::async_trait;
use bytes::Bytes;
use reality::prelude::*;
use serde::{Deserialize, Serialize};

use crate::action::ActionExt;

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

    /// Returns the command result from transient state,
    ///
    async fn find_command_result(&self, program: &str) -> Option<CommandResult>;
}

#[async_trait]
impl StdExt for ThunkContext {
    async fn find_file_text(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<String> {
        self.transient().await.current_resource::<String>(
            path.into()
                .to_str()
                .map(ResourceKey::with_hash)
                .unwrap_or(ResourceKey::root()),
        )
    }

    async fn find_file(&mut self, path: impl Into<PathBuf> + Send + Sync) -> Option<Bytes> {
        self.transient().await.current_resource::<Bytes>(
            path.into()
                .to_str()
                .map(ResourceKey::with_hash)
                .unwrap_or(ResourceKey::root()),
        )
    }

    async fn find_command_result(&self, program: &str) -> Option<CommandResult> {
        self.transient()
            .await
            .current_resource(ResourceKey::with_hash(program))
    }
}

/// Set of plugins for std.io,
///
#[derive(Reality, Deserialize, Serialize, PartialEq, Clone, Default)]
#[reality(plugin, group = "loopio", rename = "std.io")]
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
#[derive(Reality, Serialize, Deserialize, PartialEq, PartialOrd, Clone, Default)]
#[reality(plugin, rename = "read-text-file", group = "builtin")]
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

        context.transient_mut().await.put_resource(
            result,
            path.to_str()
                .map(ResourceKey::with_hash)
                .unwrap_or(ResourceKey::root()),
        );

        Ok(())
    }
}

/// Plugin for reading a file path into transient storage,
///
#[derive(Reality, Serialize, Deserialize, PartialEq, Clone, Default)]
#[reality(plugin, rename = "read-file", group = "builtin")]
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
            path.to_str()
                .map(ResourceKey::with_hash)
                .unwrap_or(ResourceKey::root()),
        );

        Ok(())
    }
}

/// Plugin for reading a file path into transient storage,
///
#[derive(Reality, Serialize, Deserialize, PartialEq, Clone, Default)]
#[reality(plugin, group = "builtin")]
pub struct Println {
    /// Path to read string from,
    ///
    #[reality(derive_fromstr)]
    pub(crate) line: String,
    #[reality(ffi)]
    pub label: String,
}

#[async_trait::async_trait]
impl CallAsync for Println {
    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
        let initialized = context.as_remote_plugin::<Println>().await;
        if !initialized.label.is_empty() {
            print!("[{}] ", initialized.label);
        }
        println!("{}", initialized.line);
        Ok(())
    }
}

/// Process plugin,
///
#[derive(Reality, Serialize, Debug, PartialEq, PartialOrd, Deserialize, Clone, Default)]
#[reality(plugin, call = start_process, group = "builtin")]
pub struct Process {
    /// Name of the program,
    ///
    #[reality(derive_fromstr)]
    pub program: String,
    /// Environment variables the process will have access to
    ///
    #[reality(map_of=String)]
    pub env: BTreeMap<String, String>,
    /// List of arguments to add to the process,
    ///
    #[reality(vec_of=String)]
    pub arg: Vec<String>,
    /// If true, the process output will be stored
    ///
    pub piped: bool,
}

async fn start_process(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.as_remote_plugin::<Process>().await;

    let command = init.env.iter().fold(
        std::process::Command::new(&init.program),
        |mut acc, (e, v)| {
            acc.env(e, v);
            acc
        },
    );

    let mut command = init.arg.iter().fold(command, |mut acc, a| {
        for arg in shlex::split(a).unwrap_or_default() {
            acc.arg(arg);
        }
        acc
    });

    if init.piped {
        use std::process::Stdio;
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());
    }

    let child = command.spawn()?;

    let output = child.wait_with_output()?;
    let c = CommandResult {
        output: output.stdout,
        error: output.stderr,
        status: output.status,
    };

    tc.transient_mut()
        .await
        .put_resource(c, ResourceKey::with_hash(init.program.as_str()));

    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct CommandResult {
    pub output: Vec<u8>,
    pub error: Vec<u8>,
    pub status: ExitStatus,
}
