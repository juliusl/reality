use std::collections::BTreeMap;
use crate::Error;
use crate::state::iter_state;
use crate::Identifier;
use crate::v2::Object;
use crate::v2::compiler::Compiled;
use crate::v2::Properties;

/// Type-alias for query result,
///
pub type QueryResult = (Identifier, BTreeMap<String, String>, Properties);

/// Type-alias for query iterator,
/// 
pub type QueryIter<'a> = Box<dyn Iterator<Item = QueryResult> + 'a>;

/// Trait providing query functionality over object state,
///
pub trait Query<'a>{
    /// Returns an iterator w/ query results,
    ///
    /// Note: Results are selected based on objects matching a string interpolation pattern, and further filtered by a
    /// predicate function.
    ///
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Fn(&Identifier, &BTreeMap<String, String>, &Properties) -> bool + Clone + 'static,
    ) -> Result<QueryIter<'a>, Error>;
}

impl<'a> Query<'a> for Properties {
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Fn(&Identifier, &BTreeMap<String, String>, &Properties) -> bool + Clone + 'static,
    ) -> Result<QueryIter<'a>, Error> {
        let ident = self.owner().commit()?;

        if let Some(map) = ident
            .interpolate(pat.as_ref())
            .filter(|map| predicate(&ident, map, self))
        {
            Ok(Box::new(core::iter::once((ident, map, self.clone()))))
        } else {
            Ok(Box::new(core::iter::empty()))
        }
    }
}

impl<'a> Query<'a> for Compiled<'a> {
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Fn(&Identifier, &BTreeMap<String, String>, &Properties) -> bool + Clone + 'static,
    ) -> Result<QueryIter<'a>, Error> {
        let pat = pat.as_ref().to_string();

        Ok(Box::new(
            iter_state::<Object, _>(self)
                .filter_map(move |(_, o)| {
                    if let Ok(mut result) = o.properties().query(&pat, predicate.clone()) {
                        result.next()
                    } else {
                        None
                    }
                }),
        ))
    }
}
