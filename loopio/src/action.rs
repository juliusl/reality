use futures_util::{stream, TryStreamExt, StreamExt};
use reality::prelude::*;
use tokio_util::either::Either;

use crate::prelude::Ext;

/// Type-alias for a list of filters,
/// 
type FilterList = Delimitted<',', String>;

/// Action plugin,
/// 
#[derive(Reality, Default, Clone)]
#[reality(call=run_action, plugin, rename = "action")]
pub struct Action {
    /// Name of the action,
    /// 
    #[reality(derive_fromstr)]
    name: String,
    /// Map of filter sequences,
    /// 
    #[reality(option_of=FilterList)]
    filter: Option<FilterList>,
}

async fn run_action(tc: &mut ThunkContext) -> anyhow::Result<()> {
    let init = tc.initialized::<Action>().await;

    /*
    
     */
    // if let Some(engine) = tc.engine_handle().await {
    //     if let Some(mut filter_list) = init.filter {
    //         if let Some(filter) = filter_list.next() {
    //             let mut tc = engine.action_filter(&init.name, &filter).await?;
    //             for filter in filter_list {
    //                 tc.filter(filter);
    //             }
    //         }

    //         for filter in filter_list {
    //         }
    //     }
    // }

    Ok(())
}