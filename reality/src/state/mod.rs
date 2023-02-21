mod load;
pub use load::Load;

mod provider;
pub use provider::Provider;
pub use provider::iter_state;

/// Tests viable usage of state module traits in a tiny example,
/// 
#[allow(dead_code)]
#[allow(unused_imports)]
mod tests {
    use specs::prelude::*;
    use specs::{join::MaybeJoin, Component, Entities, Join, ReadStorage, SystemData, VecStorage};

    use crate::state::iter_state;

    use super::{Load, Provider};

    #[derive(Component, PartialEq, Debug)]
    #[storage(VecStorage)]
    struct Weight {
        val: usize,
    }

    #[derive(Component, PartialEq, Debug)]
    #[storage(VecStorage)]
    struct Pos {
        x: usize,
        y: usize,
    }

    type GravityData<'a> = (
        &'a ReadStorage<'a, Weight>,
        MaybeJoin<&'a ReadStorage<'a, Pos>>,
    );

    type ExistenceData<'a> = (
        &'a ReadStorage<'a, Weight>,
        &'a ReadStorage<'a, Pos>,
    );

    #[derive(SystemData)]
    struct Physics<'a> {
        entities: Entities<'a>,
        weight: ReadStorage<'a, Weight>,
        position: ReadStorage<'a, Pos>,
    }

    impl<'a> Provider<'a, GravityData<'a>> for Physics<'a> {
        fn provide(&'a self) -> GravityData<'a> {
            (&self.weight, self.position.maybe())
        }
    }

    /// Demonstrating multiple implemenations can exist
    /// 
    impl<'a> Provider<'a, ExistenceData<'a>> for Physics<'a> {
        fn provide(&'a self) -> ExistenceData<'a> {
            (&self.weight, &self.position)
        }
    }

    impl<'a> AsRef<Entities<'a>> for Physics<'a> {
        fn as_ref(&self) -> &Entities<'a> {
            &self.entities
        }
    }

    struct Object<'a> {
        weight: &'a Weight,
        pos: Option<&'a Pos>,
    }

    impl<'a> Load for Object<'a> {
        type Layout = GravityData<'a>;

        fn load((weight, pos): <Self::Layout as Join>::Type) -> Self {
            Self { weight, pos }
        }
    }

    #[test]
    fn test() {
        let mut world = World::new();
        world.register::<Weight>();
        world.register::<Pos>();

        let e = world
            .create_entity()
            .with(Weight { val: 10 })
            .with(Pos { x: 4, y: 1 })
            .build();
        world.maintain();

        let physics = world.system_data::<Physics>();

        // This usage is an alternative to (Entities, ReadStorage<Weight>, ReadStorage<Pos>).join()
        // in cases where you want to get the state of a single object
        let o = physics.state::<Object>(e).expect("should exist");
        assert_eq!(o.weight.val, 10);
        assert_eq!(o.pos.unwrap().x, 4);
        assert_eq!(o.pos.unwrap().y, 1);

        // In addition, provides function to iterate over state as you would with a normal join()
        // In this case will skip calling .load()
        for (_e, (_weight, _pos)) in physics.iter_preload_state::<Object>() {
            assert_eq!(e, _e);
            assert_eq!(o.pos, _pos);
            assert_eq!(o.weight, _weight);
        }

        // Also, a method to return a vec of state 
        for (_e, _o) in physics.state_vec::<Object>() {
            assert_eq!(e, _e);
            assert_eq!(o.pos, _o.pos);
            assert_eq!(o.weight, _o.weight);
        }

        // In cases where it would be beneficial to have an iterator instead of allocating a vector,
        for (_e, _o) in iter_state::<Object, _>(&physics) {
            assert_eq!(e, _e);
            assert_eq!(o.pos, _o.pos);
            assert_eq!(o.weight, _o.weight);
        }
    }
}
