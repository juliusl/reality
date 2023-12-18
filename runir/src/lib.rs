mod interner;
mod repr;

mod field;

#[cfg(feature = "crc-interner")]
mod crc;

#[macro_use]
mod macros {
    /// Defines a global intern table,
    ///
    /// ```rs no_run
    /// define_intern_table!(EXAMPLE: &'static str);
    ///
    /// ...
    ///
    /// async fn test() -> anyhow::Result<()> {
    ///     // Assigns an intern handle for value
    ///     EXAMPLE.assign_intern(InternHandle::default(), "hello world").await?;
    ///
    ///     let handle = EXAMPLE.handle(..).await?;
    ///     
    ///     let value = handle.upgrade().unwrap_or_default();
    ///     assert_eq!("hello world".to_string(), value.to_string());
    /// }
    /// ```
    ///
    #[macro_export]
    macro_rules! define_intern_table {
        ($table:ident: $ty:ty) => {
            pub static $table: InternTable<$ty> = InternTable::<$ty>::new();
        };
    }
}

pub mod prelude {
    pub use super::macros::*;

    pub use super::interner::InternHandle;
    pub use super::interner::InternTable;
    pub use super::interner::InternerFactory;

    pub use super::field::Field;

    pub use super::repr::FieldLevel;
    pub use super::repr::HostLevel;
    pub use super::repr::InputLevel;
    pub use super::repr::ReprFactory;
    pub use super::repr::ResourceLevel;

    #[cfg(feature = "crc-interner")]
    pub use super::crc::CrcInterner;

    /// Type-alias for a function that returns a future,
    ///
    pub type FutureThunk = Box<
        dyn FnOnce(
                InternHandle,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>,
            > + Send,
    >;
}

#[allow(dead_code)]
mod tests {
    use super::prelude::*;

    define_intern_table!(TEST_INTERNER: &'static str);

    #[tokio::test]
    async fn test_intern_table() {
        TEST_INTERNER
            .assign_intern(InternHandle::default(), "hello world")
            .await
            .unwrap();

        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .get(&InternHandle::default())
                .await
                .unwrap()
                .upgrade()
                .unwrap()
                .to_string()
        );
    }
}
