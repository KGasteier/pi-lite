//! Höhen-Cache-Virtualisierung für lange, unterschiedlich hohe Listen (den Chat-Log).
//!
//! Für sehr lange Sitzungen ist es zu teuer, jeden Frame **alle** Log-Einträge
//! (inkl. Markdown-Parsing/-Layout) zu zeichnen, obwohl nur ein Bruchteil
//! davon im sichtbaren Bereich liegt. Dieses Modul implementiert das gängige
//! Muster „nur Sichtbares zeichnen, Rest durch Platzhalter ersetzen“ (wie
//! `ScrollArea::show_rows`, aber für **variabel hohe** Einträge):
//!
//! 1. [`HeightCache`] merkt sich die zuletzt gemessene Höhe jedes Eintrags.
//! 2. [`plan`] bestimmt daraus – zusammen mit dem aktuellen Scroll-Viewport –
//!    welcher Indexbereich tatsächlich gezeichnet werden muss, plus wie viel
//!    Leerraum davor/danach steht, damit Scrollbalken und Scroll-Position so
//!    wirken, als wäre die komplette Liste gezeichnet worden.
//! 3. Der Aufrufer zeichnet nur `plan.first..plan.last`, misst dabei die
//!    tatsächliche Höhe jedes gezeichneten Eintrags und schreibt sie mit
//!    [`HeightCache::set`] zurück – so werden die Schätzwerte mit der Zeit
//!    präzise, auch ohne je die komplette Liste zu rendern.
//!
//! Da nicht sichtbare Einträge gar nicht erst gezeichnet werden, wird für sie
//! insbesondere kein Markdown neu gebaut/layoutet – das ist der Hauptgewinn
//! gegenüber einer normalen `ScrollArea`.
//!
//! Unabhängig von einem konkreten UI-Toolkit verwendbar: Die Typen hier
//! kennen nur `egui::Rect`, keine App-spezifischen Datenstrukturen. Für den
//! Cache-Speicherort siehe [`load`]/[`save`] (nutzt `egui`-Kontextspeicher,
//! kein eigenes App-Feld nötig).

use egui::{Id, Rect, Ui};

/// Alles, dessen Änderung **alle** zwischengespeicherten Höhen auf einmal
/// ungültig macht (im Gegensatz zu Änderungen an einem einzelnen Eintrag,
/// die sich – weil dieser dann ohnehin neu gezeichnet und gemessen wird,
/// siehe Modul-Doku – von selbst korrigieren).
///
/// * **Breite**: Zeilenumbrüche hängen von der verfügbaren Breite ab –
///   ändert sie sich (Fenstergröße, Seitenleiste ein-/ausgeblendet), sind
///   alle alten Höhen falsch.
/// * **Schriftgröße**: bestimmt Zeilenhöhe und Heading-Skalierung.
/// * **„Epoch“**: vom Aufrufer vergebene Kennung, die sich ändert, wenn der
///   komplette Log ausgetauscht wird (z. B. neue/andere Sitzung) – verhindert,
///   dass Höhen eines völlig anderen Gesprächs wiederverwendet werden.
#[derive(Clone, Copy, Default, PartialEq)]
struct CacheKey {
    width_bits: u32,
    font_bits: u32,
    epoch: u64,
}

/// Gemessene Höhen der Log-Einträge, über Frames hinweg gespeichert.
///
/// Neu/ungemessene Einträge liefern `None` aus [`HeightCache::get`]; der
/// Aufrufer nutzt dann in [`plan`] einen Schätzwert, bis der Eintrag das
/// erste Mal tatsächlich gezeichnet wurde.
#[derive(Clone, Default)]
pub struct HeightCache {
    key: CacheKey,
    heights: Vec<Option<f32>>,
}

impl HeightCache {
    /// Zuletzt gemessene Höhe von Eintrag `i`, falls vorhanden.
    pub fn get(&self, i: usize) -> Option<f32> {
        self.heights.get(i).copied().flatten()
    }

    /// Tatsächlich genutzte Höhe von Eintrag `i` nach dem Zeichnen hinterlegen.
    pub fn set(&mut self, i: usize, height: f32) {
        if i >= self.heights.len() {
            self.heights.resize(i + 1, None);
        }
        self.heights[i] = Some(height);
    }

    /// Eintrag `i` zur erneuten Messung beim nächsten Zeichnen vormerken.
    ///
    /// In der Praxis meist unnötig: Ein Eintrag, dessen Inhalt sich ändert
    /// (z. B. Streaming-Text, auf-/zugeklappter Werkzeugblock), wird vom
    /// Nutzer gerade angeschaut und ist damit ohnehin im sichtbaren Bereich
    /// – und wird dadurch automatisch neu gezeichnet und vermessen. Diese
    /// Methode steht für Sonderfälle zur Verfügung, in denen ein Eintrag
    /// unsichtbar verändert wird und seine Schätzhöhe bis zum nächsten
    /// Sichtbarwerden bewusst verworfen werden soll.
    #[allow(dead_code)]
    pub fn invalidate(&mut self, i: usize) {
        if let Some(h) = self.heights.get_mut(i) {
            *h = None;
        }
    }

    /// Anzahl der Einträge, für die aktuell eine Höhe bekannt ist (Tests/Diagnose).
    #[allow(dead_code)]
    pub fn measured_count(&self) -> usize {
        self.heights.iter().filter(|h| h.is_some()).count()
    }

    /// Cache mit der aktuellen Listenlänge und dem Ungültigkeits-Schlüssel
    /// synchronisieren. Vor jedem [`plan`]-Aufruf einmal aufrufen.
    ///
    /// Ändert sich `width`, `font_size` oder `epoch` gegenüber dem letzten
    /// Aufruf, werden **alle** Höhen verworfen (siehe [`CacheKey`]). Wächst
    /// oder schrumpft nur die Länge (z. B. neue Nachricht angehängt), bleiben
    /// die Höhen der unveränderten Einträge erhalten.
    pub fn sync(&mut self, len: usize, width: f32, font_size: f32, epoch: u64) {
        let key = CacheKey { width_bits: width.to_bits(), font_bits: font_size.to_bits(), epoch };
        if key != self.key {
            self.heights.clear();
            self.key = key;
        }
        self.heights.resize(len, None);
    }
}

/// Den geteilten [`HeightCache`] für `id` aus dem egui-Kontext laden (leer,
/// falls noch keiner existiert). Erfordert kein eigenes Feld in der App –
/// der Zustand lebt im egui-Frame-Speicher, genau wie z. B.
/// `CollapsingHeader`-Zustände.
pub fn load(ui: &Ui, id: Id) -> HeightCache {
    ui.ctx().data_mut(|d| d.get_temp::<HeightCache>(id).unwrap_or_default())
}

/// `cache` für den nächsten Frame unter `id` im egui-Kontext ablegen.
pub fn save(ui: &Ui, id: Id, cache: HeightCache) {
    ui.ctx().data_mut(|d| d.insert_temp(id, cache));
}

/// Ergebnis von [`plan`]: welcher Indexbereich tatsächlich gezeichnet werden
/// soll und wie viel Leerraum vor/danach die ausgelassenen Einträge ersetzt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Erster zu zeichnender Index (inklusiv).
    pub first: usize,
    /// Letzter zu zeichnender Index (exklusiv) – Bereich ist `first..last`.
    pub last: usize,
    /// Leerraum (Punkte), der vor `first` eingefügt werden muss, damit die
    /// Scroll-Position stimmt (ersetzt die ausgelassenen Einträge `0..first`
    /// samt ihrer Abstände).
    pub lead_space: f32,
    /// Leerraum (Punkte) nach `last` (ersetzt `last..len`).
    pub trail_space: f32,
    /// Gesamthöhe aller Einträge inkl. Abständen (für Tests/Diagnose).
    pub total_height: f32,
}

/// Bestimmen, welche von `len` Einträgen den (inhaltslokalen) `viewport`
/// schneiden – siehe [`egui::ScrollArea::show_viewport`] – erweitert um
/// `overscan` Punkte auf beiden Seiten, damit Widgets kurz bevor sie ins
/// Bild scrollen schon gemountet sind (kein Leerbild bei schnellem Scrollen).
///
/// Einträge ohne zwischengespeicherte Höhe gehen mit `estimate` ein; das ist
/// der Grund, warum frisch in den sichtbaren Bereich gescrollter Inhalt beim
/// ersten Mal leicht „springen“ kann – ohne vorab alles zu rendern (was
/// Virtualisierung ja gerade vermeiden soll) lässt sich das nicht umgehen.
///
/// `spacing` muss dem vertikalen `item_spacing` entsprechen, mit dem die
/// Einträge tatsächlich gezeichnet werden (z. B. `ui.spacing_mut().item_spacing.y`),
/// damit die berechneten Leerräume exakt passen.
pub fn plan(cache: &HeightCache, len: usize, viewport: Rect, overscan: f32, spacing: f32, estimate: f32) -> Plan {
    if len == 0 {
        return Plan { first: 0, last: 0, lead_space: 0.0, trail_space: 0.0, total_height: 0.0 };
    }
    // offsets[i] = y-Position (oberer Rand) von Eintrag i; offsets[len] = Gesamthöhe.
    let mut offsets = Vec::with_capacity(len + 1);
    let mut y = 0.0f32;
    offsets.push(0.0);
    for i in 0..len {
        y += cache.get(i).unwrap_or(estimate);
        if i + 1 < len {
            y += spacing;
        }
        offsets.push(y);
    }
    let total_height = y;
    let lo = (viewport.min.y - overscan).max(0.0);
    let hi = viewport.max.y + overscan;
    // Erster Eintrag, dessen unterer Rand noch in/nach dem sichtbaren Bereich liegt.
    let first = (0..len).find(|&i| offsets[i + 1] >= lo).unwrap_or(len - 1);
    // Letzter (exklusiver) Eintrag, dessen oberer Rand schon hinter dem sichtbaren Bereich liegt.
    let last = (first..len).find(|&i| offsets[i] > hi).unwrap_or(len).max(first + 1).min(len);
    // Der Abstand zwischen einem Platzhalter und dem ersten echten Eintrag wird vom
    // Layout automatisch als `item_spacing` eingefügt – daher hier einmal abziehen,
    // sonst würde dieser Abstand doppelt gezählt (siehe Modultests).
    let lead_space = if first > 0 { (offsets[first] - spacing).max(0.0) } else { 0.0 };
    let trail_space = if last < len { total_height - offsets[last] } else { 0.0 };
    Plan { first, last, lead_space, trail_space, total_height }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn filled(heights: &[f32]) -> HeightCache {
        let mut c = HeightCache::default();
        c.sync(heights.len(), 800.0, 15.0, 0);
        for (i, h) in heights.iter().enumerate() {
            c.set(i, *h);
        }
        c
    }

    #[test]
    fn leere_liste() {
        let c = HeightCache::default();
        let p = plan(&c, 0, Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)), 0.0, 10.0, 20.0);
        assert_eq!(p, Plan { first: 0, last: 0, lead_space: 0.0, trail_space: 0.0, total_height: 0.0 });
    }

    #[test]
    fn alles_sichtbar_wenn_viewport_grossgenug() {
        let c = filled(&[10.0, 10.0, 10.0]);
        // spacing = 5 -> Gesamthöhe 10+5+10+5+10 = 40
        let p = plan(&c, 3, Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 1000.0)), 0.0, 5.0, 20.0);
        assert_eq!(p.first, 0);
        assert_eq!(p.last, 3);
        assert_eq!(p.lead_space, 0.0);
        assert_eq!(p.trail_space, 0.0);
        assert_eq!(p.total_height, 40.0);
    }

    #[test]
    fn mittlerer_ausschnitt_mit_korrektem_leerraum() {
        let c = filled(&[10.0, 10.0, 10.0]);
        // Viewport sieht nur y in [18, 22) -> nur Eintrag 1 (Bereich [15, 25)) trifft.
        let p = plan(&c, 3, Rect::from_x_y_ranges(0.0..=100.0, 18.0..=22.0), 0.0, 5.0, 20.0);
        assert_eq!(p.first, 1);
        assert_eq!(p.last, 2);
        // Platzhalter vor Eintrag 1 ersetzt [Eintrag0(10) + spacing(5)] minus den vom
        // Layout selbst eingefügten spacing vor Eintrag1 -> 15 - 5 = 10.
        assert_eq!(p.lead_space, 10.0);
        // Platzhalter nach Eintrag 1 ersetzt nur Eintrag2 (10) – der spacing davor kommt
        // automatisch vom Layout.
        assert_eq!(p.trail_space, 10.0);
        assert_eq!(p.total_height, 40.0);
    }

    #[test]
    fn overscan_erweitert_den_bereich() {
        let c = filled(&[10.0, 10.0, 10.0, 10.0, 10.0]);
        // Ohne Overscan: viewport [22,24) trifft nur Eintrag 1 (Bereich [15,25)).
        let tight = plan(&c, 5, Rect::from_x_y_ranges(0.0..=10.0, 22.0..=24.0), 0.0, 5.0, 20.0);
        assert_eq!((tight.first, tight.last), (1, 2));
        // Mit großzügigem Overscan werden auch Nachbarn mitgezeichnet.
        let wide = plan(&c, 5, Rect::from_x_y_ranges(0.0..=10.0, 22.0..=24.0), 20.0, 5.0, 20.0);
        assert!(wide.first < tight.first || wide.last > tight.last);
    }

    #[test]
    fn unbekannte_hoehen_nutzen_schaetzwert() {
        // Nur Eintrag 0 wurde je gemessen (5), der Rest ist unbekannt -> estimate (20) je Eintrag.
        let mut c = HeightCache::default();
        c.sync(3, 800.0, 15.0, 0);
        c.set(0, 5.0);
        let p = plan(&c, 3, Rect::from_x_y_ranges(0.0..=10.0, 0.0..=1000.0), 0.0, 0.0, 20.0);
        // Gesamt: 5 (gemessen) + 20 + 20 (geschätzt) = 45.
        assert_eq!(p.total_height, 45.0);
    }

    #[test]
    fn sync_verwirft_bei_breitenaenderung() {
        let mut c = HeightCache::default();
        c.sync(3, 800.0, 15.0, 0);
        c.set(0, 42.0);
        assert_eq!(c.measured_count(), 1);
        c.sync(3, 801.0, 15.0, 0); // andere Breite -> alles verwerfen
        assert_eq!(c.measured_count(), 0);
        assert_eq!(c.get(0), None);
    }

    #[test]
    fn sync_verwirft_bei_epochenwechsel() {
        let mut c = HeightCache::default();
        c.sync(2, 800.0, 15.0, 1);
        c.set(0, 10.0);
        c.set(1, 10.0);
        c.sync(2, 800.0, 15.0, 2); // neue Sitzung
        assert_eq!(c.measured_count(), 0);
    }

    #[test]
    fn sync_behaelt_hoehen_bei_reiner_laengenaenderung() {
        let mut c = HeightCache::default();
        c.sync(2, 800.0, 15.0, 1);
        c.set(0, 10.0);
        c.set(1, 20.0);
        c.sync(3, 800.0, 15.0, 1); // eine Nachricht angehängt, sonst unverändert
        assert_eq!(c.get(0), Some(10.0));
        assert_eq!(c.get(1), Some(20.0));
        assert_eq!(c.get(2), None);
    }

    #[test]
    fn invalidate_setzt_einzelnen_eintrag_zurueck() {
        let mut c = filled(&[10.0, 10.0]);
        c.invalidate(0);
        assert_eq!(c.get(0), None);
        assert_eq!(c.get(1), Some(10.0));
    }
}
