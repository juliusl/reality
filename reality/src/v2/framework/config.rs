use specs::VecStorage;
use specs::Component;

use crate::v2::Properties;

#[derive(Component)]
#[storage(VecStorage)]
pub struct Config {
    /// Properties,
    /// 
    properties: Properties,
    /// Input to handle from framework,
    /// 
    input: String,
}
