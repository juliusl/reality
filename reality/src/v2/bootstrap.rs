use super::{DispatchRef, Properties};

/// Trait for bootstrapping types to a dispatch_ref, used by proc_macros,
/// 
pub trait Bootstrap {
    /// Bootstraps components to dispatch ref,
    /// 
    fn bootstrap<'a>(dispatch_ref: DispatchRef<'a, Properties>) -> DispatchRef<'a, Properties>;
}