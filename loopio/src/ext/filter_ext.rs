//! Extensions for extending plugins by navigating to
//! their thunk context and applying an Extension.

use async_trait::async_trait;
use reality::ThunkContext;

#[async_trait]
pub trait FilterExt {
    /// Filters by path to an associated thunk context w/ an assigned path,
    /// 
    async fn filter_to_path(&self, path: &str) -> Option<ThunkContext>;
}

#[async_trait]
impl FilterExt for ThunkContext {
    /// Filters by path to an associated thunk context w/ an assigned path,
    /// 
    async fn filter_to_path(&self, path: &str) -> Option<ThunkContext> {
        self.navigate(path).await
    }
}