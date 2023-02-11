mod extension_table;
pub use extension_table::ExtensionTable;

pub mod action;
pub use action::Action;

mod value_provider;
pub use value_provider::ValueProvider;

mod attribute;
pub use attribute::Attribute;

mod block;
pub use block::Block;

mod root;
pub use root::Root;

mod compiled;
pub use compiled::Compiled;

mod error;
pub use error::Error;

mod tag;
pub use tag::Tag;

// #[allow(unused_imports)]
// mod tests {
//     use std::sync::Arc;

//     use specs::{Builder, Component, VecStorage};
//     use toml_edit::Value;

//     use super::{build_document, ExtensionTable, ValueProvider, action::extensions::Expand};

//     #[derive(Component)]
//     #[storage(VecStorage)]
//     struct Pos(usize, usize);

//     struct Test;

//     impl Expand for Test {
//         fn ident(self: std::sync::Arc<Self>) -> String {
//             "test".to_string()
//         }

//         fn expand(self: std::sync::Arc<Self>, _: &super::Attribute) -> Vec<super::Action> {
//             vec![build_document(|d, eb| {
//                 let x = d["test"].int("x")?;
//                 let y = d["test"].int("y")?;

//                 let eb = eb.with(Pos(x as usize, y as usize));

//                 Ok(eb.build())
//             })]
//         }
//     }
// }
