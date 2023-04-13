use crate::v2::compiler::DispatchRef;
use crate::v2::{BuildLog, Properties};
use crate::Result;
use async_trait::async_trait;
use reality_derive::dispatch_signature;
use std::sync::Arc;

dispatch_signature! {
    /// Dispatch signatures,
    ///
    pub enum DispatchSignature {
        #[interpolate("!#block#.#root#.(config);")]
        RootConfig,
        /// Dispatch would map to RootConfigExt signature --> .plugin.#root#.(ext),
        ///
        #[interpolate("!#block#.#root#.(config).(ext);")]
        RootConfigExt,
        /// Signature of an indivisual property for configuring an extension of an extended property,
        /// 
        #[interpolate("!#block#.#root#.(config).(ext).(prop);")]
        ExtendedPropertyConfig,
        /// Signature of an extended property,
        /// 
        #[interpolate("#root#.(config).(name).(extension).(property);")]
        ExtendedProperty,
        /// Given,
        ///
        /// ```
        /// struct Example {
        /// ...
        /// }
        ///
        /// impl Example {
        ///     fn test(&self) -> Result<(), Error> {
        ///         ...
        ///     }
        /// }
        /// ```
        ///
        /// Dispatch would map to fn test() to BlockRootExt signature --> #block#.#root#.example.test,
        ///
        #[interpolate("#block#.#root#.(root).(ext);")]
        BlockRootExt,
        /// Dispatch would map BlockRootConfigExtNameProp signature -->
        ///
        #[interpolate("#block#.#root#.(root).(config).(ext).(name).(?prop)")]
        BlockRootConfigExtNameProp,
    }
}

/// Trait to run async code and then dispatch actions to a world,
///
#[async_trait]
pub trait AsyncDispatch
where
    Self: Send + Sync,
{
    /// Async dispatch fn,
    ///
    async fn async_dispatch<'a, 'b>(
        &'a self,
        build_ref: DispatchRef<'b, Properties>,
    ) -> DispatchResult<'b>;
}

/// Implementation to use as a Thunk component,
///
#[async_trait]
impl AsyncDispatch for Arc<dyn AsyncDispatch> {
    async fn async_dispatch<'a, 'b>(
        &'a self,
        build_ref: DispatchRef<'b, Properties>,
    ) -> DispatchResult<'b> {
        self.as_ref().async_dispatch(build_ref).await
    }
}

/// Type alias for a dispatch result,
///
pub type DispatchResult<'a> = Result<DispatchRef<'a, Properties>>;

/// Trait to dispatch changes to a world,
///
pub trait Dispatch
where
    Self: Send + Sync,
{
    /// Compiles the build ref and returns a result containing the build ref,
    ///
    fn dispatch<'a>(&self, dispatch_ref: DispatchRef<'a, Properties>) -> DispatchResult<'a>;
}

#[allow(unused_imports)]
mod tests {
    use crate::v2::prelude::*;

    #[tracing_test::traced_test]
    #[test]
    fn test_dispatch_signature() {
        let test =  r##".plugin.process.#root#.path.redirect"##.parse::<Identifier>().unwrap();
        println!("{:?}", DispatchSignature::get_match(&test));
    }
}