#[macro_export]
macro_rules! cfg_specs {
    ($($item:expr;)*) => {
        $(
            #[cfg(feature = "specs_storage_target")]
            $item;
        )*
    };
    ($($item:item)*) => {
        $(
            #[cfg(feature = "specs_storage_target")]
            $item
        )*
    };
}

#[macro_export]
macro_rules! cfg_async_dispatcher {
    ($($item:expr;)*) => {
        $(
            #[cfg(feature = "async_dispatcher")]
            $item;
        )*
    };
    ($($item:item)*) => {
        $(
            #[cfg(feature = "async_dispatcher")]
            $item
        )*
    };
}


#[macro_export]
macro_rules! cfg_not_async_dispatcher {
    ($($item:expr;)*) => {
        $(
            #[cfg(not(feature = "async_dispatcher"))]
            $item;
        )*
    };
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "async_dispatcher"))]
            $item
        )*
    };
}