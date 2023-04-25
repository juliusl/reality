use std::marker::PhantomData;

use specs::Component;

use crate::v2::Visitor;

use super::BuildLog;
use super::DispatchRef;

/// Linker links a visitor w/ a dispatch ref,
///
pub struct Linker<'a, T> 
where
    T: Visitor + Clone + Component + Send + Sync,
{
    /// The configuration to use when creating a new instance of T,
    /// 
    new: T,
    dispatch: Option<DispatchRef<'a, T>>,
    build_log: BuildLog,
}

impl<'a, T> Linker<'a, T>
where
    T: Visitor + Clone + Component + Send + Sync,
{
    /// Creates a new empty linker w/ build log,
    /// 
    pub fn new(new: T, build_log: BuildLog) -> Self {
        Self { new, dispatch: None, build_log }
    }

    /// Activates the linker w/ a dispatch ref,
    /// 
    pub fn activate(self, dispatch: DispatchRef<'a, T>) -> Self {
        Self { new: self.new, dispatch: Some(dispatch), build_log: self.build_log }
    }
}


impl<'a, T> Visitor for Linker<'a, T> 
where
    T: Visitor + Clone + Component + Send + Sync,
{
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

    fn visit_extension(&mut self, entity: crate::v2::EntityVisitor, identifier: &crate::Identifier) {
        self.dispatch = self.dispatch.take().map(|d| {
            d.write(|v| {
                v.visit_extension(entity, identifier);
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
