use super::prelude::*;

/// Lexer context for storing state while analyzing a document
///
#[derive(Default, Debug, Clone)]
pub struct Context<'a> {
    /// Current instruction being analyzed,
    ///
    instruction: Option<Instruction>,
    /// Current block being analyzed,
    ///
    analyzing: Option<Block<'a>>,
    /// Current extension in context,
    ///
    extension: Option<Extension<'a>>,
    /// Blocks parsed by this context,
    ///
    pub blocks: Vec<Block<'a>>,
}

/// Generates code to check the type of instruction the context is analyzing,
///
macro_rules! is_instruction {
    ($rcv:ident, $ty:ident) => {
        $rcv.instruction
            .as_ref()
            .map(|k| matches!(k, Instruction::$ty))
            .unwrap_or_default()
    };
}

impl<'a> Context<'a> {
    /// Starts analying a block,
    ///
    #[inline]
    pub fn start_block(&mut self) {
        self.analyzing = Some(Block::default());
    }

    /// Ends analyzing the block,
    ///
    #[inline]
    pub fn end_block(&mut self) {
        let block = self.analyzing.take().expect("should have a block");
        self.blocks.push(block);

        // Reset the active extension
        self.extension.take();
    }

    /// Returns true if the context is currently analyzing a block,
    ///
    #[inline]
    pub fn is_analyzing(&self) -> bool {
        self.analyzing.is_some()
    }

    /// Sets the current instruction being analyzed,
    ///
    #[inline]
    pub fn set_instruction(&mut self, instruction: Instruction) {
        self.instruction = Some(instruction);
    }

    /// Sets the current extension,
    ///
    #[inline]
    pub fn set_extension(&mut self, extension: Extension<'a>) {
        self.extension = Some(extension);
    }

    /// Sets the type parameter on the current block being analyzed,
    ///
    #[inline]
    pub fn set_block_ty(&mut self, ty: &'a str) {
        if let Some(block) = self.analyzing.as_mut() {
            block.ty = Some(ty);
        }
    }

    /// Sets the moniker parameter on the current block being analyzed,
    ///
    #[inline]
    pub fn set_block_moniker(&mut self, moniker: &'a str) {
        if let Some(block) = self.analyzing.as_mut() {
            block.moniker = Some(moniker);
        }
    }

    /// Returns the current extension,
    ///
    #[inline]
    pub fn get_extension(&self, suffix: Option<&'a str>) -> Option<Extension<'a>> {
        self.extension.clone().map(|mut e| {
            e.suffix = suffix;
            e
        })
    }

    /// Adds a line to the current block,
    ///
    pub fn add_line(&mut self, mut line: Line<'a>) {
        if (self.is_instruction_load_extension() || self.is_instruction_load_extension_suffix())
            && line.extension.is_some()
        {
            let next = line.extension.clone().unwrap();
            self.set_extension(next);
        } else if self.is_instruction_add_node() {
            self.extension.take();
        }

        if let Some(block) = self.analyzing.as_mut() {
            line.extension = self.extension.clone();
            line.instruction = self.instruction.take().unwrap_or_default();
            block.lines.push(line);
        }
    }

    /// Append input to the last line if applicable,
    /// 
    pub fn append_input(&mut self, input: &'a str) {
        if let Some(line) = self.analyzing.as_mut().and_then(|b| b.lines.last_mut()) {
            if let Some(_input) = line
                .attr
                .as_mut()
                .and_then(|a| a.input.as_mut())
                .or(line.extension.as_mut().and_then(|e| e.input.as_mut()))
            {
                match _input {
                    Input::EscapedText(t) | Input::Text(t) => {
                        *_input = Input::Lines(vec![t, input])
                    }
                    Input::Lines(lines) => {
                        lines.push(input);
                    }
                }
            }
        }
    }

    /// Returns true if the current instruction is AddNode,
    ///
    #[inline]
    pub fn is_instruction_add_node(&self) -> bool {
        is_instruction!(self, AddNode)
    }

    /// Returns true if the current instruction is DefineProperty,
    ///
    #[inline]
    #[allow(dead_code)]
    pub fn is_instruction_define_property(&self) -> bool {
        is_instruction!(self, DefineProperty)
    }

    /// Returns ture if the current instruction is LoadExtension,
    ///
    #[inline]
    pub fn is_instruction_load_extension(&self) -> bool {
        is_instruction!(self, LoadExtension)
    }

    /// Returns true if the current instruction is LoadExtensionSuffix
    ///
    #[inline]
    pub fn is_instruction_load_extension_suffix(&self) -> bool {
        is_instruction!(self, LoadExtensionSuffix)
    }
}
