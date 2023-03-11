use specs::Entity;
use specs::VecStorage;
use specs::Component;

/// Component to store links to this entity,
///
#[derive(Component, Default, Clone)]
#[storage(VecStorage)]
pub struct Links {
    /// List of links,
    ///
    links: Vec<Link>,
}

impl Links {
    /// Inserts an incoming link,
    ///
    pub fn insert_incoming(&mut self, incoming: Entity) {
        self.links.push(Link::Incoming(incoming));
    }

    /// Inserts an outgoing link,
    ///
    pub fn insert_outgoing(&mut self, outgoing: Entity) {
        self.links.push(Link::Outgoing(outgoing));
    }

    /// Returns an iterator over links,
    ///
    pub fn iter_links(&self) -> impl Iterator<Item = &Link> {
        self.links.iter()
    }

    /// Returns an iterator over outgoing entities,
    /// 
    pub fn outgoing(&self) -> impl Iterator<Item = Entity> + '_ {
        self.iter_links()
            .filter_map(|l| match l {
                Link::Incoming(_) => None,
                Link::Outgoing(o) => Some(o),
            })
            .cloned()
    }

    /// Returns an iterator over incoming entities,
    /// 
    pub fn incoming(&self) -> impl Iterator<Item = Entity> + '_ {
        self.iter_links()
            .filter_map(|l| match l {
                Link::Incoming(i) => Some(i),
                Link::Outgoing(_) => None,
            })
            .cloned()
    }
}

/// Enumeration of link variants,
///
#[derive(Clone, Copy)]
pub enum Link {
    /// This entity is an incoming link,
    ///
    Incoming(Entity),
    /// This entity is an outgoing link,
    ///
    Outgoing(Entity),
}
