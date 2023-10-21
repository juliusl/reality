use std::collections::BTreeMap;

use reality::prelude::*;

/// Map of menus
///
pub struct MenuMap {
    /// Map of menus,
    ///
    map: BTreeMap<String, Menu>,
}

/// Menu is the core type exported by this library,
///
/// At it's core a menu is a collection of "Commands".
///
/// In `reality` terms, all reality objects being based on FromStr means that the goal of a command is to output a text-buffer to pass
/// to a AttributeType's OnParseField implementation.
///
#[derive(Reality)]
pub struct Menu {
    /// Identifier used when referring to this menu in config/tables/etc
    ///
    #[reality(ignore)]
    ident: String,
    /// Display name to use for this menu,
    ///
    name: String,
    /// Map of commands,
    /// 
    #[reality(ignore)]
    commands: BTreeMap<String, ()>
}

/// Struct mapping an implementation of OnParseField to an address,
///
pub struct Command<Owner, Field, const FIELD_OFFSET: usize>
where
    Owner: AttributeType<Shared> + OnParseField<FIELD_OFFSET, Field>,
    Field: FromStr + Send + Sync + 'static,
{
    /// Identifier used when referring to this command in config/tables/etc
    ///
    ident: String,
    /// Name of the field this command is handling,
    ///
    field_name: &'static str,
    /// Pointer to OnParseField on_parse fn,
    ///
    on_parse: fn(&mut Owner, Field, Option<&String>),
    /// Pointer to OnParseField get fn
    /// 
    get: fn(&Owner) -> &<Owner as OnParseField<FIELD_OFFSET, Field>>::ProjectedType,
    /// Pointer to OnParseField get_mut fn
    /// 
    get_mut: fn(&mut Owner) -> &mut <Owner as OnParseField<FIELD_OFFSET, Field>>::ProjectedType,
}

impl<Owner, Field, const FIELD_OFFSET: usize> Command<Owner, Field, FIELD_OFFSET>
where
    Field: FromStr + Send + Sync + 'static,
    Owner: AttributeType<Shared> + OnParseField<FIELD_OFFSET, Field>,
{
    /// Creates a new command,
    ///
    pub fn new(ident: impl Into<String>) -> Self {
        Command {
            ident: ident.into(),
            field_name: <Owner as OnParseField<FIELD_OFFSET, Field>>::field_name(),
            on_parse: <Owner as OnParseField<FIELD_OFFSET, Field>>::on_parse,
            get: <Owner as OnParseField<FIELD_OFFSET, Field>>::get,
            get_mut: <Owner as OnParseField<FIELD_OFFSET, Field>>::get_mut,
        }
    }
}

impl FromStr for Menu {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            ident: s.to_string(),
            name: String::from("Menu"),
            commands: BTreeMap::new(),
        })
    }
}
