/// Field access,
/// 
#[derive(Debug)]
pub struct Field<'a, T> {
    /// Field owner type name,
    /// 
    pub owner: &'static str,
    /// Name of the field,
    /// 
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    /// 
    pub value: &'a T,
}

/// Mutable field access,
/// 
#[derive(Debug)]
pub struct FieldMut<'a, T> {
    /// Field owner type name,
    /// 
    pub owner: &'static str,
    /// Name of the field,
    /// 
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Mutable access to the field,
    /// 
    pub value: &'a mut T,
}

/// Field /w owned value,
/// 
#[derive(Debug)]
pub struct FieldOwned<T> {
    /// Field owner type name,
    /// 
    pub owner: &'static str,
    /// Name of the field,
    /// 
    pub name: &'static str,
    /// Offset of the field,
    ///  
    pub offset: usize,
    /// Current value of the field,
    /// 
    pub value: T,
}

/// Trait for visiting fields w/ read-only access,
/// 
pub trait Visit<T> {
    /// Returns a vector of fields,
    /// 
    fn visit<'a>(&'a self) -> Vec<Field<'a, T>>;
}

/// Trait for visiting fields w/ mutable access,
/// 
pub trait VisitMut<T> {
    /// Returns a vector of fields w/ mutable access,
    /// 
    fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, T>>;
}

/// Trait for setting a field,
/// 
pub trait SetField<T> {
    /// Sets a field on the receiver,
    /// 
    /// Returns true if successful.
    /// 
    fn set_field(&mut self, field: FieldOwned<T>) -> bool;
}

mod tests {
    use super::{VisitMut, FieldMut};

    struct Test {
        name: String,
        other: String,
    }

    impl VisitMut<String> for Test {
        fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, String>> {
            vec![
                FieldMut { owner: std::any::type_name::<Test>(), name: "name", offset: 1, value: &mut self.name }, 
                FieldMut { owner: std::any::type_name::<Test>(), name: "other", offset: 1, value: &mut self.other }
            ]
        }
    }

    #[test]
    fn test_visit() {
        let mut test = Test { name: String::from(""), other: String::new() };
        {
            let mut fields = test.visit_mut();
            let mut fields = fields.drain(..);
            if let Some(FieldMut { name, value, .. }) = fields.next()  {
                assert_eq!("name", name);
                *value = String::from("hello-world");
            }

            if let Some(FieldMut { name, value, .. }) = fields.next()  {
                assert_eq!("other", name);
                *value = String::from("hello-world-2");
            }
        }
        assert_eq!("hello-world", test.name.as_str());
        assert_eq!("hello-world-2", test.other.as_str());
    }
}