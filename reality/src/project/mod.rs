use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::StorageTarget;
use crate::ResourceKeyHashBuilder;
use crate::ResourceKey;
use crate::AttributeParser;

mod node;
pub use node::Node;

mod extension;
pub use extension::Extension;

mod extension_controller;
pub use extension_controller::ExtensionController;
pub use extension_controller::ExtensionPlugin;

mod source;
pub use source::Source;

mod workspace;
pub use workspace::Workspace;

mod host;
pub use host::RegisterWith;

/// Block plugin fn,
///
pub type BlockPlugin<S> = Arc<dyn Fn(&mut AttributeParser<S>) + Send + Sync + 'static>;

/// Node plugin fn,
///
pub type NodePlugin<S> = Arc<dyn Fn(Option<&str>, Option<&str>, &mut AttributeParser<S>) + Send + Sync + 'static>;

/// Type-alias for a parsed node,
/// 
pub type ParsedNode<Storage> = Arc<tokio::sync::RwLock<Storage>>;

/// Type-alias for a table of storages created per node,
/// 
pub type NodeTable<Storage> = HashMap<ResourceKey<()>, ParsedNode<Storage>>;

/// Project storing the main runmd parser,
///
pub struct Project<Storage: StorageTarget + 'static> {
    root: Storage,

    pub nodes: std::sync::RwLock<NodeTable<Storage::Namespace>>,
}

impl<Storage: StorageTarget + Send + Sync + 'static> Project<Storage> {
    /// Creates a new project,
    ///
    pub fn new(root: Storage) -> Self {
        Self {
            root,
            nodes: Default::default(),
        }
    }

    /// Adds a block plugin to the project,
    ///
    /// This plugin will be used to prepare the attribute parser for all nodes evaluated within a block.
    ///
    pub fn add_block_plugin(
        &mut self,
        ty: Option<&str>,
        moniker: Option<&str>,
        plugin: impl Fn(&mut AttributeParser<Storage::Namespace>) + Send + Sync + 'static,
    ) {
        let block_info = BlockInfo {
            idx: 0,
            ty,
            moniker,
        };
        let key = ResourceKey::with_hash(block_info);

        self.root
            .put_resource::<BlockPlugin<Storage::Namespace>>(Arc::new(plugin), Some(key));
    }

    /// Adds a node plugin,
    ///
    pub fn add_node_plugin(&mut self, name: &str, plugin: impl Fn(Option<&str>, Option<&str>, &mut AttributeParser<Storage::Namespace>) + Send + Sync + 'static) {
        let key = ResourceKey::with_hash(name);

        self.root
            .put_resource::<NodePlugin<Storage::Namespace>>(Arc::new(plugin), Some(key));
    }

    /// Load a file into the project,
    ///
    pub async fn load_file(self, file: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = tokio::fs::read_to_string(file).await?;

        self.load_content(content).await
    }

    /// Load content into the project,
    ///
    pub async fn load_content(self, content: impl AsRef<str>) -> anyhow::Result<Self> {
        let loading: Loading<Storage> = self.into();

        let mut parser = runmd::prelude::Parser::new(loading.clone(), loading.clone());

        parser.parse(content.as_ref()).await;

        drop(parser);

        loading.unload()
    }
}

struct Loading<Storage: StorageTarget + Send + Sync + 'static>(Arc<Project<Storage>>);

impl<Storage: StorageTarget + Send + Sync + 'static> From<Project<Storage>>
    for Loading<Storage>
{
    fn from(value: Project<Storage>) -> Self {
        Loading(Arc::new(value))
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> Clone for Loading<Storage> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> Loading<Storage> {
    /// Unload the inner project,
    ///
    /// Will return an error if during loading a file something took additional strong references on the project
    ///
    pub fn unload(self) -> anyhow::Result<Project<Storage>> {
        if let Ok(project) = Arc::try_unwrap(self.0) {
            Ok(project)
        } else {
            panic!("could not unload project")
        }
    }

    /// Creates a new parser for the block,
    ///
    fn create_parser_for_block(
        &self,
        block_info: &BlockInfo,
    ) -> AttributeParser<Storage::Namespace> {
        let key = ResourceKey::with_hash(BlockInfo {
            idx: 0,
            ty: block_info.ty,
            moniker: block_info.moniker,
        });

        // Create a new attribute parser per-block
        let mut parser = AttributeParser::<Storage::Namespace>::default();

        // Blocks can have properties and load/unload properties
        if let Some(provider) = self
            .0
            .root
            .resource::<BlockPlugin<Storage::Namespace>>(Some(key))
        {
            provider(&mut parser);
        }

        parser
    }

    /// Applies a plugin to a parser,
    ///
    fn apply_plugin(
        &self,
        name: &str,
        input: Option<&str>,
        tag: Option<&str>,
        parser: &mut AttributeParser<Storage::Namespace>,
    ) {
        let node_plugin_key = ResourceKey::<NodePlugin<Storage::Namespace>>::with_hash(name);
        if let Some(node_plugin) = self
            .0
            .root
            .resource::<NodePlugin<Storage::Namespace>>(Some(node_plugin_key))
        {
            node_plugin(input, tag, parser);
        }
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> runmd::prelude::BlockProvider
    for Loading<Storage>
{
    fn provide(&self, block_info: BlockInfo) -> Option<runmd::prelude::BoxedNode> {
        let parser = self.create_parser_for_block(&block_info);

        Some(Box::pin(parser))
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> runmd::prelude::NodeProvider
    for Loading<Storage>
{
    fn provide(
        &self,
        name: &str,
        tag: Option<&str>,
        input: Option<&str>,
        node_info: &NodeInfo,
        block_info: &BlockInfo,
    ) -> Option<runmd::prelude::BoxedNode> {
        if let Ok(mut nodes) = self.0.nodes.write() {
            let mut key_builder = ResourceKeyHashBuilder::new_default_hasher();
            key_builder.hash(block_info);
            key_builder.hash(node_info);
            let key = key_builder.finish();

            let target = self.0.root.shared_namespace(key);
            let mut parser = self.create_parser_for_block(block_info);
            parser.set_storage(target.storage);

            self.apply_plugin(name, input, tag, &mut parser);

            if let Some(storage) = parser.clone_storage() {
                nodes.insert(key, storage);
            }

            Some(Box::pin(parser))
        } else {
            None
        }
    }
}

use std::convert::Infallible;
use std::path::PathBuf;
use std::str::FromStr;

use crate::AsyncStorageTarget;
use crate::AttributeType;
use crate::BlockObject;
use crate::OnParseField;
use reality::prelude::*;
use runmd::prelude::{BlockInfo, NodeInfo};

mod reality {
    pub use crate::*;
}

#[derive(Debug, Reality)]
#[reality(rename = "application/test")]
struct Test {
    name: String,
    file: PathBuf,
}

#[derive(Debug, Reality)]
#[reality(rename = "application/test2")]
struct Test2 {
    name: String,
    file: PathBuf,
}

impl FromStr for Test {
    type Err = Infallible;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Ok(Test {
            name: String::new(),
            file: PathBuf::from(""),
        })
    }
}

impl FromStr for Test2 {
    type Err = Infallible;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Ok(Test2 {
            name: String::new(),
            file: PathBuf::from(""),
        })
    }
}

#[tokio::test]
async fn test_project_parser() {
    use crate::Attribute;
    let mut project = Project::new(crate::Shared::default());

    project.add_node_plugin("test", |_, _, parser| {
        parser.with_object_type::<Test>();
        parser.with_object_type::<Test2>();
    });

    tokio::fs::create_dir_all(".test").await.unwrap();

    tokio::fs::write(
        ".test/v2v2test.md",
        r#"
        # Test document

        This is a test of embedded runmd blocks.

        ```runmd
        + .test
        <application/test>
        : .name Hello World 2
        : .file .test/test-1.md
        <application/test>
        : .name World Hello
        <application/test2>
        : .name World Hello3

        + .test
        <application/test>
        : .name Hello World 3
        : .file .test/test-2.md
        ```

        ```runmd
        + .test
        <application/test>
        : .name Hello World 2
        : .file .test/test-3.md

        + .test
        <application/test>
        : .name Hello World 3
        : .file .test/test-4.md
        ```
        "#,
    )
    .await
    .unwrap();

    let mut _project = project.load_file(".test/v2v2test.md").await.unwrap();

    println!("{:#?}", _project.nodes.read().unwrap().keys());

    for (k, node) in _project.nodes.write().unwrap().iter_mut() {
        let node = node.read().await;
        println!("{:?}", k);

        let attributes =  node.resource::<Vec<ResourceKey<Attribute>>>(None);

        if let Some(attributes) = attributes {
            for attr in attributes.iter() {
                let test = node.resource::<Test>(Some(attr.transmute()));
                println!("{:?}", test);
                if let Some(test) = test {
                    let fields = crate::visitor::<crate::Shared, PathBuf>(std::ops::Deref::deref(&test));
                    println!("{:#?}", fields);
                    println!("Find field: {:#?}", crate::FindField::find_field::<()>(&fields, "file"));
                }
                let test = node.resource::<Test2>(Some(attr.transmute()));
                println!("{:?}", test);

            }
        }

        // Node(node).attributes()
        // Node(node).attributes_of::<Test>()
    }
    ()
}
