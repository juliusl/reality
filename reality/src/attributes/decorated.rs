use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::str::FromStr;

use crate::CacheExt;
use crate::ParsedNode;
use crate::Property;
use crate::ResourceKey;
use crate::ThunkContext;

/// Wrapper struct to include a tag w/ a parsed value,
///
#[derive(Hash, Debug, Serialize, PartialEq, Eq, Deserialize, Default)]
pub struct Decorated<T: Send + Sync + 'static> {
    /// Inner value,
    ///
    pub value: Option<T>,
    /// Tag value,
    ///
    pub tag: Option<String>,
    /// If set, this is the property_key generated on parse,
    ///
    pub property: Option<ResourceKey<Property>>,
}

impl<T: Send + Sync + 'static> Decorated<T> {
    /// Returns the default value for this tag,
    ///
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns the value of this tag,
    ///
    pub fn tag(&self) -> Option<&String> {
        self.tag.as_ref()
    }

    /// Returns true if this container matches the provided tagged value,
    ///
    pub fn is_tag(&self, tag: impl AsRef<str>) -> bool {
        self.tag.as_ref().filter(|t| *t == tag.as_ref()).is_some()
    }

    /// Sets a tag on this container,
    ///
    pub fn set_tag(&mut self, tag: impl Into<String>) {
        self.tag = Some(tag.into());
    }

    /// Sets the property on this container,
    ///
    pub fn set_property(&mut self, key: ResourceKey<Property>) {
        self.property = Some(key);
    }

    /// Sync the state w/ a context,
    ///
    pub fn sync(&mut self, _tc: &ThunkContext) {
        if let Some(prop) = self.property.as_ref() {
            if let Some(node) = _tc.cached_ref::<ParsedNode>() {
                self.property = node
                    .properties
                    .iter()
                    .find(|p| p.key() == prop.key())
                    .cloned();
            }
        }
    }

    /// Finds a property in decorations,
    ///
    pub fn property(&self, name: impl AsRef<str>) -> Option<String> {
        self.property
            .as_ref()
            .and_then(|p| p.node())
            .and_then(|p| p.try_annotations())
            .and_then(|d| d.get(name.as_ref()).cloned())
    }
}

impl<T: FromStr + Send + Sync + 'static> FromStr for Decorated<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = T::from_str(s)?;
        Ok(Decorated {
            value: Some(value),
            tag: None,
            property: None,
        })
    }
}

impl<T: PartialOrd + Send + Sync + 'static> PartialOrd for Decorated<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.value.partial_cmp(&other.value) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.tag.partial_cmp(&other.tag)
    }
}

impl<T: Ord + Send + Sync + 'static> Ord for Decorated<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.value.as_ref(), self.tag.as_ref()).cmp(&(other.value.as_ref(), other.tag.as_ref()))
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for Decorated<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            tag: self.tag.clone(),
            property: self.property,
        }
    }
}

/// Type-alias for a Delimitted<',', String>,
///
pub type CommaSeperatedStrings = Delimitted<',', String>;

#[derive(Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Delimitted<const DELIM: char, T: FromStr + Send + Sync + 'static> {
    value: String,
    cursor: usize,
    #[serde(skip)]
    _t: PhantomData<T>,
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Clone for Delimitted<DELIM, T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            cursor: self.cursor,
            _t: PhantomData,
        }
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Debug for Delimitted<DELIM, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Delimitted")
            .field("value", &self.value)
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> FromStr for Delimitted<DELIM, T> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Delimitted {
            value: s.to_string(),
            cursor: 0,
            _t: PhantomData,
        })
    }
}

impl<const DELIM: char, T: FromStr + Send + Sync + 'static> Iterator for Delimitted<DELIM, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut value = self.value.split(DELIM).skip(self.cursor);
        self.cursor += 1;
        value.next().and_then(|v| T::from_str(v.trim()).ok())
    }
}

// TODO: Enabling this would allow for more multi-vector use-cases.
// /// Struct that contains the parsed inner type as well as the idx
// /// of the value.
// ///
// pub struct Indexed<T>
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     idx: usize,
//     val: T,
// }

// impl<T> Indexed<T>
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     /// Returns this struct w/ index set,
//     ///
//     pub fn with_index(mut self, idx: usize) -> Self {
//         self.idx = idx;
//         self
//     }
// }

// impl<T> FromStr for Indexed<T>
// where
//     T: FromStr<Err = anyhow::Error> + Send + Sync + 'static,
// {
//     type Err = anyhow::Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(Self { val: T::from_str(s)?, idx: 0 })
//     }
// }

#[allow(unused)]
mod tests {
    use runir::prelude::Recv;
    use tokio::runtime::Handle;

    use crate::prelude::*;

    use crate::Decorated;

    #[tokio::test]
    async fn test_decorated() {
        struct PsuedoTest;

        impl Recv for PsuedoTest {
            fn symbol() -> &'static str {
                "test"
            }
        }

        #[derive(Reality, Clone, Default, Debug)]
        #[reality(call=test, plugin)]
        struct DecoratedTest {
            #[reality(derive_fromstr)]
            marker: String,
            name: Decorated<String>,
        }
        async fn test(tc: &mut ThunkContext) -> anyhow::Result<()> {
            let mut test = Remote.create::<DecoratedTest>(tc).await;
            assert_eq!("test", test.marker);

            let desc = test.name.property("description");
            assert_eq!("Testing decorated", desc.unwrap().as_str());

            eprintln!("{:#?}", test);
            Ok(())
        }

        let mut project = Project::new(Shared::default());

        project.add_node_plugin("test", |_, _, parser| {
            parser.with_object_type::<Thunk<DecoratedTest>>();
            parser.push_link_recv::<PsuedoTest>();
        });

        let project = project
            .load_content(
                r#"
        ```runmd
        + .test
        <test/reality.decoratedtest> test
        : .name hello-world
        |# description = Testing decorated 
        ```
        "#,
            )
            .await
            .unwrap();

        if let Some((_, node)) = project.nodes.into_inner().pop_first() {
            let target = AsyncStorageTarget::from_parts(node, Handle::current());

            let mut tc: ThunkContext = target.into();

            let _node = tc.node().await;
            let mut node = _node
                .current_resource::<ParsedNode>(ResourceKey::root())
                .unwrap();

            node.upgrade_node(&_node).await.unwrap();
            drop(_node);

            eprintln!("{:#?}", node);

            let attr = node.attributes.first().unwrap();
            tc.set_attribute(*attr);

            tc.call().await.unwrap();
        }
        ()
    }
}
