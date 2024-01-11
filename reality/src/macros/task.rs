/// Formats into a closure for a dispatcher queue,
///
/// If the the identifier for the dispatcher is provided, will also queue the closure to the dispatcher.
///
#[macro_export]
macro_rules! task {
    (|$expr:pat_param| => $body:block ) => {
        |$expr| {
            let monad = move || $body;

            Box::pin(monad())
        }
    };
    ($dispatcher:ident $(,)? |$expr:pat_param| => $body:block ) => {
        $dispatcher.queue_dispatch_task(|$expr| {
            let monad = move || $body;

            Box::pin(monad())
        });
    };
}

/// Formats into a closure w/ mutable access for a dispatcher queue,
///
/// If the the identifier for the dispatcher is provided, will also queue the closure to the dispatcher.
///
#[macro_export]
macro_rules! task_mut {
    ( |$expr:pat_param| => $body:block ) => {
        |$expr| {
            let mut monad = move || $body;

            Box::pin(monad())
        }
    };
    ($dispatcher:ident $(,)? |$expr:pat_param| => $body:block ) => {
        $dispatcher.queue_dispatch_mut_task(|$expr| {
            let mut monad = move || $body;

            Box::pin(monad())
        });
    };
}

/// Formats a call_async fn into a thunk_fn closure,
///
/// **Example**
/// ```rs no_run
///
/// async fn call_async(tc: &mut ThunkContext) -> anyhow::Result<()> {
///     Ok(())
/// }
///
/// ..
///
/// thunk_fn!(call_async) // Creates a closure matching a ThunkFn signature
///
/// ```
#[macro_export]
macro_rules! thunk_fn {
    ($call_async:ident) => {
        |tc: ThunkContext| {
            tc.spawn(|mut tc| async move {
                $call_async(&mut tc).await?;
                Ok(tc)
            })
        }
    };
    ($call_async:path) => {
        |tc: ThunkContext| {
            tc.spawn(|mut tc| async move {
                $call_async(&mut tc).await?;
                Ok(tc)
            })
        }
    };
}
