use loopio::engine::Engine;

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
    fn take_control(self, engine: Engine) -> BackgroundWork;
}

/// Trait for allowing controllers to constrain the "super-trait" of the bus delegating control to the controller,
/// 
pub trait ControlBus {
    /// Creates a new instance of this control bus,
    /// 
    fn create(engine: Engine) -> Self;

    /// Delegates control over this type over to a controller,
    /// 
    fn delegate(controller: impl Controller<Self>, engine: Engine) -> BackgroundWork
    where
        Self: Sized
    {
        controller.take_control(engine)
    }
}