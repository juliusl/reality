use std::collections::{BTreeMap, BTreeSet};

use atlier::system::{Value, Attribute};

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
#[derive(Debug)]
pub struct BlockIndex {
    /// Stable attribute
    attr: (String, Value),
    /// Property map
    properties: BTreeMap<String, Value>,
    /// Map of complexes wiithin the properties
    complexes: BTreeMap<String, BTreeSet<String>>
}

impl BlockIndex {
    /// Returns the stable attribute that is the owner of these properties
    /// 
    pub fn attribute(&self) -> (String, Value) {
        self.attr.clone()
    }

    /// Finds a complex from the index and returns a btree map, 
    /// 
    /// If a property was not present, then a value of Value::Empty will be set.
    /// 
    pub fn complex(&self, complex_name: impl AsRef<str>) -> Option<BTreeMap<String, Value>> {
       self.complexes.get(complex_name.as_ref()).and_then(|complex| {
            let mut map = BTreeMap::default();

            for k in complex.iter() {
                if let Some(value) = self.properties.get(k) {
                    map.insert(k.clone(), value.clone());
                } else {
                    map.insert(k.clone(), Value::Empty);
                }
            }

            Some(map)
        })
    }

    /// Indexes a block and returns the indexes that were discovered
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
                let stable_attr = slice.get(0).expect("There should be an owner for these properties");
                
                // TODO: Make this a stack
                let mut block_index = BlockIndex {
                    attr: (stable_attr.name.to_string(), stable_attr.value.clone()),
                    properties: BTreeMap::default(),
                    complexes: BTreeMap::default(),
                };

                for prop in slice[1..].iter() {
                    debug_assert!(prop.name().starts_with(stable_attr.name()));
                
                    let symbol = prop.name()
                        .trim_start_matches(stable_attr.name()).trim_start_matches("::");

                    let value = prop.transient().expect("exists").1.clone();
             
                    if let Value::Complex(complex) = value {
                        block_index.complexes.insert(symbol.to_string(), complex);
                    } else {
                        block_index.properties.insert(symbol.to_string(), value.clone());
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

    let mut parser = crate::Parser::new().parse(r#"
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
    "#);

    parser.evaluate_stack();

    let index = BlockIndex::index(parser.get_block("test", "block"));
    event!(Level::TRACE, "{:#?}", index);

    // Test that complex lookup works 
    // 
    let index = index.get(0).expect("should be a block index at pos 0");
    let general_complex = index.complex("general").expect("should exist");
    assert_eq!(general_complex.get("name"), Some(&Value::Symbol("test_block".to_string())));
    assert_eq!(general_complex.get("type"), Some(&Value::Symbol("block_example_1".to_string())));

    let computation_complex = index.complex("computation").expect("should exist");
    assert_eq!(computation_complex.get("type"), Some(&Value::Symbol("block_example_1".to_string())));    
    assert_eq!(computation_complex.get("factor"), Some(&Value::Int(10)));   
    assert_eq!(computation_complex.get("enabled"), Some(&Value::Empty));
}