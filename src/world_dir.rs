use std::{fs::create_dir_all, path::Path};

use specs::{Component, DefaultVecStorage, Entity, World, WorldExt};

use crate::{Block, Interpreter, evaluate};

/// Struct for managing the directory .world/
///
/// This directory will contain all of the world assets.
///  
#[derive(Default)]
pub struct WorldDir {
    /// The root directory, defaults to ''
    ///
    root: &'static str,
    /// The world this world dir maintains
    /// 
    world: World,
}

impl From<&'static str> for WorldDir {
    fn from(root: &'static str) -> Self {
        Self { root, world: World::new() }
    }
}

impl AsMut<World> for WorldDir {
    fn as_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl AsRef<World> for WorldDir {
    fn as_ref(&self) -> &World {
        &self.world
    }
}

impl Into<World> for WorldDir {
    fn into(self) -> World {
        self.world
    }
}

impl WorldDir {
    /// Loads blocks from the world dir, interpreting each with plugins
    /// 
    pub fn evaluate(self, plugins: impl Clone + IntoIterator<Item = impl Interpreter>) {
        let blocks = self.load_blocks(self.as_ref());
        evaluate(self, blocks, plugins);
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
   fn load_blocks(&self, _world: &World) -> Vec<(Entity, Block)> {
        vec![]
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

    fn interpret(&self, _: &Block, _: Option<&Self::Output>) -> Option<Self::Output> {
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

    // Tests no plugins struct
    test.evaluate(NoPlugins());
}
