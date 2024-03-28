/// Enables a field to be evaluated by it's owning type
/// 
#[macro_export]
macro_rules! enable_eval_on_field {
    ($owner:ty, $offset:literal) => {
        #[async_trait::async_trait(?Send)]
        impl runmd::prelude::ExtensionLoader
            for FieldRef<
                $owner,
                <$owner as runir::prelude::Field<$offset>>::ParseType,
                <$owner as runir::prelude::Field<$offset>>::ProjectedType,
            >
        where
            <$owner as Plugin>::Virtual: NewFn<Inner = $owner>,
        {
            async fn load_extension(
                &self,
                _: &str,
                _: Option<&str>,
                _: Option<&str>,
            ) -> Option<runmd::prelude::BoxedNode> {
                None
            }
            async fn unload(&mut self) {}
        }

        #[async_trait::async_trait(?Send)]
        impl runmd::prelude::Node
            for FieldRef<
                $owner,
                <$owner as runir::prelude::Field<$offset>>::ParseType,
                <$owner as runir::prelude::Field<$offset>>::ProjectedType,
            >
        where
            <$owner as Plugin>::Virtual: NewFn<Inner = $owner>,
            <$owner as runir::prelude::Field<$offset>>::ParseType:
                serde::Serialize + Clone + serde::de::DeserializeOwned,
            <<$owner as runir::prelude::Field<$offset>>::ParseType as FromStr>::Err:
                std::fmt::Debug,
        {
            /// Assigns a path to this node
            fn assign_path(&mut self, _: String) {}

            fn set_info(&mut self, _: runmd::prelude::NodeInfo, _: runmd::prelude::BlockInfo) {}

            fn parsed_line(&mut self, _: runmd::prelude::NodeInfo, _: runmd::prelude::BlockInfo) {}

            fn completed(self: Box<Self>) {}

            /// Define a property for this node,
            ///
            async fn define_property(
                &mut self,
                name: &str,
                _: Option<&str>,
                input: Option<&str>,
            ) {
                let input = input.unwrap_or("");
                match <$owner as runir::prelude::Field<$offset>>::ParseType::from_str(&input)
                    .map(|v| <$owner as OnParseField<$offset>>::to_wire(&v))
                {
                    Ok(Ok(value)) => {
                        let _ = self.decode_and_apply(value).unwrap();
                    }
                    Ok(Err(err)) => {
                        tracing::error!(
                            property = name,
                            "Could not convert to wire protocol - {err}"
                        );
                    }
                    Err(err) => {
                        tracing::error!(property = name, "Could not parse input - {:?}", err);
                    }
                }
            }
        }
    };
}