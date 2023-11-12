use runmd::prelude::*;
use tracing::trace;

const SOURCE: &'static str = r"
# Test runmd document
+ This is a test document.

```runmd application/test.block root
: test .block-prop hello prop   # Test defining a property on the block

+ .test test/test.node          # Test adding a new node
<application/test.extension>    # Test loading in an extension
: .name-1 hello-world           # Test defining a property
<..extension-2> testinput       # Test loading another extension by suffix
: .name-1 hello-world-2         # Test defining a property
: .name-2 'hello-world-3'       # Test defining a property

+ example .test test/test.node  # Test adding an additional node
<application/test.extension>    # Test loading an extension
<..example>     Hello World     # Test loading an extension by suffix
: .name         cool example    # Test defining a property
```

# Testing an additional block
: shouldn't be used

```runmd .. alt
+ .test test/test.node-2        # Test adding a different block
```
";

#[derive(Debug)]
struct Test;

impl BlockProvider for Test {
    fn provide(&self, block_info: BlockInfo) -> Option<BoxedNode> {
        trace!("provide_block, {:?}", block_info);
        Some(Box::pin(Test))
    }
}

impl NodeProvider for Test {
    fn provide(&self, 
        name: &str, 
        tag: Option<&str>, 
        input: Option<&str>, 
        _node_info: &NodeInfo, 
        _block_info: &BlockInfo
    ) -> Option<BoxedNode> {
        trace!(name, tag, input, "provide_node");
        Some(Box::pin(Test))
    }
}

#[async_trait::async_trait]
impl ExtensionLoader for Test {
    async fn load_extension(&self, extension: &str, tag: Option<&str>, input: Option<&str>) -> Option<BoxedNode> {
        trace!(extension, input, tag, "load_extension");
        Some(Box::pin(Test))
    }
}

impl Node for Test {
    /// Sets the block info for this node,
    ///
    /// Block info details the location within the block this node belongs,
    ///
    fn set_info(
        &mut self,
        node_info: NodeInfo,
        block_info: BlockInfo,
    ) {
        trace!(block_info=format!("{:?}", block_info), "set_block_info\n{:#?}", node_info);
        if let Some(span) = node_info.span {
            trace!("\n\nSOURCE[{:?}] => \n---\n{}---", span.clone(), &SOURCE[span]);
        }
    }

    /// Define a property for this node,
    ///
    fn define_property(&mut self, name: &str, tag: Option<&str>, input: Option<&str>) {
        trace!(name, tag, input, "define_property");
    }

    fn completed(self: Box<Self>) {
        trace!("completed");
    }

    fn assign_path(&mut self, path: String) {
        trace!("assigning path {}", path)
    }
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_parser() {
    let mut parser = Parser::new(Test, Test);

    parser.parse(&SOURCE).await;
}