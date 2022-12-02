use std::collections::{BTreeMap, HashSet};

use specs::shred::ResourceId;

use crate::Attributes;

/// Struct for constructing documentation for a block attribute property,
/// 
#[derive(Default, Debug, Clone)]
pub struct Documentation {
    /// Short summary on what the property is for,
    /// 
    pub summary: String, 
    /// Attribute types this property supports, (In priority order)
    ///  
    pub attribute_types: Vec<Attributes>,
    /// Comment about what the property is for, per attribute type
    /// 
    pub comments: BTreeMap<Attributes, String>,
    /// Additional notes,
    /// 
    pub notes: Vec<String>,
    /// Whether this attribute is required for the object to operate,
    /// 
    pub is_required: bool,
    /// Whether this attribute is input to the object, or set by the object,
    /// 
    pub is_input: bool,
    /// Whether this attribute is intended to be a list,
    /// 
    pub is_list: bool,
    /// Whether or not this attribute will have a custom attr parser,
    /// 
    pub is_custom_attr: bool,
    /// Whether this attribute requires a name, 
    /// 
    pub name_required: bool,
    /// Whether a name is optional, 
    /// 
    /// If optional, this implies that a name is used in some way but is set implicitly.
    /// 
    pub name_optional: bool,
    /// These fields are for more advanced scenarios where a custom attribute parser may interact with 
    /// resources from the world. This is to facilitate hot-reloading scenarios.
    /// 
    /// If this is a custom attribute, this is a list of resources that this attribute will create,
    /// 
    pub creates: HashSet<ResourceId>,
    /// If this is a custom attribute, this is a list of resources that this attribute will read,
    /// 
    /// If this list is non-empty, then that implies the attribute parser is not standalone and has dependencies.
    /// Special care would need to be taken if the parser backing this attribute would do any sort of hot reloading.
    /// Ideally, a check would be made to the current world in the parser's scope for existance before applying the parser.
    /// 
    pub reads: HashSet<ResourceId>,
    /// If this is a custom attribute, this is a list of resources that this attribute will modify,
    /// 
    /// If this list is non-empty, then that implies the attribute parser is not standalone and has dependencies.
    /// Special care would need to be taken if the parser backing this attribute would do any sort of hot reloading.
    /// Ideally, a check would be made to the current world in the parser's scope for existance before applying the parser.
    /// 
    pub modifies: HashSet<ResourceId>,
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
    pub fn input(&mut self) -> &mut Self {
        self.is_input = true; 
        self
    }

    /// Sets is_required to true, 
    /// 
    pub fn required(&mut self) -> &mut Self {
        self.is_required = true;
        self 
    }

    /// Sets is_list to true, 
    /// 
    pub fn list(&mut self) -> &mut Self {
        self.is_list = true;
        self
    }

    /// Sets is_custom_attr to true,
    /// 
    pub fn custom_attr(&mut self) -> &mut Self {
        self.is_custom_attr = true;
        self 
    }


    /// Inserts a resource id that the custom attribute will create on parse,
    /// 
    pub fn creates(&mut self, resource_id: ResourceId) -> &mut Self {
        self.creates.insert(resource_id);
        self 
    }

    /// Inserts a resource id that the custom attribute will read on parse,
    /// 
    pub fn reads(&mut self, resource_id: ResourceId) -> &mut Self {
        self.reads.insert(resource_id);
        self
    }

    /// Inserts a resource id that the custom attribute will modify on parse,
    /// 
    pub fn modifies(&mut self, resource_id: ResourceId) -> &mut Self {
        self.modifies.insert(resource_id);
        self 
    }

    /// Sets name_required to true,
    /// 
    /// If true, this means that this property attribute requires a name to be used. 
    /// 
    /// For example,
    /// 
    /// ```norun
    /// : {name} .env {value}
    /// ```
    /// 
    pub fn name_required(&mut self) -> &mut Self {
        self.name_required = true;
        self
    }

    /// Sets name_optional to true,
    /// 
    /// If true, this means that the name is used, but if no name is passed then it is inferred.
    /// 
    /// For example given,
    /// 
    /// ```norun
    /// : .parent {value}
    /// : .child {child}
    /// ```
    /// It is possible the custom attribute `parent` will somehow store {value}, and then the custom attribute `child` will use {value} as the name.
    /// 
    /// This is the case because if a name was strictly required then the above would look like this, 
    /// 
    /// ```norun
    /// : .parent
    /// : {value} .child {child}
    /// ```
    /// 
    /// Since `.parent` is an attribute itself and can hold a value, the former scenario saves a bit of space.
    /// 
    pub fn name_optional(&mut self) -> &mut Self {
        self.name_optional = true;
        self
    }

    /// Adds an additional note to documentation,
    /// 
    pub fn note(&mut self, note: impl AsRef<str>) -> &mut Self {
        self.notes.push(note.as_ref().to_string());
        self
    }

    /// Adds a comment about the property as a symbol,
    /// 
    pub fn symbol(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Symbol, comment)
    }

    /// Adds a comment about the property as a text buffer, 
    /// 
    pub fn text(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Text, comment)
    }

    /// Adds a comment about the property as a binary bector, 
    /// 
    pub fn binary(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::BinaryVector, comment)
    }

    /// Adds a comment about the property as a bool, 
    /// 
    pub fn bool(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Bool, comment)
    }

    /// Adds a comment about the property as a complex, 
    /// 
    pub fn complex(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Complex, comment)
    }

    /// Adds a comment about the property as an integer,
    /// 
    pub fn int(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Int, comment)
    }

    /// Adds a comment about the property as an integer pair,
    /// 
    pub fn int_pair(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::IntPair, comment)
    }

    /// Adds a comment about the property as an integer range,
    /// 
    pub fn int_range(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::IntRange, comment)
    }

       /// Adds a comment about the property as a float,
    /// 
    pub fn float(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::Float, comment)
    }

    /// Adds a comment about the property as an float pair,
    /// 
    pub fn float_pair(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::FloatPair, comment)
    }

    /// Adds a comment about teh property as an float range,
    /// 
    pub fn float_range(&mut self, comment: impl AsRef<str>) -> &mut Self {
        self.with_attribute_type(Attributes::FloatRange, comment)
    }

    /// Adds a comment about a property w/ attribute type,
    /// 
    fn with_attribute_type(&mut self, attribute: Attributes, comment: impl AsRef<str>) -> &mut Self {
        self.attribute_types.push(attribute);
        self.comments.insert(attribute, comment.as_ref().to_string());
        self
    }
}

impl<'a> From<&'a str> for Documentation {
    fn from(value: &'a str) -> Self {
        Self::summary(value)
    }
}

#[test]
fn test_from_static_str() {
    let doc: Documentation = "test doc".into();

    assert_eq!(doc.summary.as_str(), "test doc");
}