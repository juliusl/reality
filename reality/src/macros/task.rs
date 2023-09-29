#[macro_export]
macro_rules! task {
    ( |$expr:pat_param| => $body:block )=> {
        |$expr| { 
            let monad = move || $body;
            
            Box::pin(monad())
        }
    };
}

#[macro_export]
macro_rules! task_mut {
    ( |$expr:pat_param| => $body:block )=> {
        |$expr| { 
            let mut monad = move || $body;
            
            Box::pin(monad())
        }
    };
}