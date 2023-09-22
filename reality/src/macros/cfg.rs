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
