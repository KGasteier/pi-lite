//! Acht feste Bookmark-Standardfarben für Session-Lesezeichen (copi).
//!
//! Getrennt von `theme.rs` (Kategorie-/UI-Farben), weil diese Palette ausschließlich für
//! Bookmark-Farbindizes gilt, die in `Store::bookmark_colors` persistiert werden. Die Werte
//! sind bewusst 1:1 identisch zu `ui/bookmark-palette.js` (gleiche Hex-Codes, gleiche
//! Reihenfolge, gleiche deutsche Namen). Die Integration lebt in den Session-Karten
//! von `view.rs` und im Farbwähler von `ui/app.js`.
//!
//! Index `0` ist die Standardfarbe neuer Bookmarks (entspricht der bisherigen, einfarbigen
//! Kategoriefarbe `CAT`), Indizes `1..=7` sind frei wählbare Zusatzfarben. Ein ungültiger
//! gespeicherter Index (z. B. durch manuell editierte `state.json` oder eine künftig
//! verkleinerte Palette) fällt sicher auf Index `0` zurück statt zu einem Panic zu führen.
use egui::Color32;

/// Ein Palette-Eintrag: deutscher Anzeigename, Hex-Code (wie in `ui/bookmark-palette.js`) und
/// die daraus geparste `Color32` für egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookmarkColor {
    pub name: &'static str,
    pub hex: &'static str,
}

/// Die acht festen Standardfarben, Index 0 = Vorgabe. Reihenfolge ist Teil der gespeicherten
/// Bedeutung (Index in `Store::bookmark_colors`) – **nicht** umsortieren, nur anhängen wäre
/// ohnehin durch die feste Länge 8 ausgeschlossen; stattdessen Hex-Werte nur gemeinsam mit
/// `ui/bookmark-palette.js` ändern.
pub const PALETTE: [BookmarkColor; 8] = [
    BookmarkColor { name: "Blau", hex: "#6aa5e8" },
    BookmarkColor { name: "Grün", hex: "#5ec2a0" },
    BookmarkColor { name: "Gelb", hex: "#e0b25a" },
    BookmarkColor { name: "Violett", hex: "#b08cf0" },
    BookmarkColor { name: "Orange", hex: "#e68f6e" },
    BookmarkColor { name: "Rot", hex: "#ef476f" },
    BookmarkColor { name: "Türkis", hex: "#18a999" },
    BookmarkColor { name: "Pink", hex: "#e454c4" },
];

/// Anzahl der Farben in [`PALETTE`] (8). Zentrale Konstante statt verstreuter Literale.
pub const LEN: u8 = PALETTE.len() as u8;

/// Normalisiert einen (ggf. aus gespeicherten Daten stammenden, potenziell ungültigen)
/// Farbindex auf einen gültigen Palette-Index: alles `>= LEN` fällt auf `0` zurück.
pub fn normalize(index: u8) -> u8 {
    if index < LEN {
        index
    } else {
        0
    }
}

/// `Color32` für einen (bereits normalisierten oder nicht) Index; normalisiert intern.
///
/// Farbwert für die Session-Karten und den Farbwähler.
#[allow(dead_code)]
pub fn color(index: u8) -> Color32 {
    parse_hex(PALETTE[normalize(index) as usize].hex)
}

/// Anzeigename für einen Index; normalisiert intern. `#[allow(dead_code)]`: siehe [`color`].
#[allow(dead_code)]
pub fn name(index: u8) -> &'static str {
    PALETTE[normalize(index) as usize].name
}

/// Parst einen `#rrggbb`-Hex-Code zu `Color32`. Interne Hilfsfunktion; nur für die fest
/// eingebauten Palette-Konstanten gedacht (kein allgemeiner Farbparser für Nutzereingaben).
fn parse_hex(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    debug_assert_eq!(h.len(), 6, "Palette-Hex-Code muss #rrggbb sein: {hex:?}");
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    Color32::from_rgb(byte(0), byte(2), byte(4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_exactly_eight_entries() {
        assert_eq!(PALETTE.len(), 8);
        assert_eq!(LEN, 8);
    }

    #[test]
    fn palette_entries_have_unique_names_and_valid_hex() {
        let mut names = std::collections::HashSet::new();
        for c in &PALETTE {
            assert!(names.insert(c.name), "doppelter Name: {}", c.name);
            assert!(c.hex.starts_with('#') && c.hex.len() == 7, "ungültiger Hex-Code: {}", c.hex);
            assert!(c.hex[1..].chars().all(|ch| ch.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn normalize_passes_through_valid_indices() {
        for i in 0..LEN {
            assert_eq!(normalize(i), i);
        }
    }

    #[test]
    fn normalize_falls_back_to_zero_for_out_of_range_index() {
        assert_eq!(normalize(8), 0);
        assert_eq!(normalize(255), 0);
    }

    #[test]
    fn color_parses_expected_rgb_for_first_entry() {
        // "#6aa5e8" -> (0x6a, 0xa5, 0xe8)
        let c = color(0);
        assert_eq!((c.r(), c.g(), c.b()), (0x6a, 0xa5, 0xe8));
    }

    #[test]
    fn color_and_name_fall_back_for_out_of_range_index() {
        assert_eq!(color(99), color(0));
        assert_eq!(name(99), name(0));
    }

    #[test]
    fn all_palette_colors_round_trip_distinctly() {
        // Keine zwei Paletteneinträge dürfen auf exakt dieselbe Farbe fallen (sonst wäre
        // die Farbauswahl für Nutzer visuell nicht unterscheidbar).
        let colors: Vec<Color32> = (0..LEN).map(color).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Farben {i} und {j} sind identisch");
            }
        }
    }
}
