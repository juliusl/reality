mod interner;
mod repr;

mod field;

#[cfg(feature = "crc-interner")]
mod crc;

#[macro_use]
mod macros {
    /// Defines a global intern table,
    ///
    /// **Example**
    ///
    /// ```rs no_run
    /// // Define the intern table
    /// define_intern_table!(EXAMPLE: &'static str);
    ///
    /// ...
    ///
    /// async fn test() -> anyhow::Result<()> {
    ///     // Assigns an intern handle for value
    ///     EXAMPLE.assign_intern(InternHandle::default(), "hello world").await?;
    ///
    ///     // Get a handle
    ///     let handle = EXAMPLE.handle(..).await?;
    ///     
    ///     // Upgrade to the stored value
    ///     let value = handle.upgrade().unwrap_or_default();
    ///     assert_eq!("hello world".to_string(), value.to_string());
    ///
    ///     // Create a strong reference
    ///     let value: Arc<T> = EXAMPLE.strong_ref(..).await?;
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
    #[allow(unused_imports)]
    pub use super::macros::*;

    pub use super::interner::InternHandle;
    pub use super::interner::InternTable;
    pub use super::interner::InternerFactory;

    pub use super::field::Field;

    pub use super::repr::ResourceLevel;
    pub use super::repr::FieldLevel;
    pub use super::repr::InputLevel;
    pub use super::repr::HostLevel;
    pub use super::repr::ReprFactory;
    pub use super::repr::Repr;

    #[cfg(feature = "crc-interner")]
    pub use super::crc::CrcInterner;

    /// Type-alias for a function that takes an intern handle and returns a future,
    ///
    pub type InternHandleFutureThunk = Box<
        dyn FnOnce(
                InternHandle,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>,
            > + Send,
    >;
}

#[allow(dead_code)]
#[allow(unused)]
mod tests {
    use std::sync::Arc;

    use crate::repr::{Tag, HANDLES, Level};

    use super::prelude::*;

    define_intern_table!(TEST_INTERNER: &'static str);

    #[tokio::test]
    async fn test_intern_table() {
        TEST_INTERNER
            .assign_intern(InternHandle::default(), "hello world")
            .await
            .unwrap();

        // Test get/try_get
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

        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .try_get(&InternHandle::default())
                .unwrap()
                .upgrade()
                .unwrap()
                .to_string()
        );

        // Test strong_ref/try_strong_ref
        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .strong_ref(&InternHandle::default())
                .await
                .unwrap()
                .to_string()
        );

        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .try_strong_ref(&InternHandle::default())
                .unwrap()
                .to_string()
        );

        // Test clone/try_clone
        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .clone(&InternHandle::default())
                .await
                .unwrap()
                .to_string()
        );

        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .try_clone(&InternHandle::default())
                .unwrap()
                .to_string()
        );

        // Test copy/try_copy
        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .copy(&InternHandle::default())
                .await
                .unwrap()
                .to_string()
        );

        assert_eq!(
            "hello world".to_string(),
            TEST_INTERNER
                .try_copy(&InternHandle::default())
                .unwrap()
                .to_string()
        );
    }

    #[tokio::test]
    async fn test_intern_handle_link() {
        struct Test;

        impl Field<0> for Test {
            type ParseType = String;

            type ProjectedType = String;

            fn field_name() -> &'static str {
                "test"
            }
        }

        let mut interner = CrcInterner::default();

        let resource = ResourceLevel::new::<String>().configure(&mut interner).result().unwrap();
        let field = FieldLevel::new::<0, Test>().configure(&mut interner).result().unwrap();
        let input = InputLevel::new("hello world", "").configure(&mut interner).result().unwrap();

        let from = Tag::new(&HANDLES, Arc::new(resource));
        let to = Tag::new(&HANDLES, Arc::new(field));

        let linked = from.link(&to).await.unwrap();

        eprintln!("{:x?}", linked);

        let (prev, current) = linked.node(); 

        eprintln!("{:x?} -> {:x?}", prev, current);

        let linked = &HANDLES.try_get(&current).unwrap();
        eprintln!("{:x?}", linked.upgrade());

        let from = Tag::new(&HANDLES, Arc::new(field));
        let to = Tag::new(&HANDLES, Arc::new(input));

        let linked = from.link(&to).await.unwrap();

        let (prev, current) = linked.node();
        eprintln!("{:x?} -> {:x?}", prev, current);

        let linked = &HANDLES.try_get(&prev.unwrap()).unwrap();
        eprintln!("{:x?}", linked.upgrade());

        ()
    }
}
