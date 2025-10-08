//! Assignment 8: implement `btreemap!` declarative macro and `btreemap_proc!` proc-macro wrapper.

#[macro_export]
macro_rules! btreemap {
    ( $( $k:expr => $v:expr ),* $(,)? ) => {
        {
            let mut map = ::std::collections::BTreeMap::new();
            $( map.insert($k, $v); )*
            map
        }
    };
}

// Re-export the proc-macro under a macro_rules wrapper to make usage similar
pub use assignment_8_macros::btreemap_proc as btreemap_proc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarative_macro_creates_map() {
        let m = btreemap!{
            1 => "one",
            2 => "two",
        };
        assert_eq!(m.get(&1), Some(&"one"));
        assert_eq!(m.get(&2), Some(&"two"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn proc_macro_creates_map() {
        // the proc macro expands to an expression creating a map
        let m: ::std::collections::BTreeMap<i32, &str> = btreemap_proc!{
            10 => "ten",
            20 => "twenty",
        };
        assert_eq!(m.get(&10), Some(&"ten"));
        assert_eq!(m.get(&20), Some(&"twenty"));
    }

    #[test]
    fn complex_values() {
        let m = btreemap!{
            "a" => vec![1,2,3],
            "b" => vec![4,5],
        };
        assert_eq!(m.get(&"a").unwrap().len(), 3);
        assert_eq!(m.get(&"b").unwrap().len(), 2);
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let m = btreemap!{
            1 => "one",
            1 => "uno",
        };
        assert_eq!(m.get(&1), Some(&"uno"));
    }

    #[test]
    fn proc_macro_with_macro_expressions() {
        macro_rules! make_val { ($x:expr) => { concat!("v", $x) }; }
        let m: ::std::collections::BTreeMap<i32, &str> = btreemap_proc!{
            5 => make_val!("5"),
            6 => "six",
        };
        assert!(m.get(&5).is_some());
        assert_eq!(m.get(&6), Some(&"six"));
    }

    #[test]
    fn grouping_and_nested() {
        let m = btreemap!{
            ("a", 1) => vec![1,2],
            ("b", 2) => {
                let mut v = Vec::new();
                v.push(3);
                v
            },
        };
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn key_with_side_effect() {
        use std::cell::Cell;
        thread_local! {
            static COUNTER: Cell<i32> = Cell::new(0);
        }

        fn next() -> i32 {
            COUNTER.with(|c| { let v = c.get() + 1; c.set(v); v })
        }

        let m = btreemap!{
            next() => "a",
            next() => "b",
        };

        // keys should be distinct (1 and 2)
        assert!(m.get(&1).is_some());
        assert!(m.get(&2).is_some());
    }

    // If trybuild is available, we could add compile-fail tests for non-Ord keys. Skip here if not.
    // Example: building a file that uses non-Ord key should fail to compile when using BTreeMap.
}
