use std::collections::BTreeMap;

use super::prelude::*;

/// Contains line data derived by the lexer
/// 
#[derive(Hash, Default, Debug, Clone)]
pub struct Line<'a> {
    /// Block ty,
    /// 
    pub block_ty: Option<&'a str>,
    /// Block moniker,
    /// 
    pub block_moniker: Option<&'a str>,
    /// Instruction for this line,
    /// 
    pub instruction: Instruction,
    /// Extension value if provided,
    /// 
    pub extension: Option<Extension<'a>>,
    /// Tag value if provided,
    /// 
    pub tag: Option<Tag<'a>>,
    /// Attribute value,
    /// 
    pub attr: Option<Attribute<'a>>,
    /// Comment value,
    /// 
    pub comment: Option<Vec<&'a str>>,
    /// Stack of documentation headers,
    /// 
    pub doc_headers: Vec<&'a str>,
    /// Properties derived from comments,
    /// 
    pub comment_properties: BTreeMap<String, String>,
}

impl<'a> std::fmt::Display for Line<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.instruction {
            Instruction::BlockStart => {
                match (self.block_ty, self.block_moniker) {
                    (Some(ty), Some(moniker)) => {
                        write!(f, "```runmd {ty} {moniker}")
                    },
                    (Some(ty), None) => {
                        write!(f, "```runmd {ty}")
                    },
                    _ => {
                        write!(f, "```runmd")
                    }
                }
            },
            Instruction::AddNode => {
                match (self.attr.as_ref(), self.tag.as_ref()) {
                    (Some(Attribute { name, input: Some(input) }), Some(tag)) => {
                        write!(f, "+ {} .{name} {}", tag.0, input.clone().input_str(), )
                    },
                    (Some(Attribute { name, input: Some(input) }), None) => {
                        write!(f, "+ .{name} {}", input.clone().input_str())
                    },
                    (Some(Attribute { name, input: None }), Some(tag)) => {
                        write!(f, "+ {} .{name}", tag.0)
                    },
                    (Some(Attribute { name, input: None }), None) => {
                        write!(f, "+ .{name}")
                    },
                    _ => {
                        return write!(f, "BUG -- {:?}", self.attr)
                    }
                }
            },
            Instruction::DefineProperty => {
                match (self.attr.as_ref(), self.tag.as_ref()) {
                    (Some(Attribute { name, input: Some(input) }), Some(tag)) => {
                        write!(f, ": {} .{name} {}", tag.0, input.clone().input_str())
                    },
                    (Some(Attribute { name, input: Some(input) }), None) => {
                        write!(f, ": .{name} {}", input.clone().input_str())
                    },
                    (Some(Attribute { name, input: None }), Some(tag)) => {
                        write!(f, ": {} .{name}", tag.0)
                    },
                    (Some(Attribute { name, input: None }), None) => {
                        write!(f, ": .{name}")
                    },
                    _ => {
                        return write!(f, "BUG -- {:?}", self.attr)
                    }
                }
            },
            Instruction::LoadExtension | Instruction::LoadExtensionSuffix => {
                match self.extension.as_ref() {
                    Some(Extension { tag: Some(tag), name, suffix: Some(suffix), input: Some(input) }) => {
                        write!(f, "<{tag}/{name}.{suffix}> {}", input.clone().input_str())
                    },
                    Some(Extension { tag: Some(tag), name, suffix: None, input: Some(input) }) => {
                        write!(f, "<{tag}/{name}> {}", input.clone().input_str())
                    },
                    Some(Extension { tag: None, name, suffix: Some(suffix), input: Some(input) })=> {
                        write!(f, "<{name}.{suffix}> {}", input.clone().input_str())
                    },
                    Some(Extension { tag: None, name, suffix: None, input: Some(input) })=> {
                        write!(f, "<{name}> {}", input.clone().input_str())
                    },
                    Some(Extension { tag: None, name, suffix: None, input: None })=> {
                        write!(f, "<{name}>")
                    },
                    _ => {
                        return write!(f, "BUG -- {:?}", self.extension)
                    }
                }
            },
            _ => {                        
                return write!(f, "BUG -- {:?}", self)
            }
        }?;

        match self.comment.as_ref() {
            Some(comments) if comments.len() == 1 => {
                write!(f, " {}", comments.last().unwrap_or(&""))
            },
            Some(comments) if comments.len() > 1 => {
                let first = comments.first().unwrap_or(&"");
                writeln!(f, " {}", first)?;
                for c in comments.iter().skip(1) {
                    writeln!(f, "|# {c}")?;
                }
                Ok(())
            },
            _ => {
                Ok(())
            }
        }
    }
}