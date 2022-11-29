use specs::World;

use crate::{Block, Interpreter};

/// Evaluate blocks and update world state,
///
pub fn evaluate(
    world: impl AsRef<World>,
    blocks: Vec<Block>,
    plugins: impl Clone + IntoIterator<Item = impl Interpreter>,
) {
    for block in blocks {
        for plugin in plugins.clone().into_iter() {
            plugin.interpret(world.as_ref(), &block);
        }
    }
}
