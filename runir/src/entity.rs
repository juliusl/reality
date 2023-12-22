// use crate::prelude::InternerFactory;

// /// Interner implementation that interns based on an entity id system,
// ///
// pub struct EntityInterner {

// }

// impl InternerFactory for EntityInterner {
//     fn push_tag<T: std::hash::Hash + Send + Sync + 'static>(
//         &mut self,
//         value: T,
//         assign: impl FnOnce(crate::prelude::InternHandle) -> std::pin::Pin<Box<dyn futures::prelude::Future<Output = anyhow::Result<()>> + Send>>
//             + Send
//             + 'static,
//     ) {
//         todo!()
//     }

//     fn set_level_flags(&mut self, flags: crate::prelude::LevelFlags) {
//         todo!()
//     }

//     fn interner(&mut self) -> crate::prelude::InternResult {
//         todo!()
//     }
// }
