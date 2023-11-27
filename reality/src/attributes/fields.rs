use std::sync::Arc;

/// Wrapper struct over a field table,
///
/// Provides a field api. The reource key aet can be used to map into the
/// metadata cache.
///
pub struct FieldRef<Owner = (), Value = (), ProjectedValue = ()>
where
    Owner: 'static,
    Value: 'static,
    ProjectedValue: 'static,
{
    owner: Arc<tokio::sync::watch::Sender<Owner>>,
    /// Field vtable for accessing the underlying field,
    ///
    table: &'static FieldVTable<Owner, Value, ProjectedValue>,
}

impl FieldRef {
    /// Creates a new field ref,
    ///
    pub const fn new<Owner, Value, ProjectedValue>(
        owner: Arc<tokio::sync::watch::Sender<Owner>>,
        table: &'static FieldVTable<Owner, Value, ProjectedValue>,
    ) -> FieldRef<Owner, Value, ProjectedValue> {
        FieldRef::<Owner, Value, ProjectedValue> { owner, table }
    }
}

/// Field transaction
///
pub struct FieldTx<Owner: 'static, Value: 'static, ProjectedValue: 'static> {
    /// Current field state,
    ///
    current: Option<FieldRef<Owner, Value, ProjectedValue>>,
    /// Next field state,
    ///
    next: anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>,
}

impl<Owner, Value, ProjectedValue> FieldTx<Owner, Value, ProjectedValue> {
    /// Processes the next action,
    ///
    pub fn next(
        mut self,
        next: impl Fn(
            FieldRef<Owner, Value, ProjectedValue>,
        ) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>>,
    ) -> Self {
        if self.current.is_none() {
            self.current = Some(self.next.unwrap());
        }

        let current = self.current.take().unwrap();

        let next = next(FieldRef {
            owner: current.owner,
            table: current.table,
        });

        FieldTx {
            current: self.current,
            next,
        }
    }

    /// Processes the next action,
    ///
    pub fn finish(self) -> anyhow::Result<FieldRef<Owner, Value, ProjectedValue>> {
        /*
        TODO: Insert tower integegration here? 
         */
        self.next
    }
}

impl<Owner, Value, ProjectedValue> FieldRef<Owner, Value, ProjectedValue> {
    /// Returns a new transaction w/ an initial owner,
    ///
    pub fn start_tx(self) -> FieldTx<Owner, Value, ProjectedValue> {
        FieldTx {
            current: Some(self),
            next: Err(anyhow::anyhow!("No changes")),
        }
    }

    /// Set a value for a field,
    ///
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

    /// Edits the current value,
    /// 
    /// If true is returned from, listeners will be notified.
    /// 
    pub fn edit_value(&self, mut edit: impl FnMut(&mut ProjectedValue) -> bool) -> bool {
        self.owner.send_if_modified(|owner| {
            let value = if let Some(adapter) = self.table.get_mut.adapter_ref_mut.as_ref() {
                adapter(self.table.get_mut.root, owner).1
            } else {
                (self.table.get_mut.root)(owner).1
            };

            edit(value)
        })
    }
}

/// Field VTable,
///
pub struct FieldVTable<Owner, Value, ProjectedValue> {
    /// Returns a reference to the projected value and field name,
    ///
    get_ref: AdapterRef<fn(&Owner) -> (&str, &ProjectedValue), Owner, ProjectedValue>,
    /// Returns a mutable reference to a projected value and a field name,
    ///
    get_mut: AdapterRef<fn(&mut Owner) -> (&str, &mut ProjectedValue), Owner, ProjectedValue>,
    /// Sets the value for a field,
    ///
    set: AdapterRef<fn(&mut Owner, ProjectedValue) -> bool, Owner, ProjectedValue>,
    /// If applicable, pushes a value to a field,
    ///
    push: AdapterRef<fn(&mut Owner, Value) -> bool, Owner, Value>,
    /// If applicable, inserts a value with a key to a field,
    ///
    insert_entry: AdapterRef<fn(&mut Owner, String, Value) -> bool, Owner, Value>,
    /// Takes a value from the owner,
    ///
    take: AdapterRef<fn(Owner) -> ProjectedValue, Owner, ProjectedValue>,
}

impl<Owner, Value, ProjectedValue> Clone for FieldVTable<Owner, Value, ProjectedValue> {
    fn clone(&self) -> Self {
        Self {
            get_ref: self.get_ref.clone(),
            get_mut: self.get_mut.clone(),
            set: self.set.clone(),
            push: self.push.clone(),
            insert_entry: self.insert_entry.clone(),
            take: self.take.clone(),
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
    adapter_ref_mut:
        Option<Arc<dyn Fn(R, &mut Owner) -> (&str, &mut Value) + Sync + Send + 'static>>,
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

impl<Owner, Value, ProjectedValue> FieldVTable<Owner, Value, ProjectedValue> {
    /// Creates a new field vtable,
    ///
    pub fn new(
        get_ref: fn(&Owner) -> (&str, &ProjectedValue),
        get_mut: fn(&mut Owner) -> (&str, &mut ProjectedValue),
        set: fn(&mut Owner, ProjectedValue) -> bool,
        push: fn(&mut Owner, Value) -> bool,
        insert_entry: fn(&mut Owner, String, Value) -> bool,
        take: fn(Owner) -> ProjectedValue,
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
        }
    }

    /// Includes an adapter for the set fn,
    ///
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
    pub fn with_take_adapter(
        mut self,
        adapter: impl Fn(fn(Owner) -> ProjectedValue, Owner) -> ProjectedValue + Sync + Send + 'static,
    ) {
        self.take.adapter_ref_owned = Some(Arc::new(adapter));
    }
}
