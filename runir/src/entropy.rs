use std::cell::Cell;

thread_local!(
    /// Entropy value to use when creating intern handles,
    ///
    /// **Note** In most cases this does not need to be set unless there is parallel distinct IR generation
    /// happening, which can be the case during unit tests. In that case this can be set before the test
    /// runs and will allow intern handles generated on that thread to be scoped w/ an entropy value.
    ///
    pub(crate) static ENTROPY: Cell<u64> = Cell::new(0)
);

/// Initializes a random entropy value for the current thread,
///
/// **Note**: Allows for intern handles created on this thread to have their data value set w/ entropy.
///
pub fn set_entropy() {
    let (_, e) = uuid::Uuid::new_v4().as_u64_pair();
    ENTROPY.set(e)
}
