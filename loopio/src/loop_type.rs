/// Trait abstracting the lifecycle of a loop,
/// 
pub trait LoopType 
where
    Self: Sized
{
    /// Called when the loop is initialized,
    /// 
    fn init() -> Self;

    /// Bootstrap configure Self,
    /// 
    fn bootstrap(self, config: fn(Self)->Self) -> Self {
        config(self)
    }

    /// Return true to enable the body to be called,
    /// 
    fn condition(&self) -> bool {
        true
    }

    /// Called if condition(..) returns true,
    /// 
    fn body_mut(&mut self) {}

    /// Called if condition(..) returns true after body_mut,
    /// 
    fn body(&self) {}

    /// Called after the body returns,
    /// 
    fn interval(&mut self) {}

    /// Called when the loop type is exiting,
    /// 
    fn exit(self) -> anyhow::Result<Self> {
        Ok(self)
    }

    /// Calls the body fns,
    /// 
    fn __do(&mut self) {
        self.body_mut();
        self.body();
    }
    
    /// Runs the loop ty as a do while loop,
    /// 
    fn do_while_loop(previous: Option<Self>) -> anyhow::Result<Self> {
        let mut loop_ty = previous.unwrap_or(Self::init());

        loop_ty.__do();

        Self::while_loop(Some(loop_ty))
    }

    /// Runs the loop ty as a while loop,
    /// 
    fn while_loop(previous: Option<Self>) -> anyhow::Result<Self> {
        let mut loop_ty = previous.unwrap_or(Self::init());

        loop {
            if loop_ty.condition() {
                loop_ty.__do();
            } else {
                break;
            }

            loop_ty.interval();
        }

        loop_ty.exit()
    }
}