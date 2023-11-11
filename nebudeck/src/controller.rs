use std::collections::BTreeMap;

use loopio::prelude::*;

/// Type-alias for optional background work,
///
/// If `Some`, this indicates that the controller has started work in the **background**.
///
/// If `None`, this indicates that the controller was operating in the **foreground** and no additional work
/// is being done.
///
pub type BackgroundWork = Option<tokio::task::JoinHandle<anyhow::Result<()>>>;

/// Implemented by interaction types to generalize the steps before compiling the project,
///
pub trait Controller<Bus: ControlBus> {
    /// Called when the controller should take control over the workspace,
    ///
    fn take_control(self, bus: Box<Bus>, engine: Engine) -> BackgroundWork;
}

/// Trait for allowing controllers to constrain the "super-trait" of the bus delegating control to the controller,
///
pub trait ControlBus {
    /// Bind an engine handle to this control bus,
    ///
    fn bind(&mut self, engine: EngineHandle);

    /// Delegates control over this type over to a controller,
    ///
    fn delegate(self, controller: impl Controller<Self>, engine: Engine) -> BackgroundWork
    where
        Self: Sized,
    {
        controller.take_control(Box::new(self), engine)
    }
}

/// Generic command type,
///
#[derive(Reality, Clone, Default)]
pub struct Command {
    /// Name of this command,
    /// 
    #[reality(derive_fromstr)]
    pub name: String,
    /// Name of argument and it's description,
    ///
    #[reality(map_of=String)]
    pub arg: BTreeMap<String, String>,
}
