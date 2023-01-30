use std::collections::{BTreeMap, BTreeSet};

use crate::{Attribute, Value};
use specs::{Component, VecStorage};

use crate::{BlockProperties, BlockProperty};

/// This struct takes a property map, and from each `.complex` value,
/// indexes a subset of the map.
///
/// # Indexing Proccedure
/// 1) Iterate through set of stable attributes
/// 2) Check if the attribute has any properties
/// 3) Find all properties with a .complex value
/// 4) Map the name of property to a map of properties specified by the .complex
///
/// # Lookup Procedure
/// 1) Specify the name of the complex
/// 2) Get a map of name/values
///
#[derive(Debug, Default, Clone, Component, Hash, Eq, PartialEq, PartialOrd)]
#[storage(VecStorage)]
pub struct BlockIndex {
    /// Control values
    control: BTreeMap<String, Value>,
    /// Stable attribute that is the root of this index
    root: Attribute,
    /// Map of block properties
    properties: BlockProperties,
    /// Map of complexes wiithin the properties
    complexes: BTreeMap<String, BTreeSet<String>>,
    /// Child properties 
    /// 
    /// If a propety has a different entity id, that means it belongs to 
    /// a child entity that is related to this block 
    /// 
    children: BTreeMap<u32, BlockProperties>
}

impl BlockIndex {
    /// Creates a new empty index w/ a stable root attribute
    /// 
    pub fn new(root: &Attribute) -> Self {
        assert!(root.is_stable(), "Only a stable attribute can be used as a root");

        BlockIndex {
            control: BTreeMap::default(),
            root: root.clone(),
            properties: BlockProperties::new(root.name()),
            complexes: BTreeMap::default(),
            children: BTreeMap::default(),
        }
    }

    /// Returns the stable attribute that is the root of these properties
    ///
    pub fn root(&self) -> &Attribute {
        &self.root
    }

    /// Searches for a property from the block, and returns the first result,
    /// 
    /// This method will search in the following order,
    /// 
    /// 1) Control map
    /// 2) Root block properties
    /// 
    /// Panics if the property returned is BlockProperty::Required.
    /// 
    pub fn find_property(&self, name: impl AsRef<str>) -> Option<BlockProperty> {
        if let Some(controlled) = self.control.get(name.as_ref()) {
            Some(BlockProperty::Single(controlled.clone()))
        } else {
            match self.properties.property(name.as_ref()) {
                Some(property) => match property {
                    BlockProperty::Single(_) => Some(property.clone()),
                    BlockProperty::List(_) => Some(property.clone()),
                    BlockProperty::Required(_) => panic!("Missing required property {}", name.as_ref()),
                    BlockProperty::Optional(_) => None,
                    BlockProperty::Empty => None,
                },
                None => None,
            }
        }
    }

    /// Returns a reference to current block properties,
    /// 
    pub fn properties(&self) -> &BlockProperties {
        &self.properties
    }

    /// Returns a mutable reference to current block properties,
    /// 
    pub fn properties_mut(&mut self) -> &mut BlockProperties {
        &mut self.properties
    }

    /// Returns an immutable reference to child properties,
    /// 
    pub fn child_properties(&self, child: u32) -> Option<&BlockProperties> {
        self.children.get(&child)
    }

    /// Returns a mutable reference to child properties found in this index
    /// 
    pub fn child_properties_mut(&mut self, child: u32) -> Option<&mut BlockProperties> {
        self.children.get_mut(&child)
    }

    /// Ensure a child properties exist,
    /// 
    pub fn ensure_child(&mut self, child: u32) {
        self.children.insert(child, BlockProperties::default());
    }

    /// Add's a control value to the index, 
    /// 
    pub fn add_control(&mut self, name: impl AsRef<str>, value: impl Into<Value>) {
        self.control.insert(name.as_ref().to_string(), value.into());
    }

    /// Returns control values, 
    /// 
    pub fn control_values(&self) -> &BTreeMap<String, Value> {
        &self.control
    }

    /// Returns mutable reference to control values,
    /// 
    pub fn control_values_mut(&mut self) -> &mut BTreeMap<String, Value> {
        &mut self.control
    }

    /// Returns a set of child block properties that were indexed
    /// 
    /// Since this index is built of a vector of attributes, if a root has
    /// a property that has a different id, that property is added to a set
    /// of child properties mapped to that id.
    /// 
    /// *Note* Only special attribute types can add children in this manner.
    /// 
    pub fn iter_children(&self) -> impl Iterator<Item = (&u32, &BlockProperties)> {
        self.children.iter()
    }

    /// Returns a complex if it exists
    /// 
    pub fn complex(&self, complex_name: impl AsRef<str>) -> Option<&BTreeSet<String>> {
        self.complexes
            .get(complex_name.as_ref())
    }

    /// Finds a complex from the index and returns it's block properties,
    ///
    /// If a property was not present, then a value of Value::Empty will be set.
    ///
    pub fn as_complex(&self, complex_name: impl AsRef<str>) -> Option<BlockProperties> {
        self.complex(complex_name)
            .and_then(|complex| self.properties.complex(complex))
    }

    /// Creates a vector of index results derived from a vector of attributes
    ///
    pub fn index(attributes: impl Into<Vec<Attribute>>) -> Vec<Self> {
        let attributes = attributes.into();
        let mut i = vec![];
        let mut s = vec![];

        /*
        Parses the current state
        */
        let parse = |indexes: &mut Vec<BlockIndex>, span: &mut Vec<usize>| {
            let range = span.clone();
            if let (Some(begin), Some(end)) = (range.get(0), range.get(1)) {
                let slice = &attributes.as_slice()[*begin..*end];
                let stable_attr = slice
                    .get(0)
                    .expect("There should be an owner for these properties");

                let mut block_index = BlockIndex::new(stable_attr);

                for prop in slice[1..].iter() {
                    debug_assert!(prop.name().starts_with(stable_attr.name()));

                    let symbol = prop
                        .name()
                        .trim_start_matches(stable_attr.name())
                        .trim_start_matches("::");

                    let value = prop.transient().expect("exists").1.clone();

                    if let Value::Complex(complex) = value {
                        block_index.complexes.insert(symbol.to_string(), complex);
                    } else {
                        if prop.id() != stable_attr.id() {
                            match block_index.children.get_mut(&prop.id()) {
                                Some(props) => {
                                    props.add(symbol.to_string(), value.clone());
                                },
                                None => {
                                    let mut props = BlockProperties::new(stable_attr.name());
                                    props.add(symbol.to_string(), value.clone());
                                    block_index.children.insert(prop.id(), props);
                                },
                            }
                        } else {
                            block_index
                                .properties
                                .add(symbol.to_string(), value.clone());
                        }
                    }
                }

                indexes.push(block_index);

                span.clear();
                // Move the span forward
                span.push(*end);
            }
        };

        for (pos, attr) in attributes.iter().enumerate() {
            if attr.is_stable() {
                s.push(pos);
                parse(&mut i, &mut s);
            }
        }

        s.push(attributes.len());
        parse(&mut i, &mut s);
        i
    }
}

#[test]
#[tracing_test::traced_test]
fn test_block_index() {
    use tracing::event;
    use tracing::Level;

    let mut parser = crate::Parser::new().parse(
    r#"
    ``` test block
    + test_attr .empty
    : general      .complex name, type
    : computation  .complex type, factor, enabled
    : name         .symbol test_block
    : type         .symbol block_example_1
    : factor       .int 10

    + test_attr .empty
    : name .symbol test_block_2
    ```
    "#,
    );

    parser.evaluate_stack();

    let index = BlockIndex::index(parser.get_block("test", "block"));
    event!(Level::TRACE, "{:#?}", index);

    // Test that complex lookup works
    //
    let index = index.get(0).expect("should be a block index at pos 0");
    let general_complex = index.as_complex("general").expect("should exist");
    assert_eq!(
        general_complex.property("name"),
        Some(&crate::block::BlockProperty::Single(Value::Symbol(
            "test_block".to_string()
        )))
    );
    assert_eq!(
        general_complex.property("type"),
        Some(&crate::block::BlockProperty::Single(Value::Symbol(
            "block_example_1".to_string()
        )))
    );

    let computation_complex = index.as_complex("computation").expect("should exist");
    assert_eq!(
        computation_complex.property("type"),
        Some(&crate::block::BlockProperty::Single(Value::Symbol(
            "block_example_1".to_string()
        )))
    );
    assert_eq!(
        computation_complex.property("factor"),
        Some(&crate::block::BlockProperty::Single(Value::Int(10)))
    );
    assert_eq!(
        computation_complex.property("enabled"),
        Some(&crate::block::BlockProperty::Single(Value::Empty))
    );
}
