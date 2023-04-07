use crate::Error;

/// Trait providing a select() fn to select a parameter by Type,
/// 
pub trait Select<T> {
    /// Returns a reference to T,
    /// 
    fn select(&self) -> Result<&T, Error>
    where
        Self: AsRef<T>;

    /// Returns a mutable reference to T,
    /// 
    fn select_mut(&mut self) -> Result<&mut T, Error>
    where
        Self: AsMut<T>;
}
