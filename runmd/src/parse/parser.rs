use std::pin::Pin;
use std::ops::DerefMut;
use std::fmt::Debug;
use logos::Logos;
use tracing::error;
use tracing::trace;

use crate::{
    lex::prelude::{Context, Instruction, Line},
    prelude::*,
};

/// Type-alias for a boxed block provider,
///
type BoxedBlockProvider = Pin<Box<dyn BlockProvider + Send + Sync + Unpin>>;

/// Type-alias for a boxed node provider,
///
type BoxedNodeProvider = Pin<Box<dyn NodeProvider + Send + Sync + Unpin>>;

/// Runmd parser,
///
/// **Note** The goal of the parser isn't to store any data, it's goal is to emit instructions after parsing.
///
pub struct Parser {
    /// Provides blocks when parsing runmd blocks,
    ///
    block_provider: BoxedBlockProvider,
    /// Provides nodes when parsing an AddBlock instruction,
    ///
    node_provider: BoxedNodeProvider,
    /// Graph of nodes stored as a flat vector,
    /// 
    graph: Vec<BoxedNode>,
    /// Current node index,
    ///
    current_node_idx: Option<usize>,
}

impl Parser {
    /// Returns a new empty parser w/ block/node provider,
    ///
    pub fn new(
        blocks: impl BlockProvider + Send + Sync + Unpin + 'static,
        nodes: impl NodeProvider + Send + Sync + Unpin + 'static,
    ) -> Self {
        Self {
            block_provider: Box::pin(blocks),
            node_provider: Box::pin(nodes),
            graph: vec![],
            current_node_idx: None,
        }
    }

    /// Parses source runmd input,
    /// 
    /// **Note** Will panic if node provider is unable to provide a node, if an extension is unable to load, or if any other unexpected situation occurs.
    /// 
    pub async fn parse(&mut self, source: impl AsRef<str> + Debug) {
        // Apply Lexer analysis
        let mut lexer = Instruction::lexer_with_extras(source.as_ref(), Context::default());

        let mut locations = vec![];

        while let Some(line) = lexer.next() {
            trace!(line = format!("{:?}", line), "{:<50}", lexer.slice().trim());
            if line.is_err() {
                // This might not always mean there is an issue with parsing
                error!("Lexer error encounterd at -- {:?}: '{}'", lexer.span(), lexer.slice());
            }

            if let Ok(Instruction::AddNode | Instruction::DefineProperty | Instruction::LoadExtension | Instruction::LoadExtensionSuffix) = line {
                locations.push(lexer.span());
            }
        }

        // Process instructions from lexer analysis
        for (idx, mut block) in lexer.extras.blocks.drain(..).enumerate() {
            let block_info = BlockInfo { idx, ty: block.ty, moniker: block.moniker };

            self.on_block(block_info.clone());

            for (idx, line) in block.lines.drain(..).enumerate() {
                let span = locations.pop();
                match line.instruction {
                    Instruction::AddNode => {
                        let node_info = NodeInfo { idx, parent_idx: None, line, span };
                        self.on_add_node(node_info, block_info.clone())
                    }
                    Instruction::DefineProperty => self.on_define_property(line),
                    Instruction::LoadExtension | Instruction::LoadExtensionSuffix => {
                        let node_info = NodeInfo { idx, parent_idx: self.current_node_idx, line, span };
                        self.on_load_extension(node_info, block_info.clone())
                            .await
                    }
                    _ => {
                        unimplemented!("Unimplemented instruction was used")
                    }
                }
            }

            for node in self.graph.drain(..) {
                let node = Pin::into_inner(node);
                node.completed();
            }
        }
    }

    /// Callback when processing a new block,
    ///
    fn on_block(&mut self, block_info: BlockInfo) {
        if let Some(block) = self.block_provider.provide(block_info) {
            self.graph.push(block);
        }
    }

    /// Callback when processing an AddNode instruction,
    ///
    fn on_add_node(
        &mut self,
        node_info: NodeInfo,
        block_info: BlockInfo,
    ) {
        // Reset the current node index
        self.current_node_idx.take();

        // Parse attr on line
        if let Some(ref attr) = node_info.line.attr {
            let node = self.node_provider.provide(
                attr.name,
                node_info.line.tag.as_ref().map(|t| t.0),
                attr.input.clone().map(|i| i.input_str()).as_ref().map(|s| s.as_str()),
                &node_info,
                &block_info
            );
            if let Some(mut node) = node {
                {
                    let node = node.deref_mut();
                    node.set_info(node_info.clone(), block_info);
                }
                self.current_node_idx = Some(node_info.idx);
                self.graph.push(node);
            } else {
                panic!("Could not provide node");
            }
        } else {
            panic!("Missing attribute parameters to add node");
        }
    }

    /// Callback when processing a LoadExtension instruction,
    ///
    async fn on_load_extension<'a>(
        &mut self,
        node_info: NodeInfo<'_>,
        block_info: BlockInfo<'_>,
    ) {
        if let Some(last) = self.graph.last_mut() {
            last.set_info(
                node_info.clone(), 
                block_info
            );
            if let Some(ext) = node_info.line.extension.as_ref() {
                if let Some(mut _ext) = last
                    .load_extension(
                        ext.type_name().as_str(), 
                        ext.tag(), 
                        ext.input.clone().map(|i| i.input_str()).as_ref().map(|s| s.as_str())
                    )
                    .await
                {
                    if let Some(path) = ext.path() {
                        _ext.assign_path(path);
                    }
                    self.graph.push(_ext);
                } else {
                    panic!("Could not load extension");
                }
            }
        } else {
            panic!("No node exists to load an extension with")
        }
    }

    /// Callback when processing a DefineProperty instruction,
    ///
    fn on_define_property(&mut self, line: Line<'_>) {
        if let Some(last) = self.graph.last_mut() {
            if let Some(mut attr) = line.attr {
                last.define_property(
                    attr.name,
                    line.tag.map(|t| t.0),
                    attr.input.take().map(|i| i.input_str()).as_ref().map(|s| s.as_str()),
                )
            } else {
                panic!("Line is missing attribute parameters to define property")
            }
        } else {
            panic!("No node exists to define a property on");
        }
    }
}
