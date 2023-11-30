use std::sync::Arc;

use crate::prelude::*;

use crate::{CallAsync, FieldPacket, Plugin, ThunkContext, BlockObject};

/// Field ref is a shared reference to an owner and includes a v-table for accessing fields on the owner,
///
/// This is used by the derive library to create "Virtual" representations of Reality objects.
///
/// A "Virtual" representation can have listeners and applies changes in a serialized manner. The "Virtual" type
/// is mainly useful in tooling contexts when state is going to be mutated outside of the initialized state interpreted by
/// `runmd`` blocks, or for managing runtime dependencies between nodes.
///
/// # TODO -- Intention
/// For example, if a reverse-proxy node needs to wait for an engine proxy to start before it can forward traffic,
/// then it would be useful for the reverse-proxy to "listen" to the engine-proxy's state. The virtual representation can be used
/// to create a bridge between the two-nodes through the ThunkContext.
///
pub struct FieldRef<Owner = Noop, Value = (), ProjectedValue = ()>
where
    Owner: Plugin + 'static,
    Value: 'static,
    ProjectedValue: 'static,
{
    /// Reference to the owner,
    ///
    owner: Arc<tokio::sync::watch::Sender<Owner>>,
    /// Field vtable for accessing the underlying field,
    ///
    table: &'static FieldVTable<Owner, Value, ProjectedValue>,
    /// Field condition,
    ///
    condition: FieldCondition,
}

/// TODO -- add noop attribute, allow PAth in call=
/// 
#[derive(Reality, Clone, Default, Debug)]
#[reality(call = noop, plugin)]
pub struct Noop;

async fn noop(_: &mut ThunkContext) -> anyhow::Result<()> {
    Ok(())
}

impl<Owner: Plugin, Value, ProjectedValue> Clone for FieldRef<Owner, Value, ProjectedValue> {
    fn clone(&self) -> Self {
        Self {
            condition: self.condition.clone(),
            owner: self.owner.clone(),
            table: self.table,
        }
    }
}

impl FieldRef {
    /// Creates a new field ref,
    ///
    pub const fn new<Owner: Plugin, Value, ProjectedValue>(
        owner: Arc<tokio::sync::watch::Sender<Owner>>,
        table: &'static FieldVTable<Owner, Value, ProjectedValue>,
    ) -> FieldRef<Owner, Value, ProjectedValue> {
        FieldRef::<Owner, Value, ProjectedValue> {
            condition: FieldCondition::Default,
            owner,
            table,
        }
    }
}

/// Enumeration of conditions that fields can be in,
///
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldCondition {
    /// Default means that the field's value is the default, this indicates
    /// it was not configured by any initial instructions.
    ///
    #[default]
    Default,
    /// (TODO) -- To implement this, need to somehow map the defined from the ParsedBlock back to the virtual ref.
    ///
    /// Initial means that the field has been configured by a runmd instruction and is in it's
    /// initial state.
    ///
    Initial,
    /// Pending means that the value has changed from the initial value but hasn't been committed
    /// by the owner yet.
    ///
    /// This condition is only useful in the context of multiple field tx steps in order
    /// to determine if an earlier stage is communicating that the value has is in a pending state.
    ///
    /// This condition is set automatically by .finish(), if the result .is_ok().
    ///
    Pending,
    /// Committed means that the owner acknowledges this field as being a value that
    /// it wishes to use. This implies the field has been validated by the owner, and external
    /// watchers can now use the value for this field.
    ///
    /// Committing a field should never be required by the owner, however if the owner wishes
    /// to share it's values w/ other types to consume it must commit the field before sharing.
    ///
    Committed,
}

/// Transaction struct for applying changes to a field in a serialized manner,
///
pub struct FieldTx<Owner: Plugin + 'static, Value: 'static, ProjectedValue: 'static> {
    /// Current field state,
    ///
    current: Option<FieldRef<Owner, Value, ProjectedValue>>,
    /// Next field state,
    ///
    next: anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>,
}

impl<Owner: Plugin, Value, ProjectedValue> FieldTx<Owner, Value, ProjectedValue> 
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Processes the next action,
    ///
    pub fn next(
        mut self,
        mut next: impl FnMut(
            FieldRef<Owner, Value, ProjectedValue>,
        ) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>,
    ) -> Self {
        if self.current.is_none() {
            self.current = Some(self.next.unwrap());
        }

        let current = self.current.take().unwrap();

        let next = next(FieldRef {
            condition: current.condition,
            owner: current.owner,
            table: current.table,
        });

        FieldTx {
            current: self.current,
            next,
        }
    }

    /// Finishes the transaction and puts the current field in the pending state if an updated
    /// field will be returned,
    ///
    #[inline]
    pub fn finish(self) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>> 
    {
        /*
        TODO: Insert tower integegration here?
         */
        self.next.map(|mut n| {
            n.pending();
            n
        })
    }
}

impl<Owner: Plugin, Value, ProjectedValue> FieldRef<Owner, Value, ProjectedValue> 
where
    Owner::Virtual: NewFn<Inner = Owner>,
{
    /// Creates a new transaction for editing the field,
    ///
    /// When finish() is called if a change was made, then Ok(T) will be returned w/ new state,
    /// otherwise there is no change.
    ///
    #[inline]
    pub fn start_tx(self) -> FieldTx<Owner, Value, ProjectedValue> {
        FieldTx {
            current: Some(self),
            next: Err(anyhow::anyhow!("No changes")),
        }
    }

    /// Set a value for a field,
    ///
    /// Returns true if the owner was modified.
    ///
    #[inline]
    pub fn set(&mut self, value: impl Into<ProjectedValue>) -> bool {
        self.owner.send_if_modified(|owner| {
            if let Some(adapter) = self.table.set.adapter.as_ref() {
                adapter(self.table.set.root, owner, value.into())
            } else {
                (self.table.set.root)(owner, value.into())
            }
        })
    }

    /// If applicable, pushes a value for a field,
    ///
    /// Returns true if the owner was modified.
    ///
    #[inline]
    pub fn push(&mut self, value: Value) -> bool {
        self.owner.send_if_modified(|owner| {
            if let Some(adapter) = self.table.push.adapter.as_ref() {
                adapter(self.table.push.root, owner, value)
            } else {
                (self.table.push.root)(owner, value)
            }
        })
    }

    /// If applicable, inserts a value w/ a key for a field,
    ///
    /// Returns true if the owner was modified.
    ///
    #[inline]
    pub fn insert_entry(&mut self, key: String, value: Value) -> bool {
        self.owner.send_if_modified(|owner| {
            if let Some(adapter) = self.table.insert_entry.adapter.as_ref() {
                adapter(self.table.insert_entry.root, owner, value)
            } else {
                (self.table.insert_entry.root)(owner, key, value)
            }
        })
    }

    /// Views the current value,
    ///
    /// Borrows the current value and calls view. The view fn is a mutable fn, so it can mutate values outside of the scope.
    ///
    pub fn view_value(&self, mut view: impl FnMut(&ProjectedValue)) {
        let mut owner = self.owner.subscribe();

        let owner = owner.borrow_and_update();

        let value = if let Some(adapter) = self.table.get_ref.adapter_ref.as_ref() {
            adapter(self.table.get_ref.root, &owner).1
        } else {
            (self.table.get_ref.root)(&owner).1
        };

        view(value);
    }

    /// Manually, gets a mutable reference to the underlying value,
    ///
    /// **Note** If true is passed from edit, then listeners will be notified of a change even if a change was not made.
    ///
    /// **See** `tokio::sync::watch::Sender<T>` documentation
    ///
    pub fn edit_value(&self, mut edit: impl FnMut(&str, &mut ProjectedValue) -> bool) -> bool {
        self.owner.send_if_modified(|owner| {
            let (field, value) = if let Some(adapter) = self.table.get_mut.adapter_ref_mut.as_ref()
            {
                adapter(self.table.get_mut.root, owner)
            } else {
                (self.table.get_mut.root)(owner)
            };

            edit(field, value)
        })
    }

    /// Encodes the current field into a field packet,
    /// 
    pub fn encode(&self) -> FieldPacket {
        (self.table.encode.root)(<Owner as Plugin>::Virtual::new(self.owner.borrow().to_owned()))
    }

    /// Decodes and applies a field packet to a virtual reference returning a field-ref in the pending state only
    /// if changes were successfully applied.
    /// 
    /// Otherwise, returns an error in all other cases.
    /// 
    pub fn decode_and_apply(&self, fp: FieldPacket) -> anyhow::Result<Self> {
        (self.table.decode.root)(<Owner as Plugin>::Virtual::new(self.owner.borrow().to_owned()), fp)
    }

    /// Put the field in the "pending" condition,
    ///
    /// Automatically called during a field tx on .finish(), if the tx result is_ok().
    ///
    /// Can be called manually in the case of a multi-stage pipeline in order to communicate to subsequent stages
    /// that the value has changed.
    ///
    #[inline]
    pub fn pending(&mut self) {
        self.condition = FieldCondition::Pending;
    }

    /// Put the field in the "committed" condition,
    ///
    /// This indicates to consumers that the owner considers this field validated and in use by the
    /// owner.
    ///
    /// Returns true if there was a transition.
    ///
    #[inline]
    pub fn commit(&mut self) -> bool {
        let changed = matches!(self.condition, FieldCondition::Committed);

        self.condition = FieldCondition::Committed;

        !changed
    }

    /// Returns true if the field condition is currently FieldCondition::Committed,
    ///
    #[inline]
    pub fn is_committed(&self) -> bool {
        matches!(self.condition, FieldCondition::Committed)
    }

    /// Returns true if the field condition is currently FieldCondition::Pending,
    ///
    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self.condition, FieldCondition::Pending)
    }

    /// Returns true if the field condition is currently FieldCondition::Initial,
    ///
    #[inline]
    pub fn is_initial(&self) -> bool {
        matches!(self.condition, FieldCondition::Initial)
    }

    /// Returns true if the field condition is currently FieldCondition::Default,
    ///
    #[inline]
    pub fn is_default(&self) -> bool {
        matches!(self.condition, FieldCondition::Default)
    }
}

/// V-Table containing functions for handling fields from the owning type,
///
pub struct FieldVTable<Owner: Plugin + 'static, Value: 'static, ProjectedValue: 'static> {
    /// Returns a reference to the projected value and field name,
    ///
    get_ref: AdapterRef<fn(&Owner) -> (&str, &ProjectedValue), Owner, ProjectedValue>,
    /// Returns a mutable reference to a projected value and a field name,
    ///
    get_mut: AdapterRef<fn(&mut Owner) -> (&str, &mut ProjectedValue), Owner, ProjectedValue>,
    /// Takes a value from the owner,
    ///
    take: AdapterRef<fn(Owner) -> ProjectedValue, Owner, ProjectedValue>,
    /// Sets the value for a field,
    ///
    set: AdapterRef<fn(&mut Owner, ProjectedValue) -> bool, Owner, ProjectedValue>,
    /// If applicable, pushes a value to a field,
    ///
    push: AdapterRef<fn(&mut Owner, Value) -> bool, Owner, Value>,
    /// If applicable, inserts a value with a key to a field,
    ///
    insert_entry: AdapterRef<fn(&mut Owner, String, Value) -> bool, Owner, Value>,
    /// Encode the current field into a field packet,
    ///
    encode: AdapterRef<fn(Owner::Virtual) -> FieldPacket, Owner, Value>,
    /// Decode a field packet into a field reference,
    /// 
    decode: AdapterRef<fn(Owner::Virtual, FieldPacket) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>, Owner, Value>,
}

impl<Owner: Plugin, Value, ProjectedValue> Clone for FieldVTable<Owner, Value, ProjectedValue> {
    fn clone(&self) -> Self {
        Self {
            get_ref: self.get_ref.clone(),
            get_mut: self.get_mut.clone(),
            set: self.set.clone(),
            push: self.push.clone(),
            insert_entry: self.insert_entry.clone(),
            take: self.take.clone(),
            encode: self.encode.clone(),
            decode: self.decode.clone(),
        }
    }
}

/// Wrapper over some root fn R w/ optional adapter options,
///
struct AdapterRef<R: Clone, Owner, Value> {
    /// Root fn,
    ///
    root: R,
    /// Adapter fn,
    ///
    adapter: Option<Arc<dyn Fn(R, &mut Owner, Value) -> bool + Sync + Send + 'static>>,
    /// Adapter ref fn,
    ///
    adapter_ref: Option<Arc<dyn Fn(R, &Owner) -> (&str, &Value) + Sync + Send + 'static>>,
    /// Adapter ref mut fn,
    ///
    adapter_ref_mut: Option<Arc<dyn Fn(R, &mut Owner) -> (&str, &mut Value) + Sync + Send + 'static>>,
    /// Adapter ref to owned ref,
    ///
    adapter_ref_owned: Option<Arc<dyn Fn(R, Owner) -> Value + Sync + Send + 'static>>,
}

impl<R: Clone, Owner, Value> Clone for AdapterRef<R, Owner, Value> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            adapter: self.adapter.clone(),
            adapter_ref: self.adapter_ref.clone(),
            adapter_ref_mut: self.adapter_ref_mut.clone(),
            adapter_ref_owned: self.adapter_ref_owned.clone(),
        }
    }
}

impl<Owner: Plugin + 'static, Value: 'static, ProjectedValue: 'static> FieldVTable<Owner, Value, ProjectedValue> {
    /// Creates a new field vtable,
    ///
    pub fn new(
        get_ref: fn(&Owner) -> (&str, &ProjectedValue),
        get_mut: fn(&mut Owner) -> (&str, &mut ProjectedValue),
        set: fn(&mut Owner, ProjectedValue) -> bool,
        push: fn(&mut Owner, Value) -> bool,
        insert_entry: fn(&mut Owner, String, Value) -> bool,
        take: fn(Owner) -> ProjectedValue,
        encode: fn(Owner::Virtual) -> FieldPacket,
        decode: fn(Owner::Virtual, FieldPacket) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>,
    ) -> Self {
        Self {
            get_ref: AdapterRef {
                root: get_ref,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            get_mut: AdapterRef {
                root: get_mut,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            set: AdapterRef {
                root: set,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            push: AdapterRef {
                root: push,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            insert_entry: AdapterRef {
                root: insert_entry,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            take: AdapterRef {
                root: take,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            encode: AdapterRef {
                root: encode,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
            decode: AdapterRef {
                root: decode,
                adapter: None,
                adapter_ref: None,
                adapter_ref_mut: None,
                adapter_ref_owned: None,
            },
        }
    }

    /// Includes an adapter for the set fn,
    ///
    #[inline]
    pub fn with_get_ref_mut_adapter(
        mut self,
        adapter: impl Fn(
                fn(&mut Owner) -> (&str, &mut ProjectedValue),
                &mut Owner,
            ) -> (&str, &mut ProjectedValue)
            + Send
            + Sync
            + 'static,
    ) {
        self.get_mut.adapter_ref_mut = Some(Arc::new(adapter));
    }

    /// Includes an adapter for the set fn,
    ///
    #[inline]
    pub fn with_get_ref_adapter(
        mut self,
        adapter: impl Fn(fn(&Owner) -> (&str, &ProjectedValue), &Owner) -> (&str, &ProjectedValue)
            + Send
            + Sync
            + 'static,
    ) {
        self.get_ref.adapter_ref = Some(Arc::new(adapter));
    }

    /// Includes an adapter for the set fn,
    ///
    #[inline]
    pub fn with_set_adapter(
        mut self,
        adapter: impl Fn(fn(&mut Owner, ProjectedValue) -> bool, &mut Owner, ProjectedValue) -> bool
            + Sync
            + Send
            + 'static,
    ) {
        self.set.adapter = Some(Arc::new(adapter));
    }

    /// Includes an apater for the push fn,
    ///
    #[inline]
    pub fn with_push_adapter(
        mut self,
        adapter: impl Fn(fn(&mut Owner, Value) -> bool, &mut Owner, Value) -> bool
            + Sync
            + Send
            + 'static,
    ) {
        self.push.adapter = Some(Arc::new(adapter));
    }

    /// Includes an adapter for the insert_entry fn,
    ///
    #[inline]
    pub fn with_insert_entry_adapter(
        mut self,
        adapter: impl Fn(fn(&mut Owner, String, Value) -> bool, &mut Owner, Value) -> bool
            + Sync
            + Send
            + 'static,
    ) {
        self.insert_entry.adapter = Some(Arc::new(adapter));
    }

    /// Includes an adapter for the insert_entry fn,
    ///
    #[inline]
    pub fn with_take_adapter(
        mut self,
        adapter: impl Fn(fn(Owner) -> ProjectedValue, Owner) -> ProjectedValue + Sync + Send + 'static,
    ) {
        self.take.adapter_ref_owned = Some(Arc::new(adapter));
    }
}