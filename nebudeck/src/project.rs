use std::path::PathBuf;

use loopio::prelude::*;

use anyhow::anyhow;
use tracing::warn;

/// Nebudeck project loading plugin
///
#[derive(Reality, Debug, Default, Clone)]
#[plugin_def(
    call = prepare_workspace
)]
pub(crate) struct Project {
    /// Name of the project,
    ///
    #[reality(derive_fromstr)]
    name: String,
    /// Collection of .runmd files to load into the project workspace,
    ///
    #[reality(vec_of=PathBuf)]
    file: Vec<PathBuf>,
    /// Collection of inline .runmd source to load into the project workspace,
    ///
    #[reality(vec_of=Decorated<String>)]
    source: Vec<Decorated<String>>,
}

/// Prepares a project workspace
///
async fn prepare_workspace(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let mut nbd_project = tc.initialized::<Project>().await;

    // If not set, assume the current directory is the name of the project
    if nbd_project.name.is_empty() {
        // **Panic** If the current directory can't be returned, it likely points to an environment misconfig
        nbd_project.name = std::env::current_dir()
            .expect("should have a current dir")
            .file_name()
            .expect("should have a file name")
            .to_str()
            .expect("should be able to convert to str")
            .to_string();
    }

    eprintln!("{:#?}", nbd_project);

    let mut workspace = loopio::prelude::Workspace::default();

    workspace.set_name(nbd_project.name);

    // Process file paths
    for file in nbd_project.file.iter() {
        workspace.add_local(file);
    }

    // Process inline source
    for source in nbd_project.source.iter() {
        if let Some(value) = source.value() {
            if let Some(tag) = source.tag() {
                let s = format!(
                    r#"
```runmd 
{}
```"#,
                    value
                );
                eprintln!("Adding inline source\n{s}\n");
                workspace.add_buffer(format!("{}.md", tag), s.trim());
            } else {
                Err(anyhow!("Inline source requires a tag to be set"))?;
            }
        } else {
            warn!("Missing source, skipping defined source property");
        }
    }

    // Puts the new workspace in transient storage
    tc.transient_mut()
        .await
        .put_resource(workspace, ResourceKey::root());

    Ok(())
}

#[tokio::test]
async fn test_project() {
    // TODO: Add Nebudeck struct type to facilitate this
    let mut project = loopio::prelude::Project::<Shared>::new(Shared::default());
    project.add_node_plugin("project", |input, _, parser| {
        Project::parse(parser, input.unwrap_or(""));

        let nk = parser.parsed_node.node.transmute::<Project>();
        let node = parser
            .parsed_node
            .last()
            .expect("should have a node level")
            .clone();

        if let Some(mut storage) = parser.storage_mut() {
            storage.drain_dispatch_queues();
            let res = storage
                .current_resource(node.transmute::<Project>())
                .expect("should exist");

            storage.put_resource(res, nk.transmute());
            storage.put_resource(PluginLevel::new::<Project>(), nk.transmute());
            storage.put_resource::<ResourceKey<Project>>(nk, ResourceKey::root());
        }

        parser.parsed_node.attributes.pop();
        parser.parsed_node.attributes.push(nk.transmute());
        parser.push_link_recv::<Project>();
    });

    let mut test_workspace = EmptyWorkspace.workspace();
    test_workspace.add_buffer(
        "test-RUN.md",
        r#"
    ```runmd
    + .project
    : test-source .source "+ .operation hello-world"
    | <loopio.std.io.println> hello world
    ```
    "#,
    );

    // Test compiling a nebudeck project
    let compiled_test_workspace = test_workspace.compile(project).await.unwrap();
    let project = compiled_test_workspace.project.unwrap();

    // Create a package and get the first program
    let package = project.package().await.unwrap();
    let projects = package.search("*");
    let program = &projects.first().unwrap().program;

    // Test preparing the workspace
    let tc = program.context().unwrap();
    let result = tc.call().await.unwrap().unwrap();
    let workspace = result
        .transient_mut()
        .await
        .take_resource::<loopio::prelude::Workspace>(ResourceKey::root())
        .unwrap();

    // Test that the inline source compiles and runs
    let engine = DefaultEngine.new().compile(*workspace).await.unwrap();
    let (eh, _) = engine.default_startup().await.unwrap();
    let _ = eh.run("engine://hello-world").await.unwrap();
}