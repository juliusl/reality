mod entity;
mod interner;
mod level;
mod repr;
mod tag;

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

    /// Pushes a tag and a future that can assign an intern handle for a value,
    ///
    #[macro_export]
    macro_rules! push_tag {
        ($interner:ident, $tag:expr) => {
            let tag = $tag;
            $interner.push_tag(tag.value(), move |h| {
                Box::pin(async move { tag.assign(h).await })
            });
        };
        (dyn $interner:ident, $tag:expr) => {
            let tag = $tag;

            let inner = tag.clone();
            $interner.push_tag(tag.value(), move |h| {
                Box::pin(async move { inner.assign(h).await })
            });
        };
    }
}

pub mod prelude {
    #[allow(unused_imports)]
    pub use super::macros::*;
    pub use crate::repr::prelude::*;

    pub use super::interner::InternHandle;
    pub use super::interner::InternTable;
    pub use super::interner::InternerFactory;
    pub use super::interner::InternResult;
    pub use super::interner::LevelFlags;

    pub use super::tag::Tag;

    pub use super::level::Level;

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
    use std::{collections::BTreeMap, sync::Arc};

    use crate::repr::HANDLES;

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
    #[tracing_test::traced_test]
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

        let resource = ResourceLevel::new::<String>()
            .configure(&mut interner)
            .wait_for_ready()
            .await;
        let field = FieldLevel::new::<0, Test>()
            .configure(&mut interner)
            .wait_for_ready()
            .await;

        let mut annotations = BTreeMap::new();
        annotations.insert("description".to_string(), "really cool node".to_string());

        let input = NodeLevel::new()
            .with_input("hello world")
            .with_annotations(annotations)
            .configure(&mut interner)
            .wait_for_ready()
            .await;

        let from = Tag::new(&HANDLES, Arc::new(resource));
        let to = Tag::new(&HANDLES, Arc::new(field));

        // TODO: convert eprintlns to asserts
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

        let a = crate::repr::node::ANNOTATIONS
            .try_strong_ref(&input)
            .unwrap();
        eprintln!("{:?}", a);

        let test = Test::create_repr::<CrcInterner>().unwrap();
        eprintln!("{:x?}", test.link().await.unwrap());
        ()
    }
}
