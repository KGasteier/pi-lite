//! Persistentes Abholfach-X25519 und Node-kompatible Entschlüsselung.
//! Blocking im Worker; keine Dateiübernahme und keine Empfangsbestätigung hier.
use super::empfang::canonical_json;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use curve25519_dalek::montgomery::MontgomeryPoint;
use ring::{
    aead, digest, hkdf,
    rand::{SecureRandom, SystemRandom},
};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ARCHIV: usize = 2 * 1024 * 1024 * 1024;
const VERFAHREN: &str = "X25519-HKDF-SHA256-AES-256-GCM";

/// Kein Debug/Clone/privater Export. Nur dieser öffentliche Schlüssel geht zum Empfang.
pub struct Schluessel {
    secret: [u8; 32],
    pub public_b64url: String,
}
impl Schluessel {
    /// `dir` ist ein lokaler privater Ordner, nie ein Netzpfad.
    /// Datei: 32 rohe Zufallsbytes, `abholfach.x25519`, Unix 0600.
    /// Veröffentlichung per atomarem Hardlink: parallele Erzeuger überschreiben nie.
    pub fn laden_oder_erzeugen(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref();
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(dir)
            .map_err(|_| "Der Schlüsselordner konnte nicht angelegt werden.")?;
        let meta = fs::symlink_metadata(dir).map_err(|_| "Der Schlüsselordner fehlt.")?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Der Schlüsselordner darf keine Verknüpfung sein.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(
                    "Der Schlüsselordner ist nicht privat. Er benötigt Rechte 0700.".into(),
                );
            }
        }
        let path = dir.join("abholfach.x25519");
        match fs::symlink_metadata(&path) {
            Ok(_) => return Self::laden(&path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Die Schlüsseldatei kann nicht geprüft werden.".into()),
        }
        let mut secret = [0; 32];
        let mut suffix = [0; 16];
        let random = SystemRandom::new();
        random
            .fill(&mut secret)
            .and_then(|_| random.fill(&mut suffix))
            .map_err(|_| "Der sichere Zufallsgenerator ist nicht verfügbar.")?;
        let temp = dir.join(format!(".abholfach-{}.neu", B64.encode(suffix)));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|_| "Die Schlüsseldatei konnte nicht angelegt werden.")?;
        let result: Result<(), String> = (|| {
            file.write_all(&secret)
                .and_then(|_| file.sync_all())
                .map_err(|_| "Die Schlüsseldatei konnte nicht sicher gespeichert werden.")?;
            drop(file);
            match fs::hard_link(&temp, &path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => {
                    return Err("Die Schlüsseldatei konnte nicht atomar übernommen werden.".into());
                }
            }
            Ok(())
        })();
        // Nur eigene, zufällig benannte temporäre Datei entfernen.
        let cleanup = fs::remove_file(&temp);
        result?;
        cleanup.map_err(|_| "Die temporäre Schlüsseldatei konnte nicht entfernt werden.")?;
        #[cfg(unix)]
        File::open(dir)
            .and_then(|d| d.sync_all())
            .map_err(|_| "Der Schlüsselordner konnte nicht sicher gespeichert werden.")?;
        Self::laden(&path)
    }
    fn laden(path: &Path) -> Result<Self, String> {
        let before = fs::symlink_metadata(path).map_err(|_| "Die Schlüsseldatei fehlt.")?;
        if !before.is_file() || before.file_type().is_symlink() || before.len() != 32 {
            return Err(
                "Die Schlüsseldatei ist beschädigt oder eine Verknüpfung. Sie wurde nicht ersetzt."
                    .into(),
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if before.permissions().mode() & 0o777 != 0o600 {
                return Err(
                    "Die Schlüsseldatei ist nicht privat. Sie benötigt Rechte 0600.".into(),
                );
            }
        }
        let mut file =
            File::open(path).map_err(|_| "Die Schlüsseldatei konnte nicht geöffnet werden.")?;
        let after = file
            .metadata()
            .map_err(|_| "Die Schlüsseldatei konnte nicht geprüft werden.")?;
        if !after.is_file() || after.len() != 32 {
            return Err("Die Schlüsseldatei hat sich beim Öffnen verändert.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if before.dev() != after.dev()
                || before.ino() != after.ino()
                || after.permissions().mode() & 0o777 != 0o600
                || after.uid() != before.uid()
            {
                return Err("Die Schlüsseldatei hat sich beim Öffnen verändert.".into());
            }
        }
        let mut secret = [0; 32];
        file.read_exact(&mut secret)
            .map_err(|_| "Die Schlüsseldatei ist unvollständig.")?;
        let public_b64url = B64.encode(MontgomeryPoint::mul_base_clamped(secret).to_bytes());
        Ok(Self {
            secret,
            public_b64url,
        })
    }
    /// `meta_json`: Empfangs-`archiv` (mit `tag`, optional `chiffrat`).
    /// `ciphertext`: base64url-dekodiertes `chiffrat`, OHNE GCM-Tag.
    /// Prüft Ablauf in Unix-Millisekunden, Version, Verfahren, AEAD und Klartext-SHA256.
    /// Erst Ok erlaubt Rückspielen; `abholfach_erhalten` erst nach dessen Erfolg.
    pub fn entschluesseln(&self, meta_json: &Value, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "Die lokale Uhr ist ungültig.")?
            .as_millis();
        self.entschluesseln_zu(meta_json, ciphertext, now)
    }
    fn entschluesseln_zu(
        &self,
        meta_json: &Value,
        ciphertext: &[u8],
        now_ms: u128,
    ) -> Result<Vec<u8>, String> {
        let mut meta = meta_json
            .as_object()
            .cloned()
            .ok_or("Die Archivangaben sind unvollständig.")?;
        let tag = meta.remove("tag").ok_or("Die Archivbestätigung fehlt.")?;
        meta.remove("chiffrat"); // Genau Node: tag/chiffrat sind nicht Bestandteil der AAD.
        let meta = Value::Object(meta);
        if meta["v"] != 1
            || meta["verfahren"] != VERFAHREN
            || meta["ablauf"]
                .as_u64()
                .is_none_or(|t| u128::from(t) <= now_ms)
            || ciphertext.len() > MAX_ARCHIV
        {
            return Err("Das Abholfach ist abgelaufen, zu groß oder unbekannt.".into());
        }
        let aad = canonical_json(&meta)?;
        let peer = decode::<32>(&meta["ephemeral_pubkey"])?;
        let salt = decode::<32>(&meta["salt"])?;
        let iv = decode::<12>(&meta["iv"])?;
        let tag = decode::<16>(&tag)?;
        // RFC 7748: oberstes Eingabebit ignorieren; Dalek erwartet es bereits maskiert.
        let mut peer = peer;
        peer[31] &= 127;
        let shared = MontgomeryPoint(peer).mul_clamped(self.secret).to_bytes();
        if shared == [0; 32] {
            return Err("Der Abholfach-Schlüsselaustausch ist ungültig.".into());
        }
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt).extract(&shared);
        let info: &[&[u8]] = &[b"cocopi-abholfach-v1"];
        let okm = prk
            .expand(info, &aead::AES_256_GCM)
            .map_err(|_| "Der Archivschlüssel konnte nicht abgeleitet werden.")?;
        let key = aead::LessSafeKey::new(aead::UnboundKey::from(okm));
        let mut data = Vec::with_capacity(ciphertext.len() + 16);
        data.extend_from_slice(ciphertext);
        data.extend_from_slice(&tag);
        let plaintext = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(iv),
                aead::Aad::from(aad.as_bytes()),
                &mut data,
            )
            .map_err(|_| "Das Archiv wurde verändert oder gehört zu einem anderen Schlüssel.")?;
        let expected = meta["sha256"]
            .as_str()
            .ok_or("Die Archiv-Prüfsumme fehlt.")?;
        let actual: String = digest::digest(&digest::SHA256, plaintext)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if expected != actual {
            return Err("Das Archiv ist beschädigt. Die Prüfsumme stimmt nicht.".into());
        }
        let len = plaintext.len();
        data.truncate(len);
        Ok(data)
    }
}
fn decode<const N: usize>(value: &Value) -> Result<[u8; N], String> {
    B64.decode(
        value
            .as_str()
            .ok_or("Die Archivangaben sind unvollständig.")?,
    )
    .map_err(|_| "Die Archivangaben sind ungültig.")?
    .try_into()
    .map_err(|_| "Die Archivangaben haben eine ungültige Länge.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{path::PathBuf, process::Command};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let mut suffix = [0; 16];
            SystemRandom::new().fill(&mut suffix).unwrap();
            Self(std::env::temp_dir().join(format!("cocopi-abholfach-{}", B64.encode(suffix))))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn key_persistence_private_atomic_and_corruption_never_replaced() {
        let dir = Temp::new();
        let key = Schluessel::laden_oder_erzeugen(&dir.0).unwrap();
        let file = dir.0.join("abholfach.x25519");
        let original = fs::read(&file).unwrap();
        assert_eq!(original.len(), 32);
        assert_eq!(B64.decode(&key.public_b64url).unwrap().len(), 32);
        assert_eq!(
            Schluessel::laden_oder_erzeugen(&dir.0)
                .unwrap()
                .public_b64url,
            key.public_b64url
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
                0o700
            );
            fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(Schluessel::laden_oder_erzeugen(&dir.0).is_err());
            assert_eq!(fs::read(&file).unwrap(), original);
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::write(&file, b"beschaedigt").unwrap();
        assert!(Schluessel::laden_oder_erzeugen(&dir.0).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"beschaedigt");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[test]
    fn parallel_key_creation_keeps_one_complete_key() {
        let dir = Temp::new();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let path = dir.0.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Schluessel::laden_oder_erzeugen(path).unwrap().public_b64url
                })
            })
            .collect();
        let keys: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(keys.iter().all(|k| k == &keys[0]));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_file_and_directory_and_public_directory_are_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = Temp::new();
        let link_dir = Temp::new();
        Schluessel::laden_oder_erzeugen(&dir.0).unwrap();
        symlink(&dir.0, &link_dir.0).unwrap();
        assert!(Schluessel::laden_oder_erzeugen(&link_dir.0).is_err());
        fs::remove_file(&link_dir.0).unwrap();
        let file = dir.0.join("abholfach.x25519");
        let real = dir.0.join("original");
        fs::rename(&file, &real).unwrap();
        symlink(&real, &file).unwrap();
        assert!(Schluessel::laden_oder_erzeugen(&dir.0).is_err());
        fs::remove_file(&file).unwrap();
        fs::rename(&real, &file).unwrap();
        fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Schluessel::laden_oder_erzeugen(&dir.0).is_err());
    }
    #[test]
    fn x25519_wrapper_matches_rfc7748() {
        fn hex(s: &str) -> [u8; 32] {
            std::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        }
        let alice = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let alice_public = hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        let bob_public = hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let shared = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(
            MontgomeryPoint::mul_base_clamped(alice).to_bytes(),
            alice_public
        );
        assert_eq!(
            MontgomeryPoint(bob_public).mul_clamped(alice).to_bytes(),
            shared
        );
    }
    #[test]
    fn null_ecdh_invalid_format_and_expiry_are_rejected() {
        let dir = Temp::new();
        let key = Schluessel::laden_oder_erzeugen(&dir.0).unwrap();
        let mut meta = json!({"v":1,"verfahren":VERFAHREN,"checkin_id":"test","ablauf":1001,
            "ephemeral_pubkey":B64.encode([0;32]),"salt":B64.encode([0;32]),"iv":B64.encode([0;12]),
            "sha256":"0".repeat(64),"tag":B64.encode([0;16])});
        for peer in [[0; 32], {
            let mut p = [0; 32];
            p[0] = 1;
            p
        }] {
            meta["ephemeral_pubkey"] = json!(B64.encode(peer));
            assert!(
                key.entschluesseln_zu(&meta, b"", 1000)
                    .unwrap_err()
                    .contains("Schlüsselaustausch")
            );
        }
        assert!(
            key.entschluesseln_zu(&meta, b"", 1001)
                .unwrap_err()
                .contains("abgelaufen")
        );
        for (field, value) in [
            ("v", json!(2)),
            ("verfahren", json!("fremd")),
            ("ablauf", json!(-1)),
            ("ablauf", json!(1.5)),
            ("tag", json!("kaputt")),
        ] {
            let mut bad = meta.clone();
            bad[field] = value;
            assert!(key.entschluesseln_zu(&bad, b"", 1000).is_err());
        }
    }
    /// Node nutzt das tatsächliche S7-Modul, keinen nachgebauten Verschlüsseler.
    /// Keine Dienste, Konten oder Modellaufrufe. Fehlt Node: expliziter Skip.
    #[test]
    fn node_encrypts_rust_decrypts_and_rejects_tampering_wrong_key_expiry() {
        if Command::new("node").arg("--version").output().is_err() {
            eprintln!("Abholfach-Cross-Test übersprungen: node fehlt.");
            return;
        }
        let hub = std::env::var_os("COCOPI_PI_HUB")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/medienhaus/Work/pi-hub"));
        let module = hub.join("empfang/ausweis.mjs");
        assert!(
            module.is_file(),
            "COCOPI_PI_HUB muss empfang/ausweis.mjs enthalten: {}",
            module.display()
        );
        let dir = Temp::new();
        let wrong_dir = Temp::new();
        let key = Schluessel::laden_oder_erzeugen(&dir.0).unwrap();
        let wrong_key = Schluessel::laden_oder_erzeugen(&wrong_dir.0).unwrap();
        let out = Command::new("node").args(["--input-type=module", "-e", r#"
            import {pathToFileURL} from 'node:url';
            import crypto from 'node:crypto';
            const {encryptArchive, canonical} = await import(pathToFileURL(process.argv[1]).href);
            const bytes = Buffer.concat([Buffer.from('Grüße aus R2\n'),Buffer.from([0,255,13,10])]);
            const fresh = encryptArchive(bytes, process.argv[2], '12345678-1234-1234-1234-123456789abc');
            const expired = encryptArchive(bytes, process.argv[2], fresh.checkin_id, Date.now()-73*3600e3);
            // Gültiges AEAD mit absichtlich falscher Klartext-Prüfsumme: Hashprüfung separat nachweisen.
            const badHash = (() => {
              const ep = crypto.generateKeyPairSync('x25519');
              const pub = crypto.createPublicKey({format:'der',type:'spki',key:Buffer.concat([Buffer.from('302a300506032b656e032100','hex'),Buffer.from(process.argv[2],'base64url')])});
              const salt=crypto.randomBytes(32),iv=crypto.randomBytes(12);
              const key=crypto.hkdfSync('sha256',crypto.diffieHellman({privateKey:ep.privateKey,publicKey:pub}),salt,'cocopi-abholfach-v1',32);
              const meta={v:1,verfahren:fresh.verfahren,checkin_id:fresh.checkin_id,ablauf:fresh.ablauf,ephemeral_pubkey:ep.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64url'),salt:salt.toString('base64url'),iv:iv.toString('base64url'),sha256:'0'.repeat(64)};
              const cipher=crypto.createCipheriv('aes-256-gcm',key,iv);cipher.setAAD(Buffer.from(canonical(meta)));
              const chiffrat=Buffer.concat([cipher.update(bytes),cipher.final()]).toString('base64url');
              return {...meta,tag:cipher.getAuthTag().toString('base64url'),chiffrat};
            })();
            console.log(JSON.stringify({fresh,expired,badHash,plaintext:bytes.toString('base64url')}));
        "#]).arg(&module).arg(&key.public_b64url).output().unwrap();
        assert!(
            out.status.success(),
            "Node: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        let envelope = &result["fresh"];
        let ciphertext = B64.decode(envelope["chiffrat"].as_str().unwrap()).unwrap();
        let expected = B64.decode(result["plaintext"].as_str().unwrap()).unwrap();
        assert_eq!(key.entschluesseln(envelope, &ciphertext).unwrap(), expected);
        // Auch nach Neustart/Neuladen identischer dauerhafter Schlüssel.
        let reloaded = Schluessel::laden_oder_erzeugen(&dir.0).unwrap();
        assert_eq!(
            reloaded.entschluesseln(envelope, &ciphertext).unwrap(),
            expected
        );
        assert!(wrong_key.entschluesseln(envelope, &ciphertext).is_err());
        let mut tampered = envelope.clone();
        tampered["checkin_id"] = json!("anderer-checkin");
        assert!(key.entschluesseln(&tampered, &ciphertext).is_err());
        let mut tampered = envelope.clone();
        tampered["ablauf"] = json!(envelope["ablauf"].as_u64().unwrap() + 1000);
        assert!(key.entschluesseln(&tampered, &ciphertext).is_err());
        let mut corrupted = ciphertext.clone();
        corrupted[0] ^= 1;
        assert!(key.entschluesseln(envelope, &corrupted).is_err());
        let mut tampered = envelope.clone();
        tampered["tag"] = json!(B64.encode([0; 16]));
        assert!(key.entschluesseln(&tampered, &ciphertext).is_err());
        let expired = &result["expired"];
        let expired_cipher = B64.decode(expired["chiffrat"].as_str().unwrap()).unwrap();
        assert!(
            key.entschluesseln(expired, &expired_cipher)
                .unwrap_err()
                .contains("abgelaufen")
        );
        let bad_hash = &result["badHash"];
        let hash_cipher = B64.decode(bad_hash["chiffrat"].as_str().unwrap()).unwrap();
        assert!(
            key.entschluesseln(bad_hash, &hash_cipher)
                .unwrap_err()
                .contains("Prüfsumme")
        );
    }
}
