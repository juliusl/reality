use super::{DispatchRef, Properties};

pub trait Bootstrap {
    /// Bootstraps components to dispatch ref,
    /// 
    fn bootstrap<'a>(dispatch_ref: DispatchRef<'a, Properties>) -> DispatchRef<'a, Properties>;
}