//! Abholfach: nur nach erfolgreicher AEAD-/Hashprüfung konfliktfrei importieren.
//! Die Empfangsbestätigung bleibt ausdrücklich beim aufrufenden Worker.
use super::{
    checkin::{Pickup, ReturnSummary},
    sync_flow::{self, Transfer},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use pi_backend::hub_v1::{abholfach::Schluessel, empfang::Empfang};
use pi_sync::LocalFolder;

pub(super) fn angebot(empfang: &Empfang) -> Result<Option<Pickup>, String> {
    let list = empfang.abholfach()?;
    let boxes = list["archive"]
        .as_array()
        .ok_or("Die Abholfachliste ist unvollständig.")?;
    let Some(first) = boxes.first() else {
        return Ok(None);
    };
    let id = first["checkin_id"]
        .as_str()
        .ok_or("Die Abholfach-ID fehlt.")?;
    // Die Liste enthält keine Größe; die Länge des AEAD-Chiffrats entspricht
    // der Klartextarchivgröße (GCM-Tag liegt separat). Abruf ist nicht quittierend.
    let response = empfang.abholfach_holen(id)?;
    let meta = &response["archiv"];
    if meta["checkin_id"] != id {
        return Err("Das Abholfach gehört nicht zum Angebot.".into());
    }
    Ok(Some(Pickup {
        id: id.into(),
        bytes: URL_SAFE_NO_PAD
            .decode(
                meta["chiffrat"]
                    .as_str()
                    .ok_or("Das verschlüsselte Archiv fehlt.")?,
            )
            .map_err(|_| "Das verschlüsselte Archiv ist beschädigt.")?
            .len() as u64,
        expires: sync_flow::zeitpunkt(meta["ablauf"].as_u64().ok_or("Die Abholfrist fehlt.")?),
    }))
}

pub(super) fn holen(
    empfang: &Empfang,
    key: &Schluessel,
    folder: &LocalFolder,
    id: &str,
    transfer: &Transfer,
) -> Result<(String, ReturnSummary), String> {
    transfer.check()?;
    let response = empfang.abholfach_holen(id)?;
    let meta = &response["archiv"];
    if meta["checkin_id"] != id {
        return Err("Das Abholfach gehört nicht zum Angebot.".into());
    }
    let ciphertext = URL_SAFE_NO_PAD
        .decode(
            meta["chiffrat"]
                .as_str()
                .ok_or("Das verschlüsselte Archiv fehlt.")?,
        )
        .map_err(|_| "Das verschlüsselte Archiv ist beschädigt.")?;
    transfer.check()?;
    let plaintext = key.entschluesseln(meta, &ciphertext).map_err(|e| format!("Dieses Abholfach kann nur der Rechner öffnen, mit dem du eingecheckt warst. Checke an diesem Rechner ein. Nach 72 h wird es gelöscht. ({e})"))?;
    transfer.check()?;
    let sha = meta["sha256"]
        .as_str()
        .ok_or("Die Archiv-Prüfsumme fehlt.")?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let result = folder
        .import(
            sync_flow::Meter {
                io: std::io::Cursor::new(&plaintext),
                transfer,
            },
            sha,
            &format!("abholfach-{sha}-{}-{suffix}.tar.gz", std::process::id()),
            &sync_flow::stamp(),
        )
        .map_err(|e| e.to_string())?;
    sync_flow::journal(
        folder,
        serde_json::json!({"ereignis":"abholfach_importiert","checkin_id":id,"sha256":sha,"dateien":result.dateien,"konflikte":result.konflikte.len()}),
    )?;
    Ok((
        sha.into(),
        ReturnSummary {
            files: result.dateien,
            bytes: plaintext.len() as u64,
            conflicts: result.konflikte.len(),
        },
    ))
}
