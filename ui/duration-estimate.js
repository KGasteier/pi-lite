// Lokale, ehrliche Laufzeit-Schätzung für den Busy-Fortschritt (pi-lite).
//
// Es gibt keine serverseitige Fortschrittsmeldung für Agentenläufe – "busy elapsed"
// liefert nur die verstrichene Zeit, keinen tatsächlichen Gesamtfortschritt. Diese
// Datei leitet daraus eine *transparente* Prozentanzeige ab, ohne einen falschen
// Eindruck echten Fortschritts zu erzeugen:
//
//   - Abgeschlossene Läufe (busy -> nicht busy) werden lokal als Dauer (Sekunden)
//     gespeichert, begrenzt auf die letzten 30 Werte (kein unbegrenztes Wachstum,
//     kein Export, keine Übertragung).
//   - Mit genügend Stichproben (Vorgabe: mindestens 3) ist die Schätzung der Median
//     dieser Historie; sonst eine konservative Standardannahme von 180 s.
//   - Bis zur geschätzten Dauer steigt die Anzeige linear bis maximal 90 % – nie bis
//     100 %, damit kein abgeschlossener Lauf suggeriert wird, bevor er es ist.
//   - Nach Überschreiten: verbleibende Dauer längerer beobachteter Läufe, sonst
//     neuer Schätzhorizont. Rücksprünge sind möglich; + signalisiert Überziehung.
//     Keine reale Gesamtfortschrittsmessung und keine zugesicherte Restdauer.
const DURATION_DEFAULT_ESTIMATE_SECONDS = 180;
const DURATION_MIN_SAMPLES_FOR_MEDIAN = 3;
const DURATION_MAX_HISTORY = 30;

function durationMedian(values) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function createDurationEstimate(storage, opts = {}) {
  const key = opts.key || "pi-lite.busy-duration-history.v1";
  const maxHistory = opts.maxHistory || DURATION_MAX_HISTORY;
  const minSamples = opts.minSamples || DURATION_MIN_SAMPLES_FOR_MEDIAN;
  const defaultSeconds = opts.defaultSeconds || DURATION_DEFAULT_ESTIMATE_SECONDS;
  let history = [];
  try {
    const saved = JSON.parse(storage.getItem(key) || "[]");
    if (Array.isArray(saved)) history = saved.filter(n => Number.isFinite(n) && n > 0).slice(-maxHistory);
  } catch (_) { history = []; /* beschädigter Speicher darf die Funktion nicht zerstören */ }
  const persist = () => { try { storage.setItem(key, JSON.stringify(history)); } catch (_) { /* Speichern ist best effort */ } };
  const api = {
    get history() { return [...history]; },
    // Nur tatsächlich abgeschlossene Läufe (busy -> nicht busy) zählen; 0/negative/NaN
    // werden stillschweigend ignoriert statt die Historie zu verfälschen.
    record(seconds) {
      if (!Number.isFinite(seconds) || seconds <= 0) return;
      history.push(seconds);
      if (history.length > maxHistory) history = history.slice(-maxHistory);
      persist();
    },
    estimate() {
      if (history.length >= minSamples) return { seconds: durationMedian(history), source: "median", samples: history.length };
      return { seconds: defaultSeconds, source: "default", samples: history.length };
    },
    // percent: niemals ein behauptetes "fertig", niemals eine fake Plateau-Zahl.
    percent(elapsedSeconds) {
      const est = api.estimate();
      const estimateSeconds = Math.max(1, est.seconds);
      if (elapsedSeconds <= estimateSeconds) {
        const pct = Math.max(0, Math.min(90, (elapsedSeconds / estimateSeconds) * 90));
        return { percent: pct, overdue: false, source: est.source, estimateSeconds, samples: est.samples };
      }
      const longer = history.filter(n => n > elapsedSeconds);
      const remaining = longer.length ? durationMedian(longer.map(n => n - elapsedSeconds)) : estimateSeconds;
      const pct = Math.min(99, 100 * elapsedSeconds / (elapsedSeconds + Math.max(1, remaining)));
      return { percent: pct, overdue: true, source: est.source, estimateSeconds, samples: est.samples };
    },
    label(elapsedSeconds) {
      const r = api.percent(elapsedSeconds), pct = Math.round(r.percent);
      return r.overdue ? `≈${pct}% +` : `≈${pct}%`;
    },
    title(elapsedSeconds) {
      const r = api.percent(elapsedSeconds);
      const basis = r.source === "median"
        ? `Schätzung aus ${r.samples} zuvor abgeschlossenen Läufen (Median, nur lokal, max. 30 Werte).`
        : `Vorläufige Standardschätzung (${Math.round(r.estimateSeconds)} s) – noch zu wenige abgeschlossene Läufe für einen Median.`;
      return r.overdue
        ? basis + " Diese Schätzung ist bereits überschritten. Die tatsächliche Restdauer ist unbekannt; die Prozentzahl zeigt ab hier kein echtes Fortschrittsmaß mehr, und wird anhand längerer Läufe bzw. eines neuen Schätzhorizonts korrigiert. Rücksprünge sind möglich."
        : basis + " Dies ist keine tatsächliche Fortschrittsmessung – nur eine grobe, rein lokale Schätzung anhand früherer Laufzeiten dieser Installation.";
    },
  };
  return api;
}
if (typeof module !== "undefined") module.exports = { createDurationEstimate, durationMedian, DURATION_DEFAULT_ESTIMATE_SECONDS, DURATION_MIN_SAMPLES_FOR_MEDIAN, DURATION_MAX_HISTORY };
