use std::sync::Arc;

use async_trait::async_trait;

use crate::define_intern_table;
use crate::prelude::*;
use crate::push_tag;

// Intern table of receiver names
define_intern_table!(RECV_NAMES: String);

// Intern table containing repr's of receiver owned fields
define_intern_table!(RECV_FIELDS: Vec<Repr>);

/// Allows types to link runmd nodes to their respective type/field representation levels,
///
#[async_trait(?Send)]
pub trait Recv {
    /// Symbol for this receiver,
    ///
    fn symbol() -> &'static str;

    /// Links a node level to a receiver and returns a new Repr,
    ///
    async fn link_recv(
        node: NodeLevel,
        fields: Vec<Repr>,
    ) -> anyhow::Result<Repr>
    where
        Self: Sized + Send + Sync + 'static,
    {
        let mut repr = Linker::<CrcInterner>::describe_resource::<Self>();
        let recv = RecvLevel::new::<Self>(fields);
        repr.push_level(recv)?;
        repr.push_level(node.clone())?;
        repr.link().await
    }

    /// Links a node level to a field level and returns a new Repr,
    ///
    async fn link_field(
        resource: ResourceLevel,
        field: FieldLevel,
        node: NodeLevel,
    ) -> anyhow::Result<Repr> {
        let mut repr = Linker::<CrcInterner>::default();
        repr.push_level(resource)?;
        repr.push_level(field)?;
        repr.push_level(node)?;
        repr.link().await
    }
}

/// Receiver level contains tags that can point to fields owned by a receiver,
///
pub struct RecvLevel {
    /// Name of this receiver,
    ///
    name: Tag<String, Arc<String>>,
    /// **Active** fields owned by this receiver,
    ///
    fields: Tag<Vec<Repr>, Arc<Vec<Repr>>>,
}

impl RecvLevel {
    /// Creates a new receiver level,
    ///
    pub fn new<R>(fields: Vec<Repr>) -> Self
    where
        R: Recv,
    {
        Self {
            name: Tag::new(&RECV_NAMES, Arc::new(R::symbol().to_string())),
            fields: Tag::new(&RECV_FIELDS, Arc::new(fields)),
        }
    }
}

impl Level for RecvLevel {
    fn configure(&self, interner: &mut impl InternerFactory) -> InternResult {
        push_tag!(dyn interner, &self.name);
        push_tag!(dyn interner, &self.fields);

        interner.set_level_flags(LevelFlags::LEVEL_1);
        interner.interner()
    }

    type Mount = ();

    fn mount(&self) -> Self::Mount {
        ()
    }
}

/// Wrapper-struct for an intern handle providing api's to access receiver level tags,
///
pub struct RecvRepr(pub(crate) InternHandle);

impl RecvRepr {
    /// Returns the name of the receiver,
    ///
    #[inline]
    pub async fn name(&self) -> Option<Arc<String>> {
        self.0.recv_name().await
    }

    /// Tries to return the name of the receiver,
    ///
    #[inline]
    pub fn try_name(&self) -> Option<Arc<String>> {
        self.0.try_recv_name()
    }

    /// Returns the name of the receiver fields,
    ///
    #[inline]
    pub async fn fields(&self) -> Option<Arc<Vec<Repr>>> {
        self.0.recv_fields().await
    }

    /// Tries to return the receiver fields,
    ///
    #[inline]
    pub fn try_fields(&self) -> Option<Arc<Vec<Repr>>> {
        self.0.try_recv_fields()
    }

    /// Finds the repr of a field owned by receiver,
    ///
    pub fn find_field(&self, name: &str) -> Option<Repr> {
        if let Some(fields) = self.try_fields() {
            fields
                .iter()
                .find(|f| {
                    f.as_field()
                        .and_then(|f| {
                            if f.try_name() == Some(name) {
                                Some(f)
                            } else {
                                None
                            }
                        })
                        .is_some()
                })
                .copied()
        } else {
            None
        }
    }
}
