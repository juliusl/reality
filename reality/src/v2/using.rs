use super::{DispatchRef, Properties};

pub trait Using {
    /// Maps components on a dispatch_ref,
    /// 
    fn using<'a>(dispatch_ref: DispatchRef<'a, Properties>) -> DispatchRef<'a, Properties>;
}