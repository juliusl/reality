use specs::{Entity, World, WorldExt};

use crate::{Block, Interpreter};

/// Evaluate blocks and update world state,
///
pub fn evaluate(
    mut world: impl AsMut<World> + AsRef<World>,
    blocks: Vec<(Entity, Block)>,
    plugins: impl Clone + IntoIterator<Item = impl Interpreter>,
) {
    for (entity, block) in blocks {
        for plugin in plugins.clone().into_iter() {
            if let Some(component) = plugin.interpret(&block, None) {
                plugin.initialize(world.as_mut());
                match world.as_ref().write_component().insert(entity, component) {
                    Ok(previous) => {
                        // Reinterpret if replacing a component
                        if let Some(previous) = previous {
                            if let Some(updated) = plugin.interpret(&block, Some(&previous)) {
                                world.as_ref()
                                    .write_component()
                                    .insert(entity, updated)
                                    .expect("inserted");
                            }
                        }
                    }
                    Err(err) => panic!("Could not insert component {err}"),
                }
            }
        }
    }
}
