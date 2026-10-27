//! Dauerhafte Geräteidentität (32 Byte roh, Unix 0600) und Sicherheitswörter.
//! Die Ablage ist nicht verschlüsselt und ersetzt keine System-Schlüsselverwaltung.
use crate::model::PeerId;

pub struct Identity {
    pub secret: iroh::SecretKey,
}

impl Identity {
    /// Bestehende Schlüssel nur mit privaten Rechten laden; neue Ordner mit 0700.
    /// Eine gleichzeitig erzeugte Identität wird niemals überschrieben.
    pub fn load_or_create(path: &std::path::Path) -> Result<Identity, String> {
        use std::io::{Read, Write};
        fn load(path: &std::path::Path) -> Result<Identity, String> {
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|e| format!("Schlüssel nicht lesbar: {e}"))?;
            if !metadata.file_type().is_file() {
                return Err("Schlüssel muss eine reguläre Datei sein (kein Verweis)".into());
            }
            let mut file =
                std::fs::File::open(path).map_err(|e| format!("Schlüssel nicht lesbar: {e}"))?;
            let metadata = file
                .metadata()
                .map_err(|e| format!("Schlüsselrechte nicht lesbar: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err("Unsichere Schlüsselrechte: Gruppe/Andere dürfen keine Rechte haben; bitte chmod 600 verwenden".into());
                }
            }
            if metadata.len() != 32 {
                return Err("Schlüsseldatei muss genau 32 Byte lang sein".into());
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(33)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Schlüssel nicht lesbar: {e}"))?;
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| "Schlüsseldatei muss genau 32 Byte lang sein")?;
            Ok(Identity {
                secret: iroh::SecretKey::from_bytes(&bytes),
            })
        }
        match std::fs::symlink_metadata(path) {
            Ok(_) => return load(path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Schlüssel nicht lesbar: {e}")),
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        let mut dirs = std::fs::DirBuilder::new();
        dirs.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            dirs.mode(0o700);
        }
        dirs.create(parent)
            .map_err(|e| format!("Schlüsselordner konnte nicht angelegt werden: {e}"))?;
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| format!("Kein sicherer Zufall verfügbar: {e}"))?;
        let mut name = path.as_os_str().to_os_string();
        name.push(".new");
        let tmp = std::path::PathBuf::from(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|e| format!("Temporärer Schlüssel konnte nicht angelegt werden: {e}"))?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            // Atomar veröffentlichen, aber anders als rename nie eine vorhandene
            // Identität ersetzen (auch nicht beim gleichzeitigen ersten Start).
            std::fs::hard_link(&tmp, path)?;
            std::fs::File::open(parent)?.sync_all()
        })();
        let _ = std::fs::remove_file(&tmp);
        if let Err(e) = result {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(format!(
                    "Schlüssel konnte nicht atomar gespeichert werden: {e}"
                ));
            }
        }
        load(path)
    }
    /// Nur für Tests/Demo: flüchtig.
    pub fn ephemeral() -> Identity {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).expect("Zufall");
        Identity {
            secret: iroh::SecretKey::from_bytes(&b),
        }
    }
    pub fn peer(&self) -> PeerId {
        peer_of(&self.secret.public())
    }
}

pub fn peer_of(pk: &iroh::PublicKey) -> PeerId {
    PeerId(pk.to_string())
}

/// Acht Sicherheitswörter aus 256 Wörtern: 64 Bit Vergleichsstärke, nicht 66 Bit.
/// BLAKE3 bindet beide sortierten IDs an den festen Kontext `cocopi-safety-v1`.
/// Alle acht Wörter über einen vertrauenswürdigen Kanal vergleichen; Kurz-IDs
/// dienen nur der Anzeige und sind kein Ersatz dafür.
pub fn safety_words(a: &PeerId, b: &PeerId) -> Vec<&'static str> {
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let mut input = b"cocopi-safety-v1".to_vec();
    input.extend_from_slice(a.as_str().as_bytes());
    input.extend_from_slice(b.as_str().as_bytes());
    let hash = blake3::hash(&input);
    hash.as_bytes()[..8]
        .iter()
        .map(|&i| WORDS[i as usize])
        .collect()
}

// Eigene Liste kurzer, in der Anzeige unterscheidbarer ASCII-Wörter. Die Reihenfolge
// gehört zum Protokoll und darf nach Austausch von Sicherheitswörtern nicht wechseln.
const WORDS: [&str; 256] = [
    "aal", "abend", "acker", "adler", "ahorn", "akte", "alarm", "alge", "alm", "ampel", "amsel",
    "angel", "anker", "apfel", "arena", "arm", "ast", "atlas", "auge", "auto", "bach", "bad",
    "bagger", "bahn", "balken", "ball", "band", "bank", "bart", "bass", "bauch", "baum", "becher",
    "beet", "bein", "berg", "besen", "bett", "beule", "biber", "biene", "bild", "birke", "birne",
    "blatt", "blech", "blitz", "block", "blume", "boden", "bohne", "boot", "borke", "box", "brand",
    "brett", "brief", "brise", "brot", "brunnen", "buch", "bucht", "burg", "bus", "dach", "dachs",
    "dampf", "damm", "decke", "deich", "delfin", "distel", "docht", "dorf", "dorn", "dose",
    "draht", "drache", "drossel", "duft", "echo", "ecke", "efeu", "eiche", "eimer", "eis", "elch",
    "engel", "ente", "erbse", "erde", "esel", "essig", "eule", "falke", "farn", "feder", "feige",
    "feile", "fels", "fenster", "ferse", "feuer", "fichte", "fink", "fisch", "flamme", "flasche",
    "fliege", "fliese", "flosse", "fluss", "frosch", "fuchs", "funke", "gabel", "garten", "gasse",
    "gatter", "geige", "geist", "gelb", "gemse", "gerste", "gipfel", "gitter", "glas", "glocke",
    "glut", "gold", "gras", "griff", "grille", "grotte", "gurke", "gurt", "haar", "hafen", "hafer",
    "hahn", "hai", "halle", "hals", "hammer", "hand", "harfe", "hase", "haus", "hecht", "hecke",
    "heft", "helm", "herbst", "herd", "herz", "heu", "himmel", "hirsch", "hobel", "honig", "horn",
    "hose", "huhn", "hund", "hut", "igel", "insel", "jacke", "jagd", "januar", "jaspis", "jubel",
    "juli", "kabel", "kaktus", "kalb", "kamel", "kamm", "kanne", "kante", "kappe", "karpfen",
    "karte", "katze", "kegel", "kelch", "kerze", "kessel", "kiefer", "kies", "kirsche", "kissen",
    "kiste", "klee", "klippe", "knopf", "kohle", "kolben", "komet", "korb", "korn", "krabbe",
    "kran", "kreide", "kreuz", "krone", "krug", "kuh", "kugel", "kupfer", "lachs", "laden",
    "lampe", "lappen", "lauch", "leder", "lehm", "leine", "lerche", "licht", "linde", "linse",
    "lippe", "loch", "lotse", "luft", "lunge", "lupe", "mais", "malve", "mantel", "markt", "mauer",
    "maus", "meer", "mehl", "meise", "melone", "messer", "milch", "minze", "mond", "moos", "motor",
    "mulde", "muschel", "mut", "nadel", "nagel", "name", "nase", "nebel", "nest", "netz", "nord",
    "nuss",
];

/// Kurzform einer ID (nur Anzeige, kein Sicherheitsvergleich).
pub fn short(p: &PeerId) -> String {
    let s: String = p.as_str().chars().take(8).collect();
    let split = s.char_indices().nth(4).map(|(i, _)| i);
    match split {
        Some(i) => format!("{}·{}", &s[..i], &s[i..]),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cocopi-identity-{}", crate::model::random_id()))
    }

    #[test]
    fn words_are_unique_ascii_and_symmetric() {
        let unique: std::collections::HashSet<_> = WORDS.iter().collect();
        assert_eq!(unique.len(), 256);
        assert!(
            WORDS
                .iter()
                .all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()))
        );
        let a = Identity::ephemeral().peer();
        let b = Identity::ephemeral().peer();
        assert_eq!(safety_words(&a, &b).len(), 8);
        assert_eq!(safety_words(&a, &b), safety_words(&b, &a));
        assert_ne!(safety_words(&a, &b), safety_words(&a, &a));
        let (lo, hi) = if a < b { (&a, &b) } else { (&b, &a) };
        let digest = blake3::hash(format!("cocopi-safety-v1{lo}{hi}").as_bytes());
        assert_eq!(
            safety_words(&a, &b),
            digest.as_bytes()[..8]
                .iter()
                .map(|&i| WORDS[i as usize])
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn identity_roundtrip_and_invalid_length() {
        let dir = temp();
        let path = dir.join("nested/identity.key");
        let first = Identity::load_or_create(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap().len(), 32);
        assert_eq!(
            first.peer(),
            Identity::load_or_create(&path).unwrap().peer()
        );
        std::fs::write(&path, [0u8; 31]).unwrap();
        assert!(
            Identity::load_or_create(&path)
                .unwrap_err_text()
                .contains("32 Byte")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    // Kein Debug-Ausdruck des geheimen Schlüssels bei fehlgeschlagenen Erwartungen.
    trait ErrText {
        fn unwrap_err_text(self) -> String;
    }
    impl ErrText for Result<Identity, String> {
        fn unwrap_err_text(self) -> String {
            match self {
                Err(e) => e,
                Ok(_) => panic!("Fehler erwartet"),
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn private_permissions_and_unsafe_file_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp();
        let path = dir.join("private/identity.key");
        Identity::load_or_create(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        for mode in [0o640, 0o604, 0o660, 0o777] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                Identity::load_or_create(&path)
                    .unwrap_err_text()
                    .contains("Schlüsselrechte")
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_and_existing_staging_file_rejected() {
        let dir = temp();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("identity.key");
        let target = dir.join("target");
        std::fs::write(&target, [1u8; 32]).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(Identity::load_or_create(&path).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::write(dir.join("identity.key.new"), b"Rest").unwrap();
        assert!(Identity::load_or_create(&path).is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read(&target).unwrap(), [1u8; 32]);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn short_handles_non_ascii_without_panicking() {
        assert_eq!(short(&PeerId::from("123456789")), "1234·5678");
        assert_eq!(short(&PeerId::from("äöüß日本語")), "äöüß·日本語");
    }
}
