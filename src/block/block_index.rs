use std::collections::{BTreeMap, BTreeSet};

use atlier::system::{Attribute, Value};

use crate::BlockProperties;

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
#[derive(Debug, Clone)]
pub struct BlockIndex {
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
            root: root.clone(),
            properties: BlockProperties::default(),
            complexes: BTreeMap::default(),
            children: BTreeMap::default(),
        }
    }

    /// Returns the stable attribute that is the root of these properties
    ///
    pub fn root(&self) -> &Attribute {
        &self.root
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
                                    let mut props = BlockProperties::default();
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

impl From<BTreeMap<String, Value>> for BlockIndex {
    fn from(_: BTreeMap<String, Value>) -> Self {
        todo!()
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
    :: general      .complex name, type
    :: computation  .complex type, factor, enabled
    :: name         .symbol test_block
    :: type         .symbol block_example_1
    :: factor       .int 10

    + test_attr .empty
    :: name .symbol test_block_2
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
