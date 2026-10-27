#!/usr/bin/env bash
# Prüft, ob copi/cocopi versehentlich als Terminal, Browser oder Standard-App eingetragen ist.
# Dann öffnet z. B. Super+A (Web-App) oder der Bildschirmschoner jedes Mal ein neues Fenster.
# Ohne Argument nur Anzeige; mit --apply werden die falschen Zuordnungen entfernt (Sicherung *.bak-copi).
set -euo pipefail
apply=0
[[ ${1:-} == --apply ]] && apply=1
cfg=${XDG_CONFIG_HOME:-$HOME/.config}
data=${XDG_DATA_HOME:-$HOME/.local/share}
pat='(^|[/"= ;])(co)?copi(\.desktop)?([" ;]|$)'
found=0

say() { printf '%s\n' "$*"; }
backup() { [[ -f $1.bak-copi ]] || cp -p -- "$1" "$1.bak-copi"; }

say "== Desktop-Dateien mit copi/cocopi =="
for d in "$data/applications" /usr/local/share/applications /usr/share/applications; do
  for f in "$d"/*.desktop; do
    [[ -f $f ]] || continue
    if grep -Eq '^Exec=.*(co)?copi' "$f"; then
      say "  $f"
      grep -E '^(Exec|MimeType|Categories|X-TerminalArg|TerminalArg)' "$f" | sed 's/^/      /'
      if grep -Eq '^(MimeType=.|Categories=.*TerminalEmulator)' "$f"; then
        say "    !! Diese Datei meldet sich als Browser/Terminal/Dateiöffner an."
        found=1
        if ((apply)); then
          backup "$f"
          sed -i -E '/^MimeType=/d; s/^(Categories=.*)TerminalEmulator;?/\1/' "$f"
          say "    -> MimeType/TerminalEmulator entfernt."
        fi
      fi
    fi
  done
done

say "== Terminal-Zuordnung (xdg-terminal-exec) =="
for f in "$cfg"/xdg-terminals.list "$cfg"/*-xdg-terminals.list; do
  [[ -f $f ]] || continue
  if grep -Eq "$pat" "$f"; then
    say "  !! $f:"; grep -En "$pat" "$f" | sed 's/^/      /'
    found=1
    if ((apply)); then backup "$f"; sed -i -E "\#$pat#d" "$f"; say "    -> Zeile entfernt."; fi
  else
    say "  ok $f"
  fi
done

say "== Standard-Apps (mimeapps.list) =="
for f in "$cfg/mimeapps.list" "$data/applications/mimeapps.list"; do
  [[ -f $f ]] || continue
  if grep -Eq '(co)?copi\.desktop' "$f"; then
    say "  !! $f:"; grep -En '(co)?copi\.desktop' "$f" | sed 's/^/      /'
    found=1
    if ((apply)); then
      backup "$f"
      # copi-Einträge aus Wertlisten streichen; leer gewordene Zeilen löschen.
      sed -i -E 's/(co)?copi\.desktop;?//g; /^[^=]+=[[:space:]]*$/d' "$f"
      say "    -> Einträge entfernt."
    fi
  else
    say "  ok $f"
  fi
done
if command -v xdg-settings >/dev/null; then
  b=$(xdg-settings get default-web-browser 2>/dev/null || true)
  say "  Standardbrowser: ${b:-?}"
  [[ $b =~ copi ]] && { say "  !! Browser zeigt auf copi – mit xdg-settings set default-web-browser <browser>.desktop korrigieren."; found=1; }
fi

say "== Umgebung =="
for v in TERMINAL BROWSER; do
  val=${!v:-}
  say "  $v=${val:-<leer>}"
  [[ $val =~ copi ]] && { say "  !! $v zeigt auf copi (Shell-/uwsm-Umgebung prüfen)."; found=1; }
done
for f in "$cfg/uwsm/default" "$cfg/uwsm/env" "$cfg/environment.d"/*.conf; do
  [[ -f $f ]] && grep -En '(TERMINAL|BROWSER)=.*copi' "$f" | sed "s|^|  !! $f:|" && found=1
done

say "== Hyprland-Tastenkürzel mit copi =="
if [[ -d $cfg/hypr ]]; then
  if grep -rEn 'copi' "$cfg/hypr" --include='*.conf' --include='*.lua' 2>/dev/null | sed 's/^/  /'; then
    say "  (nur Anzeige – Hyprland-Konfiguration wird nie automatisch geändert)"
  else
    say "  keine"
  fi
fi

say "== Laufende Fenster =="
pgrep -a -x cocopi 2>/dev/null | sed 's/^/  /' || true
pgrep -a -x copi 2>/dev/null | sed 's/^/  /' || true

if ((found)); then
  if ((apply)); then
    command -v update-desktop-database >/dev/null && update-desktop-database "$data/applications" 2>/dev/null || true
    say "Behoben. Danach überzählige cocopi-Fenster normal schließen."
  else
    say "Fehlzuordnung gefunden. Beheben mit: $0 --apply"
  fi
else
  say "Keine Fehlzuordnung gefunden."
fi
