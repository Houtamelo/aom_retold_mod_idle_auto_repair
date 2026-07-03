use std::collections::HashSet;

/// Parser context used to decide whether a leading identifier is a type
/// name inside rule/function bodies.
///
/// `primitives` seeds the built-in XS type keywords. `classes` holds
/// caller-supplied user-defined type names (e.g. from workspace class
/// extraction) so local declarations like `BOSystem myBO;` parse as
/// declarations rather than expression statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeTable {
    /// Built-in type keywords: void, int, bool, float, string, vector.
    pub primitives: HashSet<&'static str>,
    /// Caller-supplied class / user-defined type names.
    pub classes: HashSet<String>,
}

impl Default for TypeTable {
    fn default() -> Self {
        Self::with_primitives()
    }
}

impl TypeTable {
    /// Returns a table pre-seeded with the built-in primitive type
    /// keywords. This is the default parser context for callers that do
    /// not yet have a workspace class list.
    pub fn with_primitives() -> Self {
        Self {
            primitives: ["void", "int", "bool", "float", "string", "vector"]
                .into_iter()
                .collect(),
            classes: HashSet::new(),
        }
    }

    /// Adds a caller-supplied class/type name to the table.
    pub fn insert_class(&mut self, name: &str) {
        self.classes.insert(name.to_owned());
    }

    /// Returns `true` when `name` is a known primitive type keyword or
    /// has been inserted as a class name.
    pub fn is_type(&self, name: &str) -> bool {
        self.primitives.contains(name) || self.classes.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_primitives_seeds_builtin_types() {
        let table = TypeTable::with_primitives();
        assert!(table.is_type("void"));
        assert!(table.is_type("int"));
        assert!(table.is_type("bool"));
        assert!(table.is_type("float"));
        assert!(table.is_type("string"));
        assert!(table.is_type("vector"));
    }

    #[test]
    fn default_seeds_primitives() {
        let table = TypeTable::default();
        assert!(table.is_type("int"));
        assert!(table.is_type("vector"));
    }

    #[test]
    fn insert_class_adds_user_type() {
        let mut table = TypeTable::with_primitives();
        assert!(!table.is_type("MyClass"));
        table.insert_class("MyClass");
        assert!(table.is_type("MyClass"));
    }

    #[test]
    fn is_type_false_for_unknown_identifier() {
        let table = TypeTable::with_primitives();
        assert!(!table.is_type("someFunction"));
        assert!(!table.is_type("x"));
    }

    #[test]
    fn empty_table_has_no_types() {
        let table = TypeTable {
            primitives: HashSet::new(),
            classes: HashSet::new(),
        };
        assert!(!table.is_type("int"));
        assert!(!table.is_type("MyClass"));
    }
}
