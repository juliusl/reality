use crate::v2::Runmd;
use crate::v2::Visitor;

use super::BuildLog;
use super::DispatchRef;

/// Linker takes a Component and scans the build_log for candidates that the Component must visit,
/// 
pub struct Linker<'a, T> 
where
    T: Runmd,
{
    /// The configuration to use when creating a new instance of T,
    /// 
    new: T,
    /// Build log w/ mapping from identifiers <-> Entity ID in storage,
    /// 
    build_log: BuildLog,
    /// The dispatch ref w/ access to storage,
    /// 
    dispatch: Option<DispatchRef<'a, T>>,
}

impl<'a, T> Linker<'a, T>
where
    T: Runmd,
{
    /// Creates a new empty linker w/ build log,
    /// 
    pub fn new(new: T, build_log: BuildLog) -> Self {
        Self { new, dispatch: None, build_log }
    }

    /// Activates the linker by providing a dispatch ref to storage,
    /// 
    pub fn activate(self, dispatch: DispatchRef<'a, T>) -> Self {
        Self { 
            new: self.new, 
            dispatch: Some(dispatch), 
            build_log: self.build_log 
        }
    }
}


impl<'a, T> Visitor for Linker<'a, T> 
where
    T: Runmd,
{
    fn visit_object(&mut self, object: &super::Object) {
        object.as_block().map(|b| {
            for ex in b.extensions() {
                
            }
        });
    }

    /// Visiting the identifier will update the current entity in the dispatch ref,
    /// 
    fn visit_identifier(&mut self, identifier: &crate::Identifier) {
        if let Some(e) = self.build_log.try_get(identifier).ok() {
            self.dispatch = self
                .dispatch
                .take()
                .map(|d| { 
                    d.with_entity(e).map(|_| Ok(identifier.clone()))
                });
        }
    }

    fn visit_extension(&mut self, identifier: &crate::Identifier) {
        self.dispatch = self.dispatch.take().map(|d| {
            d.write(|v| {
                v.visit_extension(identifier);
                Ok(())
            })
        })
    }

    fn visit_property(&mut self, name: &String, property: &crate::v2::Property) {
        self.dispatch = self.dispatch.take().map(|d| {
            d.write(|v| {
                v.visit_property(name, property);
                Ok(())
            })
        })
    }
}
