#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=deploy/r2/common.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/common.sh"
parse_args teardown "$@"
note "Rückbau: $MODE; Slots: $SLOT_LIST; purge=$PURGE. Nur ausdrücklich verwaltete Konten."
# I1-Rückbau archiviert dauerhaft und widerspricht E9. In I2 absichtlich gesperrt.
[[ $MODE != apply ]] || fail 'I1-Rückbau mit Nutzdatenarchiv ist im Pool gesperrt. Erst regulär auschecken; Kontenabbau separat freigeben.'

# Rückbau ist ein wiederaufnehmbarer Ablauf, keine userdel -r-Abkürzung.
# Der Slot bleibt standardmäßig gesperrt erhalten, damit UID/Dateibesitz nicht neu vergeben werden.
if [[ $MODE == apply ]]; then
    require_commands runuser usermod userdel systemctl loginctl tar sha256sum \
        findmnt pgrep flock install getent stat python3
    root_controlled "$ARCHIVE_BASE" || {
        [[ ! -e $ARCHIVE_BASE ]] || fail 'Archivziel nicht rootkontrolliert.'
        root_controlled "$(dirname -- "$ARCHIVE_BASE")" || fail 'Eltern des Archivziels nicht rootkontrolliert.'
    }
    acquire_lock
fi

for slot in "${SELECTED[@]}"; do
    slot_vars "$slot"
    if [[ $MODE == dry-run ]]; then
        note "--- $SLOT ($USER_NAME) ---"
        note 'PLAN: Identität/UID-Inventar und Rückbaufortschritt prüfen; kein fremdes Konto übernehmen'
        note "PLAN: nur $UNIT stoppen/deaktivieren; Gerätetokens extern über S3 widerrufen lassen"
        note "PLAN: nur $USER_NAME sperren (Passwort + Ablauf + nologin); Linger aus; eigenen Usermanager stoppen"
        note 'PLAN: bei verbleibenden Prozessen abbrechen (keine Prozesssuche mit pkill, kein automatisches Töten)'
        note "PLAN: Home + SATA-Daten + Slotkonfiguration + UID als root-only tar unter $ARCHIVE_BASE/$SLOT sichern; SHA-256 + Lesbarkeit prüfen"
        if [[ $PURGE == true ]]; then
            note 'PLAN: erst nach gültigem Archiv und erneuter Pfad-/Mount-/Prozessprüfung Konto + Home + SATA-Daten löschen; Sicherheitslimits/UID-Tombstone erhalten'
        else
            note 'PLAN: Konto bleibt gesperrt, Home/Daten bleiben unverändert; keine Datenlöschung'
        fi
        note 'PLAN: Fortschritt rootverwaltet protokollieren; wiederholter Rückbau verwendet verifiziertes Archiv, keine Überschreibung'
        continue
    fi
    marker=$STATE_BASE/$SLOT.teardown
    archive_dir=$ARCHIVE_BASE/$SLOT
    archive=$archive_dir/snapshot.tar
    [[ -f $STATE_BASE/$SLOT.uid ]] || fail "Kein UID-Inventar für $SLOT; keine Rückbauänderung."
    if [[ -f $marker && $(< "$marker") == purged ]]; then
        [[ -z $UID_NUM && ! -e $SLOT_HOME && ! -e $SLOT_DATA && ! -e $SLOT_ETC ]] || fail "Nach purged existieren wieder Slotkonto/-pfade. Neue Zuordnung manuell prüfen."
        note "OK: $SLOT bereits mit Archiv rückgebaut; keine erneute Änderung."
        continue
    fi
    if [[ -f $marker ]]; then
        case $(< "$marker") in
            preparing|archived|purging) ;;
            *) fail "Unbekannter Rückbaustatus für $SLOT; Recovery nötig." ;;
        esac
    fi
    managed_dir "$ARCHIVE_BASE" 700 root root
    managed_dir "$archive_dir" 700 root root
    if [[ -f $marker && $(< "$marker") == purging ]]; then
        [[ $PURGE == true ]] || fail 'Purge war bereits begonnen; Fortsetzen benötigt --purge.'
        [[ -f $archive_dir/purge-receipt ]] || fail 'Purge-Sicherungsbeleg fehlt; Recovery nötig.'
        purge_file=$(< "$archive_dir/purge-receipt")
        [[ $purge_file =~ ^purge\.[a-zA-Z0-9]+\.tar$ ]] || fail 'Ungültiger Purge-Sicherungsbeleg.'
        (cd -- "$archive_dir" && sha256sum --check --status "$purge_file.sha256") || fail 'Purge-Archiv beschädigt; Recovery nötig.'
        tar --list --file "$archive_dir/$purge_file" >/dev/null || fail 'Purge-Archiv nicht lesbar.'
        if [[ -z $UID_NUM ]]; then
            [[ ! -e $SLOT_HOME && ! -e $SLOT_DATA && ! -e $SLOT_ETC ]] || fail 'Konto fehlt, aber Slotdaten existieren noch; Recovery nötig.'
            managed_file "$marker" 600 root root <<< purged
            note "OK: $SLOT nach bereits erfolgter Kontolöschung abgeschlossen; Archiv erhalten."
            continue
        fi
    fi
    if [[ -z $UID_NUM ]]; then
        fail "Konto $USER_NAME fehlt, aber kein abgeschlossener Rückbau belegt. Keine Daten löschen."
    fi
    # Auch einen bereits begonnenen Rückbau mit shell=nologin ausdrücklich erkennen.
    if [[ -f $marker ]]; then
        record=$(getent passwd "$USER_NAME")
        IFS=: read -r _ _ record_uid _ comment home shell <<< "$record"
        [[ $comment == "cocopi managed slot $SLOT" && $home == "$SLOT_HOME" &&
           $record_uid == "$UID_NUM" && $UID_NUM == "$EXPECTED_UID" && $UID_NUM -ge 1000 &&
           $(id -gn "$USER_NAME") == "$USER_NAME" &&
           $(id -g "$USER_NAME") == "$EXPECTED_UID" &&
           ( $shell == /usr/sbin/nologin || $shell == /bin/bash ) ]] || fail "Rückbaukonto $SLOT weicht vom Inventar ab."
    else
        assert_account
    fi
    # Sicherheitsgrenze nach unten: bereits gefüllte Slotpfade dürfen keine
    # fremd besitzenden Wurzeln haben, auch wenn ein Rückbaumarker existiert.
    for root_path in "$SLOT_HOME" "$SLOT_DATA"; do
        if [[ -e $root_path ]]; then
            [[ -d $root_path && $(stat -c %u "$root_path") == "$UID_NUM" ]] || fail 'Slotwurzel gehört nicht dem bestätigten Konto.'
        fi
    done
    [[ -f $STATE_BASE/$SLOT.uid && $(< "$STATE_BASE/$SLOT.uid") == "$UID_NUM" ]] || fail "UID-Inventar für $SLOT fehlt / weicht ab."
    if [[ ! -f $marker ]]; then
        managed_file "$marker" 600 root root <<< preparing
    fi
    # Geräte-/Enrollment-Widerruf ist vor Wiedervergabe Pflicht. Der lokale Token
    # wird beim Purge archiviert und entfernt; keine Netzwerkaktion im Rückbau.
    if systemctl is-active --quiet "user@$UID_NUM.service"; then
        if user_systemctl is-active --quiet "$UNIT"; then user_systemctl stop "$UNIT"; fi
        if user_systemctl is-enabled --quiet "$UNIT"; then user_systemctl disable "$UNIT"; fi
    fi
    # I2: System-Hub ebenfalls stoppen; Userdienst nur als Altmigrationsschutz.
    if systemctl is-active --quiet "$UNIT"; then systemctl stop "$UNIT"; fi
    if systemctl is-enabled --quiet "$UNIT"; then systemctl disable "$UNIT"; fi
    # Idempotente Sperre, ohne irgendeine fremde UID anzufassen.
    if [[ $(getent shadow "$USER_NAME" | cut -d: -f2) != '!'* ]]; then usermod --lock "$USER_NAME"; fi
    if [[ $(getent passwd "$USER_NAME" | cut -d: -f7) != /usr/sbin/nologin ]]; then
        usermod --shell /usr/sbin/nologin "$USER_NAME"
    fi
    if [[ $(getent shadow "$USER_NAME" | cut -d: -f8) != 1 ]]; then usermod --expiredate 1970-01-02 "$USER_NAME"; fi
    if [[ $(loginctl show-user "$USER_NAME" -p Linger --value 2>/dev/null || true) == yes ]]; then
        loginctl disable-linger "$USER_NAME"
    fi
    if systemctl is-active --quiet "user@$UID_NUM.service"; then systemctl stop "user@$UID_NUM.service"; fi
    if pgrep -u "$UID_NUM" >/dev/null; then
        fail "Noch Prozesse von $USER_NAME aktiv. Kein Archiv/kein Purge; Nutzer informieren und separat geordnet beenden."
    fi
    no_symlinks "$SLOT_HOME" && no_symlinks "$SLOT_DATA" && no_symlinks "$SLOT_ETC" || fail 'Symlink an Rückbaupfad.'
    [[ ! -e $SLOT_ETC || $(stat -c '%a:%U' "$SLOT_ETC") == 755:root ]] || fail 'Slotkonfiguration nicht rootverwaltet.'
    phase=$(< "$marker")
    if [[ $phase != archived && $phase != purging ]]; then
        [[ -d $SLOT_HOME && -d $SLOT_DATA && -d $SLOT_ETC ]] || fail 'Archivquellen unvollständig; erst manuell prüfen.'
    fi
    # Untergeordnete Mounts würden mit --one-file-system nicht gesichert. Dann
    # abbrechen statt ein unvollständiges Archiv zur Löschfreigabe zu machen.
    python3 - "$SLOT_HOME" "$SLOT_DATA" "$SLOT_ETC" <<'MOUNTS'
import os
import sys
with open('/proc/self/mountinfo', encoding='utf-8') as source:
    for line in source:
        mount = line.split()[4]
        for code, char in ((r'\040', ' '), (r'\011', '\t'), (r'\134', '\\')):
            mount = mount.replace(code, char)
        for path in sys.argv[1:]:
            if mount == path or mount.startswith(path + os.sep):
                sys.exit('FEHLER: Untergeordneter Mount; Sicherung/Rückbau separat planen.')
MOUNTS
    if [[ ! -e $archive ]]; then
        no_symlinks "$archive.tmp" || fail 'Symlink am Archivziel.'
        # In root-only Verzeichnis, ohne Dereferenzieren von Symlinks im Home.
        # Fehler (z. B. Platzmangel) lassen Quellen vollständig stehen.
        umask 077
        tar --create --file "$archive.tmp" --numeric-owner --acls --xattrs \
            --one-file-system --directory / -- \
            "${SLOT_HOME#/}" "${SLOT_DATA#/}" "${SLOT_ETC#/}" "${STATE_BASE#/}/$SLOT.uid"
        tar --list --file "$archive.tmp" >/dev/null
        mv -- "$archive.tmp" "$archive"
    else
        [[ -f $archive.sha256 ]] || fail 'Archiv ohne abgeschlossenes Prüfsummenmanifest gefunden; Recovery nötig, nicht als gültig übernehmen.'
    fi
    if [[ ! -f $archive.sha256 ]]; then
        (cd -- "$archive_dir" && sha256sum snapshot.tar > snapshot.tar.sha256)
    fi
    (cd -- "$archive_dir" && sha256sum --check --status snapshot.tar.sha256) || fail 'Archiv-Prüfsumme stimmt nicht; nichts löschen.'
    tar --list --file "$archive" >/dev/null || fail 'Archiv nicht lesbar; nichts löschen.'
    if [[ $phase != purging ]]; then managed_file "$marker" 600 root root <<< archived; fi
    if [[ $PURGE == false && $phase != purging ]]; then
        note "OK: $SLOT gesperrt und archiviert; Home/Daten unverändert. Archiv enthält Geheimnisse: nur root, extern verschlüsselt sichern."
        continue
    fi
    if [[ $PURGE == false ]]; then
        fail 'Purge war bereits begonnen; Fortsetzen benötigt ausdrücklich --purge.'
    fi
    # Direkt vor dem ersten Purge ein zusätzliches, neues Archiv anlegen: auch
    # seit der ersten Sicherung hinzugekommene Dateien sind damit enthalten.
    if [[ $phase != purging ]]; then
        purge_archive=$(mktemp "$archive_dir/purge.XXXXXXXX.tar")
        tar --create --file "$purge_archive" --numeric-owner --acls --xattrs \
            --one-file-system --directory / -- \
            "${SLOT_HOME#/}" "${SLOT_DATA#/}" "${SLOT_ETC#/}" "${STATE_BASE#/}/$SLOT.uid"
        tar --list --file "$purge_archive" >/dev/null
        purge_file=$(basename -- "$purge_archive")
        (cd -- "$archive_dir" && sha256sum "$purge_file" > "$purge_file.sha256")
        (cd -- "$archive_dir" && sha256sum --check --status "$purge_file.sha256")
        managed_file "$archive_dir/purge-receipt" 600 root root <<< "$purge_file"
        managed_file "$marker" 600 root root <<< purging
    fi
    # Kein rm -rf und kein userdel -r: offene/veränderte Pfade oder neue Mounts
    # sollen fehlschlagen. Python löscht nur exakt die beiden bestätigten Wurzeln.
    # Benutzer existiert beim Löschen noch (gesperrt); Prozesse erneut ausschließen.
    ! pgrep -u "$UID_NUM" >/dev/null || fail 'Neue Slotprozesse erkannt; Purge abgebrochen.'
    python3 - "$SLOT_HOME" "$SLOT_DATA" "$UID_NUM" <<'PURGE'
import os
import shutil
import stat
import sys
uid = int(sys.argv[3])
for path in sys.argv[1:3]:
    if not os.path.lexists(path):
        continue  # Wiederaufnahme eines bereits begonnenen Purge.
    data = os.lstat(path)
    if not stat.S_ISDIR(data.st_mode) or data.st_uid != uid or os.path.ismount(path):
        sys.exit('FEHLER: Purge-Pfad nicht mehr eindeutig. Nichts weiter löschen.')
# shutil.rmtree verwendet auf Linux fd-basierte, symlinksichere Traversierung.
if not shutil.rmtree.avoids_symlink_attacks:
    sys.exit('FEHLER: Sichere Löschfunktion nicht verfügbar.')
for path in sys.argv[1:3]:
    if os.path.lexists(path):
        shutil.rmtree(path)
PURGE
    # Nur rootverwaltete Metadaten entfernen; globale Vorlage/Gruppe bleiben für andere Slots.
    if [[ -d $SLOT_ETC ]]; then
        rm -f -- "$SLOT_ETC/token" "$SLOT_ETC/hub.env"
        rmdir -- "$SLOT_ETC"
    fi
    # Neues Konto könnte sonst diese UID erben: feste Slot-IDs werden verwendet;
    # private Gruppe, Inventar und Limits bleiben als zusätzliche Reservierung.
    userdel "$USER_NAME"
    # UID-Tombstone und Slice-/Quota-Grenzen bleiben absichtlich erhalten:
    # unbekannte Restdateien dieser UID könnten sonst durch UID-Recycling zugänglich werden.
    managed_file "$marker" 600 root root <<< purged
    note "OK: $SLOT mit Archiv entfernt. UID-Tombstone/Limits erhalten; kein automatisches UID-Recycling."
done
