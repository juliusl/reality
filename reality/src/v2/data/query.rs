use crate::state::iter_state;
use crate::v2::compiler::Compiled;
use crate::v2::Object;
use crate::v2::Properties;
use crate::Error;
use crate::Identifier;
use std::collections::BTreeMap;

/// Type-alias for query result,
///
pub type QueryResult = (Identifier, BTreeMap<String, String>, Properties);

/// Type-alias for query iterator,
///
pub type QueryIter<'a> = Box<dyn Iterator<Item = QueryResult> + 'a>;

/// Trait to filter query results,
/// 
pub trait Predicate 
where
    Self: Send + Sync + Copy,
{
    /// Return true to include in query results,
    /// 
    fn filter(
        self,
        ident: &Identifier,
        rv_interpolated_ident_map: &BTreeMap<String, String>, // Reverse interpolated identifier map
        properties: &Properties,
    ) -> bool;
}

impl<F> Predicate for F
where
    F: Fn(&Identifier, &BTreeMap<String, String>, &Properties) -> bool + Send + Sync + Copy,
{
    fn filter(
        self,
        ident: &Identifier,
        interpolated: &BTreeMap<String, String>,
        properties: &Properties,
    ) -> bool {
        self(ident, interpolated, properties)
    }
}

/// Query predicate that returns all results,
/// 
#[allow(dead_code)]
pub fn all(_: &Identifier, _: &BTreeMap<String, String>, _: &Properties) -> bool { true }

/// Trait providing query functionality over object state,
///
pub trait Query<'a> {
    /// Returns an iterator w/ query results,
    ///
    /// Note: Results are selected based on objects matching a string interpolation pattern, and further filtered by a
    /// predicate function.
    ///
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Predicate + 'static,
    ) -> Result<QueryIter<'a>, Error>;

    /// Returns an iterator w/ all query results,
    /// 
    fn all(&'a self, pat: impl AsRef<str>) -> Result<QueryIter<'a>, Error> {
        self.query(pat, all)
    }
}

impl<'a> Query<'a> for Properties {
    fn query(
        &'a self,
        pat: impl AsRef<str>,
        predicate: impl Predicate + 'static,
    ) -> Result<QueryIter<'a>, Error> {
        let ident = self.owner().commit()?;

        if let Some(map) = ident
            .interpolate(pat.as_ref())
            .filter(|map| predicate.filter(&ident, map, self))
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
        predicate: impl Predicate + 'static,
    ) -> Result<QueryIter<'a>, Error> {
        let pat = pat.as_ref().to_string();

        Ok(Box::new(iter_state::<Object, _>(self).filter_map(
            move |(_, o)| {
                if let Ok(mut result) = o.properties().query(&pat, predicate) {
                    result.next()
                } else {
                    None
                }
            },
        )))
    }
}
