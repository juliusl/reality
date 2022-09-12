use std::{fs::create_dir_all, path::Path};

use specs::{Component, DefaultVecStorage, Entity, World, WorldExt};

use crate::{Block, Interpreter};

mod save;
pub use save::Save;

/// Struct for managing the directory .world/
///
/// This directory will contain all of the world assets.
///  
#[derive(Default, Clone)]
pub struct WorldDir {
    /// The root directory, defaults to ''
    ///
    root: &'static str,
}

impl From<&'static str> for WorldDir {
    fn from(root: &'static str) -> Self {
        Self { root }
    }
}

impl WorldDir {
    /// Returns a new World reading blocks from data stored in this directory,
    /// and then interpreting each block with a collection of plugins. If the plugin
    /// returns a component it will be added to the block's entity.
    ///
    pub fn world(&self, plugins: impl Clone + IntoIterator<Item = impl Interpreter>) -> World {
        let mut world = World::new();

        for (entity, block) in self.load_blocks(&world) {
            for plugin in plugins.clone().into_iter() {
                if let Some(component) = plugin.interpret(&block) {
                    plugin.initialize(&mut world);
                    world
                        .write_component()
                        .insert(entity, component)
                        .expect("can insert component");
                }
            }
        }
        
        world
    }

    /// Returns the canonical path to the root of the directory,
    /// 
    /// If the directory doesn't already exist, this method will create it
    /// under the root dir.
    ///
    pub fn dir(&self) -> impl AsRef<Path> {
        let dir = Path::new(self.root).join(".world");

        if !dir.exists() {
            create_dir_all(&dir).expect("can create directory");
        }

        dir.canonicalize().expect("should be able to canonicalize")
    }

    /// Returns a vector of blocks loaded from the dir,
    ///
    pub fn load_blocks(&self, _world: &World) -> Vec<(Entity, Block)> {
        todo!()
    }
}

/// This type can be used to specify no plugins when loading an
/// instance of a World.
///
/// Also, this serves an example of how to write a collection of plugins.  
///
#[derive(Clone, Component, Default)]
#[storage(DefaultVecStorage)]
pub struct NoPlugins();

impl Interpreter for NoPlugins {
    type Output = Self;

    fn initialize(&self, _: &mut World) {
        unimplemented!()
    }

    fn interpret(&self, _: &Block) -> Option<Self::Output> {
        unimplemented!()
    }

    fn interpret_mut(&mut self, _: &Block) {
        unimplemented!()
    }
}

impl IntoIterator for NoPlugins {
    type Item = Self;

    type IntoIter = std::vec::IntoIter<Self>;

    fn into_iter(self) -> Self::IntoIter {
        vec![].into_iter()
    }
}

#[test]
fn test_world_dir() {
    // Create a test world dir in the .test folder
    let test = WorldDir::from(".test");

    // Make sure that the directory is created
    assert!(test.dir().as_ref().exists(), "should exist");

    // Cleanup the directory
    std::fs::remove_dir_all(test.dir()).expect("deleted");
}
