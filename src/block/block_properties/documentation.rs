use std::collections::BTreeMap;

use crate::Attributes;


/// Struct for constructing documentation for a block property,
/// 
#[derive(Default, Debug, Clone)]
pub struct Documentation {
    /// Short summary on what the property is for,
    /// 
    summary: String, 
    /// Attribute types this property supports, (In priority order)
    ///  
    attribute_types: Vec<Attributes>,
    /// Comment about what the property is for, per attribute type
    /// 
    comments: BTreeMap<Attributes, String>,
    /// Additional notes,
    /// 
    notes: Vec<String>,
    /// Whether this property is required for the object to operate,
    /// 
    is_required: bool,
    /// Whether this property is input to the object, or set by the object,
    /// 
    is_input: bool,
    /// Whether this property is intended to be a list,
    /// 
    is_list: bool,
    /// Whether or not this property will have a custom attr parser,
    /// 
    is_custom_attr: bool,
}

/// API for constructing documentation, uses method chaining style, 
/// 
impl Documentation {
    /// Starts a new property documentation,
    /// 
    pub fn summary(summary: impl AsRef<str>) -> Self {
        let mut new_doc = Documentation::default();
        new_doc.summary = summary.as_ref().to_string();
        new_doc
    }

    /// Sets is_input to true,
    ///
    pub fn input(mut self) -> Self {
        self.is_input = true; 
        self
    }

    /// Sets is_required to true, 
    /// 
    pub fn required(mut self) -> Self {
        self.is_required = true;
        self 
    }

    /// Sets is_list to true, 
    /// 
    pub fn list(mut self) -> Self {
        self.is_list = true;
        self
    }

    /// Sets is_custom_attr to true,
    /// 
    pub fn custom_attr(mut self) -> Self {
        self.is_custom_attr = true;
        self 
    }

    /// Adds an additional note to documentation,
    /// 
    pub fn note(mut self, note: impl AsRef<str>) -> Self {
        self.notes.push(note.as_ref().to_string());
        self
    }

    /// Adds a comment about the property as a symbol,
    /// 
    pub fn symbol(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Symbol, comment)
    }

    /// Adds a comment about the property as a text buffer, 
    /// 
    pub fn text(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Text, comment)
    }

    /// Adds a comment about the property as a binary bector, 
    /// 
    pub fn binary(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::BinaryVector, comment)
    }

    /// Adds a comment about the property as a bool, 
    /// 
    pub fn bool(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Bool, comment)
    }

    /// Adds a comment about the property as a complex, 
    /// 
    pub fn complex(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Complex, comment)
    }

    /// Adds a comment about the property as an integer,
    /// 
    pub fn int(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Int, comment)
    }

    /// Adds a comment about the property as an integer pair,
    /// 
    pub fn int_pair(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::IntPair, comment)
    }

    /// Adds a comment about teh property as an integer range,
    /// 
    pub fn int_range(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::IntRange, comment)
    }

       /// Adds a comment about the property as a float,
    /// 
    pub fn float(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::Float, comment)
    }

    /// Adds a comment about the property as an float pair,
    /// 
    pub fn float_pair(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::FloatPair, comment)
    }

    /// Adds a comment about teh property as an float range,
    /// 
    pub fn float_range(self, comment: impl AsRef<str>) -> Self {
        self.with_attribute_type(Attributes::FloatRange, comment)
    }

    /// Adds a comment about a property w/ attribute type,
    /// 
    fn with_attribute_type(mut self, attribute: Attributes, comment: impl AsRef<str>) -> Self {
        self.attribute_types.push(attribute);
        self.comments.insert(attribute, comment.as_ref().to_string());
        self
    }
}
