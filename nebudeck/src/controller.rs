use loopio::engine::Engine;

/// Implemented by interaction types to generalize the steps before compiling the project,
///
pub trait Controller<Bus: ControlBus>
{
    /// Called when the controller should take control over the workspace,
    ///
    fn take_control(self, engine: Engine);
}

/// Trait for allowing controllers to constrain the "super-trait" of the bus delegating control to the controller,
/// 
pub trait ControlBus {
    /// Creates a new instance of this control bus,
    /// 
    fn create(engine: Engine) -> Self;

    /// Delegates control to the given controller,
    /// 
    fn delegate(controller: impl Controller<Self>, engine: Engine) 
    where
        Self: Sized
    {
        controller.take_control(engine)
    }
}