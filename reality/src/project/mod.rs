use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use crate::Attribute;
use crate::AttributeParser;
use crate::ParsedAttributes;
use crate::ParsedBlock;
use crate::ResourceKey;
use crate::ResourceKeyHashBuilder;
use crate::StorageTarget;

mod node;
pub use node::Node;

mod extension;
pub use extension::Transform;

mod source;
use runmd::prelude::BlockInfo;
use runmd::prelude::NodeInfo;
use serde::Deserialize;
use serde::Serialize;
pub use source::Source;

mod workspace;
pub use workspace::CurrentDir;
pub use workspace::Dir;
pub use workspace::Empty as EmptyWorkspace;
pub use workspace::Workspace;

mod host;
pub use host::RegisterWith;

/// Block plugin fn,
///
pub type BlockPlugin<S> = Arc<dyn Fn(&mut AttributeParser<S>) + Send + Sync + 'static>;

/// Node plugin fn,
///
pub type NodePlugin<S> =
    Arc<dyn Fn(Option<&str>, Option<&str>, &mut AttributeParser<S>) + Send + Sync + 'static>;

/// Type-alias for a parsed node,
///
pub type ParsedNode<Storage> = Arc<tokio::sync::RwLock<Storage>>;

/// Type-alias for a table of storages created per node,
///
pub type NodeTable<Storage> = HashMap<ResourceKey<crate::attributes::Node>, ParsedNode<Storage>>;

/// Project storing the main runmd parser,
///
pub struct Project<Storage: StorageTarget + 'static> {
    root: Storage,

    pub nodes: tokio::sync::RwLock<NodeTable<Storage::Namespace>>,
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
            .put_resource::<BlockPlugin<Storage::Namespace>>(Arc::new(plugin), key);
    }

    /// Adds a node plugin,
    ///
    pub fn add_node_plugin(
        &mut self,
        name: &str,
        plugin: impl Fn(Option<&str>, Option<&str>, &mut AttributeParser<Storage::Namespace>)
            + Send
            + Sync
            + 'static,
    ) {
        let key = ResourceKey::with_hash(name);

        self.root
            .put_resource::<NodePlugin<Storage::Namespace>>(Arc::new(plugin), key);
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

    /// Returns the parsed block from this project,
    ///
    pub async fn parsed_block(&self) -> anyhow::Result<ParsedBlock> {
        let nodes = self.nodes.read().await;

        let mut block = ParsedBlock {
            nodes: HashMap::new(),
            paths: BTreeMap::new(),
            resource_paths: BTreeMap::new(),
        };
        for (rk, s) in nodes.deref().iter() {
            let s = s.read().await;

            if let Some(parsed) = s.current_resource::<ParsedAttributes>(ResourceKey::root()) {
                block.nodes.insert(rk.transmute(), parsed);
            }
        }

        Ok(block)
    }
}

struct Loading<Storage: StorageTarget + Send + Sync + 'static>(Arc<Project<Storage>>);

impl<Storage: StorageTarget + Send + Sync + 'static> From<Project<Storage>> for Loading<Storage> {
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
        node: Option<ResourceKey<Attribute>>,
    ) -> AttributeParser<Storage::Namespace> {
        let key = ResourceKey::with_hash(BlockInfo {
            idx: 0,
            ty: block_info.ty,
            moniker: block_info.moniker,
        });

        // Create a new attribute parser per-block
        let mut parser = AttributeParser::<Storage::Namespace>::default();

        if let Some(node) = node {
            parser.attributes.node = node;
        }

        // Blocks can have properties and load/unload properties
        if let Some(provider) = self
            .0
            .root
            .current_resource::<BlockPlugin<Storage::Namespace>>(key)
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
            .current_resource::<NodePlugin<Storage::Namespace>>(node_plugin_key)
        {
            node_plugin(input, tag, parser);
        }
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> runmd::prelude::BlockProvider
    for Loading<Storage>
{
    fn provide(&self, block_info: BlockInfo) -> Option<runmd::prelude::BoxedNode> {
        let parser = self.create_parser_for_block(&block_info, None);

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
        if let Ok(mut nodes) = self.0.nodes.try_write() {
            let mut key_builder = ResourceKeyHashBuilder::new_default_hasher();
            key_builder.hash(block_info);
            key_builder.hash(node_info);
            let key = key_builder.finish();

            let target = self.0.root.shared_namespace(key);
            let mut parser = self.create_parser_for_block(block_info, Some(key.transmute()));
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
mod tests {
    use super::*;

    use std::convert::Infallible;
    use std::path::PathBuf;
    use std::str::FromStr;
    
    use crate::AsyncStorageTarget;
    use crate::AttributeType;
    use crate::BlockObject;
    use crate::OnParseField;
    use reality::prelude::*;
    
    mod reality {
        pub use crate::*;
    }
    
    #[derive(Debug, Default, Serialize, Clone, Reality)]
    #[reality(group = "reality", call = test_noop, plugin)]
    pub struct Test {
        pub name: String,
        pub file: PathBuf,
    }

    async fn test_noop(_tc: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }

    
    #[derive(Debug, Serialize, Default, Deserialize, Clone, Reality)]
    #[reality(group = "reality", call = test_noop, plugin)]
    pub struct Test2 {
        name: String,
        file: PathBuf,
        #[reality(attribute_type=Test3, not_wire)]
        test3: Test3,
    }
    
    #[derive(Reality, Default, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
    #[reality(call = test_noop, plugin)]
    pub enum Test3 {
        A {
            #[reality(not_wire)]
            a: String,
        },
        _B {
            #[reality(not_wire)]
            b: String,
        },
        #[default]
        D,
    }
    
    impl FromStr for Test3 {
        type Err = Infallible;
    
        fn from_str(_s: &str) -> Result<Self, Self::Err> {
            Ok(Test3::A { a: String::new() })
        }
    }
    
    use async_trait::async_trait;
    
    #[derive(Reality, Serialize, Default, Clone, Debug)]
    #[reality(plugin, call = test_call)]
    pub struct Test4;
    
    pub async fn test_call(_: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }
    
    #[test]
    fn test() {
        let _test = Test3::A { a: String::new() };
    }
    impl Test3 {
        fn __test1(&self) -> &String {
            struct _Test {
                a: String,
            }
    
            let _test = Test3::A {
                a: Default::default(),
            };
    
            if let Test3::A { a } = self {
                a
            } else {
                unreachable!()
            }
        }
    }
    
    // impl FromStr for Test3 {
    //     type Err = anyhow::Error;
    
    //     fn from_str(s: &str) -> Result<Self, Self::Err> {
    //         match s {
    //             "a" => {
    //                 Ok(Test3::A { a: s.to_string() })
    //             },
    //             "b" => {
    //                 Ok(Test3::B { b: s.to_string() })
    //             },
    //             _ => {
    //                 Err(anyhow::anyhow!("unrecognized input"))
    //             }
    //         }
    //     }
    // }
    
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
                test3: Test3::A { a: String::new() },
            })
        }
    }
    
    #[tokio::test]
    async fn test_project_parser() {
        let mut project = Project::new(crate::Shared::default());
    
        project.add_node_plugin("test", |_, _, parser| {
            parser.with_object_type::<Test>();
            parser.with_object_type::<Test2>();
            parser.with_object_type::<Test3>();
        });
    
        tokio::fs::create_dir_all(".test").await.unwrap();
    
        tokio::fs::write(
            ".test/v2v2test.md",
            r#"
            # Test document
    
            This is a test of embedded runmd blocks.
    
            ```runmd
            + .test
            <app/reality.test>
            : .name Hello World 2
            : .file .test/test-1.md
            </reality.test>
            : .name World Hello
            </reality.test2>
            : .name World Hello3
    
            + .test
            <a/reality.test>
            : .name Hello World 3
            : .file .test/test-2.md
            ```
    
            ```runmd
            + .test
            <b/reality.test>
            : .name Hello World 2
            : .file .test/test-3.md
    
            + .test
            <c/reality.test>
            : .name Hello World 3
            : .file .test/test-4.md
            ```
            "#,
        )
        .await
        .unwrap();
    
        let mut _project = project.load_file(".test/v2v2test.md").await.unwrap();
    
        println!("{:#?}", _project.nodes.read().await.keys());
    
        for (k, node) in _project.nodes.write().await.iter_mut() {
            let node = node.read().await;
            println!("{:?}", k);
    
            let attributes = node.resource::<ParsedAttributes>(ResourceKey::root());
    
            if let Some(attributes) = attributes {
                println!("{:#?}", attributes);
    
                for attr in attributes.parsed() {
                    let test = node.resource::<Test>(attr.transmute());
                    println!("{:?}", test);
                    if let Some(test) = test {
                        let fields =
                            crate::visitor::<crate::Shared, PathBuf>(std::ops::Deref::deref(&test));
                        println!("{:#?}", fields);
                        println!(
                            "Find field: {:#?}",
                            crate::FindField::find_field::<()>(&fields, "file")
                        );
                    }
                    let test = node.resource::<Test2>(attr.transmute());
                    println!("{:?}", test);
                }
            }
        }
        ()
    }
}
