//! Gemeinsame Testinfrastruktur für Umgebungsvariablen-Mutationen.
//!
//! **Root cause des flakyen `cargo test -p pi-backend` (paralleler Testlauf):** Mehrere
//! Testmodule (`lib.rs`, `media.rs`, `preview.rs`, `sessions.rs`, `rpc.rs`) setzen
//! prozessweite Umgebungsvariablen (`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `PATH`,
//! `PI_CODING_AGENT_SESSION_DIR`) während der Dauer eines Tests. Jedes dieser Module hatte
//! bisher sein **eigenes, lokales** `static ENV_GUARD: Mutex<()>` – das serialisiert Tests
//! nur *innerhalb* desselben Moduls, nicht aber zwischen den Modulen. Da `cargo test` ohne
//! `--test-threads=1` alle Tests eines Binaries parallel auf mehreren Threads ausführt,
//! konnte z. B. `media::tests::verified_route_without_key_is_unavailable` mitten in seinem
//! `with_temp_home`-Block laufen, während gleichzeitig `preview::tests::...` oder
//! `lib::tests::...` `HOME`/`XDG_CONFIG_HOME` auf ein *anderes* temporäres Verzeichnis setzt
//! und später wieder zurücksetzt – der jeweils andere Test liest dann zwischenzeitlich ein
//! falsches `HOME`, findet seine eigene `media-routes.json`/Cache-Datei nicht am erwarteten
//! Pfad und schlägt mit einem Assertion-Fehler fehl (kein Deadlock, kein Panic im
//! Produktivcode – reiner Daten-Race über eine prozessweite globale Variable, reproduzierbar
//! allein durch `media::`+`lib::`+`preview::` gemeinsam laufen zu lassen, s. Regressionstest
//! unten).
//!
//! Die Lösung ist **ein einziges, crate-weites** `Mutex`, das *alle* env-mutierenden Tests
//! über Modulgrenzen hinweg serialisiert, kombiniert mit einem RAII-Guard, der die alten
//! Werte auch bei einem Panic im Testkörper zuverlässig wiederherstellt (ein bloßes
//! "danach manuell zurücksetzen" nach `f(...)` – wie es vorher in jedem Modul separat
//! gemacht wurde – wird bei einem `panic!`/fehlgeschlagenen `assert!` *übersprungen* und
//! hinterlässt die global gesetzte Variable dauerhaft verändert für alle nachfolgenden
//! Tests. Das war ein zweiter, von der fehlenden Cross-Modul-Serialisierung unabhängiger
//! Bug: ein einmal fehlschlagender Test konnte so sogar nachfolgende, an sich korrekte
//! Tests zum Fehlschlagen bringen).
#![cfg(test)]

use std::sync::{Mutex, MutexGuard};

/// Der einzige Mutex für alle Tests im Crate, die `HOME`, `XDG_CONFIG_HOME`,
/// `XDG_DATA_HOME`, `PATH` oder `PI_CODING_AGENT_SESSION_DIR` verändern. Wird aus jedem
/// Modul über [`EnvGuard::new`] angefragt – es gibt bewusst keine modul-lokalen Kopien
/// mehr, damit *alle* Tests, die Prozessumgebung mutieren, sich gegenseitig ausschließen,
/// unabhängig davon, in welcher Datei sie stehen.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// RAII-Guard, der den crate-weiten [`ENV_LOCK`] für die gesamte Lebensdauer hält und eine
/// beliebige Anzahl von Umgebungsvariablen setzt/entfernt. Stellt beim `Drop` (auch bei
/// einem Panic im Testkörper, dank `catch_unwind`-unabhängiger `Drop`-Semantik von Rust)
/// jede Variable auf ihren vorherigen Wert zurück, bevor der Lock freigegeben wird – kein
/// anderer Thread kann also jemals einen teilweise wiederhergestellten Zustand beobachten.
pub struct EnvGuard {
    saved: Vec<(String, Option<String>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    /// `vars`: Liste von `(name, Some(value))` zum Setzen oder `(name, None)` zum Entfernen.
    /// Nimmt den globalen Lock, merkt sich für jede genannte Variable den *aktuellen* Wert
    /// und wendet dann alle gewünschten Änderungen an. Da der Lock bereits beim Lesen der
    /// Altwerte gehalten wird, gibt es kein Zeitfenster, in dem ein anderer Thread die
    /// Variablen zwischen "Altwert merken" und "neuen Wert setzen" verändern könnte.
    pub fn new(vars: &[(&str, Option<&str>)]) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut saved = Vec::with_capacity(vars.len());
        for (name, value) in vars {
            saved.push(((*name).to_string(), std::env::var(name).ok()));
            match value {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
        Self { saved, _lock: lock }
    }

    /// Bequemlichkeit für den häufigsten Fall: `HOME` auf ein temporäres Verzeichnis
    /// setzen und `XDG_CONFIG_HOME`/`XDG_DATA_HOME` entfernen, damit `dirs::config_dir()`/
    /// `data_dir()` unter Linux nicht versehentlich die echten XDG-Variablen des
    /// Testrechners statt `HOME` lesen.
    pub fn temp_home(home: &std::path::Path) -> Self {
        Self::new(&[("HOME", Some(&home.to_string_lossy())), ("XDG_CONFIG_HOME", None), ("XDG_DATA_HOME", None)])
    }

    /// Wie [`EnvGuard::temp_home`], aber mit zusätzlichen Variablen, die unter *demselben*
    /// gehaltenen Lock gesetzt werden (z. B. ein Test-API-Schlüssel). Wichtig, weil der
    /// zugrunde liegende `Mutex` **nicht reentrant** ist: ein zweiter `EnvGuard::new`-Aufruf
    /// *innerhalb* eines bereits laufenden `with_temp_home`-Blocks würde sich selbst
    /// blockieren (derselbe Thread hält den Lock bereits). Wer HOME und weitere Variablen
    /// gemeinsam braucht, muss sie daher in einem einzigen `EnvGuard`/Aufruf anfordern.
    /// Erzeugt einen Guard, der *nichts* ändert, aber den crate-weiten Lock hält – für
    /// Tests, die lediglich lesend auf aktuell gesetzte Umgebungsvariablen zugreifen (z. B.
    /// `HOME` indirekt über `pi_backend::home()`), aber trotzdem verhindern müssen, dass ein
    /// *anderer* Thread mitten im Lesen `std::env::set_var`/`remove_var` aufruft (in Rust vor
    /// Edition 2024 nicht als `unsafe` markiert, aber bei gleichzeitigem Lesen/Schreiben
    /// derselben Prozessumgebung auf manchen Plattformen ein echter Daten-Race).
    pub fn read_only() -> Self {
        Self::new(&[])
    }

    pub fn temp_home_with(home: &std::path::Path, extra: &[(&str, Option<&str>)]) -> Self {
        let home_str = home.to_string_lossy().into_owned();
        let mut all: Vec<(&str, Option<&str>)> = Vec::with_capacity(3 + extra.len());
        all.push(("HOME", Some(home_str.as_str())));
        all.push(("XDG_CONFIG_HOME", None));
        all.push(("XDG_DATA_HOME", None));
        all.extend_from_slice(extra);
        Self::new(&all)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Umgekehrte Reihenfolge ist für unabhängige Variablennamen irrelevant, aber
        // konsistent mit "zuletzt gesetzt, zuerst zurückgesetzt".
        for (name, old) in self.saved.iter().rev() {
            match old {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
        // `_lock` wird erst danach fallen gelassen (Felder werden in Deklarationsreihenfolge
        // gedroppt, `saved` steht vor `_lock`), die Wiederherstellung passiert also noch
        // unter dem Lock.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_previous_value_even_after_panic_in_body() {
        // Kein verschachtelter Mutex: Ausgangszustand außerhalb des Guards erfassen.
        let before = std::env::var("COPI_TEST_SUPPORT_PANIC_VAR").ok();
        // Innerer Scope simuliert einen verschachtelten Testkörper, der mit einem neuen
        // Wert arbeitet und dann (wie ein fehlschlagender assert!) panict.
        let result = std::panic::catch_unwind(|| {
            let _inner = EnvGuard::new(&[("COPI_TEST_SUPPORT_PANIC_VAR", Some("changed"))]);
            assert_eq!(std::env::var("COPI_TEST_SUPPORT_PANIC_VAR").unwrap(), "changed");
            panic!("simulierter Testfehler mitten in der Änderung");
        });
        assert!(result.is_err());
        // Trotz Panic muss der Wert bereits wieder "original" sein, weil der innere Guard
        // beim Unwind gedroppt wurde, bevor dieser Code hier weiterläuft.
        assert_eq!(std::env::var("COPI_TEST_SUPPORT_PANIC_VAR").ok(), before);
    }

    #[test]
    fn restores_absence_when_variable_was_unset_before() {
        std::env::remove_var("COPI_TEST_SUPPORT_UNSET_VAR");
        {
            let _g = EnvGuard::new(&[("COPI_TEST_SUPPORT_UNSET_VAR", Some("x"))]);
            assert_eq!(std::env::var("COPI_TEST_SUPPORT_UNSET_VAR").unwrap(), "x");
        }
        assert!(std::env::var("COPI_TEST_SUPPORT_UNSET_VAR").is_err());
    }
}
