use crate::v2::Property;

pub trait Config {
    fn config(&mut self, property: Property);
}
