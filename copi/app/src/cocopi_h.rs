// Separate Hermes-Ausgabe: gleicher cocopi-Kern, eigener Binary-/Fenstername.
include!("main.rs");

#[cfg(test)]
mod hermes_identity {
    #[test]
    fn separate_application_name() {
        assert_eq!(super::app_name(), "cocopi-h");
    }
}
