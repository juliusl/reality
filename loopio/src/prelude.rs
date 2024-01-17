#[cfg(feature = "hyper-ext")]
pub use crate::ext::hyper_ext::*;

#[cfg(feature = "poem-ext")]
pub use crate::ext::poem_ext::*;

#[cfg(feature = "std-ext")]
pub use crate::ext::std_ext::*;

#[cfg(feature = "wire-ext")]
pub use crate::ext::wire_ext::*;

pub use crate::action::Action;
pub use crate::action::ActionExt;
pub use crate::address::Address;
pub use crate::engine::Engine;
pub use crate::engine::EngineBuilder;
pub use crate::engine::EngineHandle;
pub use crate::engine::Published;
pub use crate::ext::*;
pub use crate::foreground::ForegroundEngine;
pub use crate::host::Host;
pub use crate::operation::Operation;
pub use crate::sequence::Sequence;
pub use crate::work::WorkState;

pub use reality::prelude::*;

/// Engine build middleware,
///
pub type EngineBuildMiddleware = fn(EngineBuilder) -> EngineBuilder;

/// Function for defining an engine builder,
///
pub fn define_engine(middleware: &[EngineBuildMiddleware]) -> EngineBuilder {
    let engine_builder = Engine::builder();

    let engine_builder = middleware.iter().fold(engine_builder, |eb, f| f(eb));

    engine_builder
}
