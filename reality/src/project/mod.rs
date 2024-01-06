mod extension;
mod host;
mod node;
mod package;
mod program;
mod source;
mod workspace;

use crate::Attribute;
use crate::AttributeParser;
use crate::ResourceKey;
use crate::ResourceKeyHashBuilder;
use crate::Shared;
use crate::StorageTarget;
use async_trait::async_trait;
pub use extension::Transform;
pub use host::RegisterWith;
pub use node::Node;
pub use program::Program;
use runmd::prelude::BlockInfo;
use runmd::prelude::NodeInfo;
use serde::Deserialize;
use serde::Serialize;
pub use source::Source;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
pub use workspace::CurrentDir;
pub use workspace::Dir;
pub use workspace::Empty as EmptyWorkspace;
pub use workspace::Workspace;

pub use self::package::Package;

/// Block plugin fn,
///
pub type BlockPlugin<S> = Arc<dyn Fn(&mut AttributeParser<S>) + Send + Sync + 'static>;

/// Node plugin fn,
///
pub type NodePlugin<S> =
    Arc<dyn Fn(Option<&str>, Option<&str>, &mut AttributeParser<S>) + Send + Sync + 'static>;

/// Type-alias for a table of storages created per node,
///
pub type NodeTable<Storage> =
    BTreeMap<ResourceKey<crate::attributes::Node>, Arc<tokio::sync::RwLock<Storage>>>;

/// Project storing the main runmd parser,
///
pub struct Project<Storage: StorageTarget + 'static> {
    root: Storage,

    pub nodes: tokio::sync::RwLock<NodeTable<Storage::Namespace>>,
}

impl Project<Shared> {
    /// Creates a new project,
    ///
    pub fn new(root: Shared) -> Self {
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
        plugin: impl Fn(&mut AttributeParser<Shared>) + Send + Sync + 'static,
    ) {
        let block_info = BlockInfo {
            idx: 0,
            ty,
            moniker,
        };
        let key = ResourceKey::with_hash(block_info);

        self.root
            .put_resource::<BlockPlugin<Shared>>(Arc::new(plugin), key);
    }

    /// Adds a node plugin,
    ///
    pub fn add_node_plugin(
        &mut self,
        name: &str,
        plugin: impl Fn(Option<&str>, Option<&str>, &mut AttributeParser<Shared>)
            + Send
            + Sync
            + 'static,
    ) {
        let key = ResourceKey::with_hash(name);

        self.root
            .put_resource::<NodePlugin<Shared>>(Arc::new(plugin), key);
    }

    /// Load a file into the project,
    ///
    pub async fn load_file(self, file: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = tokio::fs::read_to_string(file.as_ref()).await?;

        self.load_content(file.as_ref().to_path_buf(), content)
            .await
    }

    /// Load content into the project,
    ///
    pub async fn load_content(
        self,
        relative: impl Into<PathBuf>,
        content: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let mut loading: Loading<Shared> = self.into();

        loading.set_relative(relative);

        let mut parser = runmd::prelude::Parser::new(loading.clone(), loading.clone());

        parser.parse(content.as_ref()).await;

        drop(parser);

        for (_, n) in loading.project.nodes.write().await.iter() {
            let mut n = n.write().await;

            n.drain_dispatch_queues();
        }

        loading.unload()
    }

    /// Creates a package for this project,
    ///
    pub async fn package(&self) -> anyhow::Result<Package> {
        let nodes = self.nodes.read().await;

        let mut programs = vec![];
        for (_, n) in nodes.iter() {
            let node = n.read().await.clone();

            let program = Program::create(node).await?;
            programs.push(program);
        }

        Ok(Package {
            workspace: self
                .root
                .current_resource::<Workspace>(ResourceKey::root())
                .unwrap_or_default(),
            programs,
        })
    }
}

struct Loading<Storage: StorageTarget + Send + Sync + 'static> {
    project: Arc<Project<Storage>>,
    relative: PathBuf,
}

impl<Storage: StorageTarget + Send + Sync + 'static> From<Project<Storage>> for Loading<Storage> {
    fn from(value: Project<Storage>) -> Self {
        Loading {
            project: Arc::new(value),
            relative: PathBuf::new(),
        }
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> Clone for Loading<Storage> {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            relative: self.relative.clone(),
        }
    }
}

impl<Storage: StorageTarget + Send + Sync + 'static> Loading<Storage> {
    /// Sets the relative path value,
    ///
    pub fn set_relative(&mut self, relative: impl Into<PathBuf>) {
        self.relative = relative.into();
    }

    /// Unload the inner project,
    ///
    /// Will return an error if during loading a file something took additional strong references on the project
    ///
    pub fn unload(self) -> anyhow::Result<Project<Storage>> {
        if let Ok(project) = Arc::try_unwrap(self.project) {
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
            parser.parsed_node.node = node;
        }

        // Blocks can have properties and load/unload properties
        if let Some(provider) = self
            .project
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
            .project
            .root
            .current_resource::<NodePlugin<Storage::Namespace>>(node_plugin_key)
        {
            node_plugin(input, tag, parser);
        }
    }
}

impl runmd::prelude::BlockProvider for Loading<Shared> {
    fn provide(&self, block_info: BlockInfo) -> Option<runmd::prelude::BoxedNode> {
        let parser = self.create_parser_for_block(&block_info, None);

        Some(Box::pin(parser))
    }
}

#[async_trait(?Send)]
impl runmd::prelude::NodeProvider for Loading<Shared> {
    async fn provide(
        &self,
        name: &str,
        tag: Option<&str>,
        input: Option<&str>,
        node_info: &NodeInfo,
        block_info: &BlockInfo,
    ) -> Option<runmd::prelude::BoxedNode> {
        let mut key_builder = ResourceKeyHashBuilder::new_default_hasher();
        key_builder.hash(block_info);
        key_builder.hash(node_info);
        let key = key_builder.finish();

        let target = self.project.root.shared_namespace(key);
        let mut parser = self.create_parser_for_block(block_info, Some(key.transmute()));
        parser.set_storage(target.storage);

        // Create and push a new node level
        let mut node = runir::prelude::NodeLevel::new()
            .with_symbol(name)
            .with_doc_headers(node_info.line.doc_headers.clone())
            .with_annotations(node_info.line.comment_properties.clone())
            .with_idx(node_info.idx)
            .with_source_span(node_info.span.as_ref().cloned().unwrap_or_default())
            .with_source_relative(self.relative.clone())
            .with_block(block_info.idx);

        if let Some(input) = input {
            node.set_input(input);
        }
        if let Some(tag) = tag {
            node.set_tag(tag);
        }

        parser.relative = Some(self.relative.clone());
        parser.nodes.push(node);

        self.apply_plugin(name, input, tag, &mut parser);

        if let Some(storage) = parser.clone_storage() {
            let mut nodes = self.project.nodes.write().await;
            nodes.insert(key, storage);
        }

        Some(Box::pin(parser))
    }
}

#[allow(unused)]
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
    #[reality(group = "reality", call = test_call, plugin)]
    pub struct Test {
        pub name: String,
        pub file: PathBuf,
        // #[reality(map_of=String)]
        // pub fields: BTreeMap<String, String>,
    }

    async fn test_noop(_tc: &mut ThunkContext) -> anyhow::Result<()> {
        Ok(())
    }

    #[derive(Debug, Serialize, Default, Deserialize, Clone, Reality)]
    #[reality(group = "reality", call = test_call, plugin)]
    pub struct Test2 {
        name: String,
        file: PathBuf,
        // #[reality(attribute_type=Test3, not_wire)]
        // test3: Test3,
    }

    #[derive(Reality, Default, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
    #[reality(call = test_call, plugin)]
    pub enum Test3 {
        A {
            a: String,
        },
        _B {
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
    use runir::prelude::CrcInterner;
    use runir::prelude::HostLevel;
    use runir::prelude::Linker;
    use runir::prelude::NodeLevel;
    use runir::prelude::Recv;
    use runir::prelude::RecvLevel;
    use runir::prelude::ResourceLevel;

    #[derive(Reality, Serialize, Default, Clone, Debug)]
    #[reality(plugin, call = test_call)]
    pub struct Test4;

    pub async fn test_call(tc: &mut ThunkContext) -> anyhow::Result<()> {
        let test = tc.initialized::<Test>().await;
        eprintln!("test call {:#?}", test);
        Ok(())
    }

    #[test]
    fn test() {
        let _test = Test3::A { a: String::new() };

        let virt = VirtualTest3::new(_test.clone());
        assert!(virt.a.edit_value(|_, a| {
            *a = String::from("hello world");
            true
        }));

        let encoded = Test3::__encode_field_offset_0(virt);
        eprintln!("{:#?}", encoded);
        assert_eq!(
            "hello world",
            &bincode::deserialize::<String>(encoded.wire_data.as_ref().unwrap()).unwrap()
        );

        let decoded =
            Test3::__decode_apply_field_offset_0(VirtualTest3::new(_test), encoded).unwrap();
        decoded.view_value(|v| {
            assert_eq!("hello world", v);
        });
        assert!(decoded.is_pending());
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

    impl FromStr for Test {
        type Err = Infallible;

        fn from_str(_: &str) -> Result<Self, Self::Err> {
            Ok(Test {
                name: String::new(),
                file: PathBuf::from(""),
                // fields: BTreeMap::new(),
            })
        }
    }

    impl FromStr for Test2 {
        type Err = Infallible;

        fn from_str(_: &str) -> Result<Self, Self::Err> {
            Ok(Test2 {
                name: String::new(),
                file: PathBuf::from(""),
                // test3: Test3::A { a: String::new() },
            })
        }
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_project_parser() {
        let mut project = Project::new(crate::Shared::default());

        struct PsuedoTest;

        impl Recv for PsuedoTest {
            fn symbol() -> &'static str {
                "test"
            }
        }

        project.add_node_plugin("test", |_, _, parser| {
            parser.with_object_type::<Thunk<Test>>();
            parser.with_object_type::<Thunk<Test2>>();
            parser.with_object_type::<Thunk<Test3>>();
            parser.push_link_recv::<PsuedoTest>();
        });

        tokio::fs::create_dir_all(".test").await.unwrap();

        tokio::fs::write(
            ".test/v2v2test.md",
            r#"
            # Test document
    
            This is a test of embedded runmd blocks.
    
            ```runmd
            + .test example
            <app/reality.test>
            : .name Hello World 2
            : .file .test/test-1.md
            : message .fields hello world 
            <reality.test>
            : .name World Hello
            <reality.test2>
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

        // TODO: Add asserts
        let mut _project = project.load_file(".test/v2v2test.md").await.unwrap();

        eprintln!("{:#?}", _project.nodes.read().await.keys());

        for (k, node) in _project.nodes.write().await.iter_mut() {
            let _node = node.read().await;

            let parsed_node = _node
                .current_resource::<ParsedNode>(ResourceKey::root())
                .unwrap();

            let mut parsed = parsed_node.to_owned();
            parsed.parse(CrcInterner::default, &_node).await.unwrap();

            eprintln!("{:#}", parsed.node.repr().unwrap());
            eprintln!("-----------------------------------")
        }

        let package = _project.package().await.unwrap();

        let mut matches = package.search("app/reality.test");
        eprintln!("{:#x?}", matches);

        let program = matches.pop().unwrap();

        let node = program.node.unwrap();
        eprintln!("{:?} {:?}", node.span(), node.relative());

        let tc = program.program.context().unwrap();
        let _ = tc.call().await.unwrap();

        ()
    }
}
