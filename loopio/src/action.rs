use std::pin::Pin;

use anyhow::anyhow;
use futures_util::Future;
use reality::HostedResource;
use reality::ResourceKey;
use reality::CallOutput;
use reality::CallAsync;
use reality::SpawnResult;
use reality::ThunkContext;
use reality::attributes;

/// Common trait for engine node types,
///
pub trait Action {
    /// Return the address of an action,
    ///
    fn address(&self) -> String;

    /// Bind a thunk context to the action,
    ///
    /// **Note** This context has access to the compiled node this action corresponds to.
    ///
    fn bind(&mut self, context: ThunkContext);

    /// Binds the node attribute's resource key to this action,
    /// 
    fn bind_node(&mut self, node: ResourceKey<attributes::Node>);

    /// Binds a plugin to this action's plugin resource key,
    /// 
    /// **Note** If not set, then the default is the default plugin key.
    /// 
    fn bind_plugin(&mut self, plugin: ResourceKey<reality::attributes::Attribute>);

    /// Returns the bound node resource key for this action,
    /// 
    fn node_rk(&self) -> ResourceKey<attributes::Node>;

    /// Returns the plugin fn resource key for this action,
    /// 
    fn plugin_rk(&self) -> ResourceKey<attributes::Attribute>;

    /// Returns the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context(&self) -> &ThunkContext;

    /// Returns a mutable reference to the current context,
    ///
    /// **Note** Should panic if currently unbound,
    ///
    fn context_mut(&mut self) -> &mut ThunkContext;

    /// Spawns the thunk attached to the current context for this action,
    /// 
    fn spawn(&self) -> SpawnResult
    where
        Self: CallAsync
    {
        self.context().spawn(|mut tc| async move {
            <Self as CallAsync>::call(&mut tc).await?;
            Ok(tc)
        })
    }

    /// Returns a future that contains the result of the action,
    /// 
    fn spawn_call(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<ThunkContext>> + Send + '_>> 
    where
        Self: Sync
    {
        Box::pin(async move {
            let r = self.into_hosted_resource();
            if let Some(s) = r.spawn() {
                if let Ok(s) = s.await {
                    s
                } else {
                    Err(anyhow!("Task could not join"))
                }
            } else {
                Err(anyhow!("Did not spawn a a task"))
            }
        })
    }

    /// Convert the action into a generic hosted resource,
    /// 
    fn into_hosted_resource(&self) -> HostedResource {
        HostedResource { 
            address: self.address(),
            node_rk: self.node_rk(),
            rk: self.context().attribute,
            decoration: self.context().decoration.clone(), 
            binding: Some(self.context().clone()), 
        }
    }

    /// Converts a pointer to the hosted resource into call output,
    /// 
    fn into_call_output(&self) -> CallOutput {
        CallOutput::Spawn(
            self.into_hosted_resource().spawn()
        )
    }
}
