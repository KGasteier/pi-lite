//! Virtuelle Gliederung der Seitenleiste (copi/cocopi und pi-lite gemeinsam).
//!
//! Die Gliederung ist nur eine Sicht: benennbare Kategorien plus Verweise auf Sessions
//! (`SessionInfo::key`) und Markdown-Dateien (Pfad). Echte Dateien werden nie verschoben,
//! umbenannt oder gelöscht. Gespeichert wird flach mit Ebenen (wie AUGMOST-Zeilen):
//! Kinder eines Eintrags sind die direkt folgenden Einträge mit größerer Ebene.
//! Ablage: `<config>/pi-frontends/outline.json` neben `bookmarks.json`.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use serde_json::Value;

pub const COLOR_PREFIX: &str = "bookmark-color:";
pub fn color_root(r: &Ref) -> Option<Option<u8>> {
    if let Ref::Cat { id, .. } = r {
        let s = id.strip_prefix(COLOR_PREFIX)?;
        return if s == "none" { Some(None) } else { s.parse::<u8>().ok().map(Some) };
    }
    None
}
pub fn ref_key(r: &Ref) -> Option<&str> { match r { Ref::Session { key } => Some(key), Ref::Md { path } => Some(path), _ => None } }
pub fn bookmark_group(v: &Value, r: &Ref) -> Option<u8> {
    let k = ref_key(r)?;
    if !v["bookmarks"].as_array().is_some_and(|a| a.iter().any(|p| p.as_str()==Some(k))) { return None; }
    Some(v["bookmark_colors"][k].as_u64().filter(|i| (*i as usize)<crate::shared_bookmarks::color_count(v)).unwrap_or(0) as u8)
}

/// Farboberkategorien sind aus Bookmarks abgeleitet. Reihenfolge und Unterbäume bleiben
/// erhalten; bei externem Umfärben wandert der Eintrag in die passende Oberkategorie.
pub fn reconcile(o: &mut Outline, bookmarks: &Value, inventory: &[Ref]) {
    let count = crate::shared_bookmarks::color_count(bookmarks);
    let old = o.items.clone();
    let mut groups: Vec<Vec<Item>> = vec![Vec::new(); count+1];
    let mut collapsed = vec![false; count+1];
    let mut current = count;
    let mut i = 0;
    while i < old.len() {
        if let Some(c) = color_root(&old[i].r) {
            current = c.map(|c| (c as usize).min(count)).unwrap_or(count);
            collapsed[current] = old[i].collapsed;
            i += 1; continue;
        }
        let start = i;
        let base = old[i].level;
        i += 1;
        while i<old.len() && color_root(&old[i].r).is_none() && old[i].level>base { i+=1; }
        let target = if ref_key(&old[start].r).is_some() { bookmark_group(bookmarks,&old[start].r).map(usize::from).unwrap_or(count) } else { current };
        let mut block = old[start..i].to_vec();
        for it in &mut block { it.level = (it.level-base+1).min(MAX_LEVEL); }
        groups[target].extend(block);
    }
    // Bereits einsortierte Kinder einzeln extern geändert: aus dem alten Teilbaum lösen.
    for g in 0..=count {
        let mut j=0;
        while j<groups[g].len() {
            let r=&groups[g][j].r;
            if ref_key(r).is_some() {
                let target=bookmark_group(bookmarks,r).map(usize::from).unwrap_or(count);
                if target!=g {
                    let mut it=groups[g].remove(j); it.level=1;
                    groups[target].push(it); continue;
                }
            }
            j+=1;
        }
    }
    for r in inventory {
        if ref_key(r).is_some() && !groups.iter().flatten().any(|it| it.r.same(r)) {
            let g=bookmark_group(bookmarks,r).map(usize::from).unwrap_or(count);
            groups[g].push(Item::new(r.clone(),1));
        }
    }
    o.items.clear();
    for (g, block) in groups.into_iter().enumerate() {
        let (id,name) = if g==count { (format!("{COLOR_PREFIX}none"),"Unmarkiert".into()) } else {
            let name=bookmarks["category_names"][g].as_str().filter(|n| !n.trim().is_empty()).map(str::to_owned).unwrap_or_else(|| ["Blau","Grün","Gelb","Violett","Orange","Rot","Türkis","Pink","Limette","Braun"].get(g).map(|s|s.to_string()).unwrap_or_else(||format!("Farbe {}",g+1)));
            (format!("{COLOR_PREFIX}{g}"),name)
        };
        let mut root=Item::new(Ref::Cat{id,name},0); root.collapsed=collapsed[g];
        o.items.push(root); o.items.extend(block);
    }
    o.normalize();
}

/// Bewegungen dürfen die automatisch erzeugten Farboberkategorien nicht verändern.
pub fn apply_colored_model(o: &mut Outline, b: &mut Value, op: Op) -> (bool,Option<usize>) {
    let root = |i:usize| o.items.get(i).is_some_and(|it|color_root(&it.r).is_some());
    match &op {
        Op::Remove{i}|Op::Rename{i,..}|Op::Indent{i}|Op::Outdent{i}|Op::Up{i}|Op::Down{i} if root(*i) => return (false,Some(*i)),
        Op::MoveTo{src,..} if root(*src) => return (false,Some(*src)),
        Op::Outdent{i} if o.items.get(*i).is_some_and(|it|it.level<=1) => return (false,Some(*i)),
        _=>{}
    }
    let moved=match &op { Op::MoveTo{src,..}|Op::Indent{i:src}|Op::Outdent{i:src}|Op::Up{i:src}|Op::Down{i:src} => o.items.get(*src).map(|it|it.r.clone()), Op::Insert{r,..}|Op::IntoCat{r,..}=>Some(r.clone()), _=>None };
    let op=match op {
        Op::MoveTo{src,dest,level}=>Op::MoveTo{src,dest:dest.max(1),level:level.max(1)},
        Op::Insert{r,dest,level}=>Op::Insert{r,dest:dest.max(1),level:level.max(1)},
        Op::AddCat{after,child,name}=>Op::AddCat{after:after.or_else(||o.items.iter().rposition(|it|color_root(&it.r).is_some())),child:child || after.is_none_or(root),name},
        other=>other
    };
    let result=o.apply(op);
    if let Some(r)=moved {
        if let Some(i)=o.find(&r) {
            let g=(0..=i).rev().find_map(|j|color_root(&o.items[j].r));
            if let Some(c)=g {
                for it in &o.items[i..o.subtree_end(i)] {
                    if let Some(k)=ref_key(&it.r) { set_marker(b,k,c); }
                }
            }
        }
    }
    result
}
pub fn set_marker(b:&mut Value,k:&str,c:Option<u8>) {
    let mut paths=b["bookmarks"].as_array().cloned().unwrap_or_default();
    paths.retain(|p|p.as_str()!=Some(k));
    if c.is_some() { paths.push(Value::String(k.into())); }
    b["bookmarks"]=Value::Array(paths);
    if !b["bookmark_colors"].is_object() { b["bookmark_colors"]=serde_json::json!({}); }
    if let Some(c)=c { b["bookmark_colors"][k]=serde_json::json!(c); } else { b["bookmark_colors"].as_object_mut().unwrap().remove(k); }
}
pub fn prepare(inventory:&[Ref]) -> Result<Outline,String> {
    locked(&path()?,|disk| { let mut o=disk.clone(); reconcile(&mut o,&crate::shared_bookmarks::read()?,inventory); Ok((o,())) }).map(|r|r.0)
}
pub fn apply_colored(rev:u64,op:Op) -> Result<(Outline,Option<usize>),String> {
    locked(&path()?,|disk| {
        if disk.rev!=rev { return Err(CONFLICT.into()); }
        let base=crate::shared_bookmarks::read()?;
        let mut b=base.clone(); let mut o=disk.clone();
        let (_,at)=apply_colored_model(&mut o,&mut b,op);
        // Die Bookmark-Datei hat eine eigene Deltasperre; nie einen alten Vollstand schreiben.
        if b!=base { crate::shared_bookmarks::update(&base,&b)?; }
        Ok((o,at))
    })
}


#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Ref {
    Cat { id: String, name: String },
    Session { key: String },
    Md { path: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    #[serde(flatten)]
    pub r: Ref,
    #[serde(default)]
    pub level: usize,
    #[serde(default)]
    pub collapsed: bool,
}

impl Ref {
    /// Gleicher Eintrag (Kategorien über `id`, Namen egal).
    pub fn same(&self, o: &Ref) -> bool {
        match (self, o) {
            (Ref::Cat { id: a, .. }, Ref::Cat { id: b, .. }) => a == b,
            (a, b) => a == b,
        }
    }
}

/// Änderung an der Gliederung (von copi direkt, von pi-lite per Tauri-Befehl `outline_apply`).
/// Indizes beziehen sich auf `items` im Stand `rev`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Toggle { i: usize },
    SetCollapsed { i: usize, collapsed: bool },
    Indent { i: usize },
    Outdent { i: usize },
    Up { i: usize },
    Down { i: usize },
    MoveTo { src: usize, dest: usize, level: usize },
    Insert { r: Ref, dest: usize, level: usize },
    /// Als letztes Kind der Kategorie `cat` ablegen (aufnehmen oder verschieben).
    IntoCat { r: Ref, cat: usize },
    Remove { i: usize },
    AddCat { after: Option<usize>, child: bool, name: String },
    Rename { i: usize, name: String },
    ToggleUnsorted { group: String },
}

impl Item {
    pub fn new(r: Ref, level: usize) -> Self { Self { r, level, collapsed: false } }
    pub fn is_cat(&self) -> bool { matches!(self.r, Ref::Cat { .. }) }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Outline {
    pub version: u32,
    /// Wird bei jedem Speichern erhöht; Speichern mit veraltetem Stand wird abgelehnt.
    pub rev: u64,
    pub items: Vec<Item>,
    /// Aufgeklappte Pseudo-Gruppen „Unsortiert“ (`"sessions"`, `"md"`); Vorgabe: zu.
    pub open_unsorted: Vec<String>,
}

pub const MAX_LEVEL: usize = 12;

impl Outline {
    /// Ebenen reparieren (erste 0, höchstens Vorgänger + 1), doppelte Verweise entfernen.
    pub fn normalize(&mut self) {
        let mut seen: Vec<Ref> = Vec::new();
        self.items.retain(|it| {
            let key = match &it.r { Ref::Cat { id, .. } => Ref::Cat { id: id.clone(), name: String::new() }, r => r.clone() };
            if matches!(&it.r, Ref::Cat { id, .. } if id.is_empty()) || seen.contains(&key) { return false; }
            seen.push(key);
            true
        });
        let mut prev: Option<usize> = None;
        for it in &mut self.items {
            it.level = match prev { None => 0, Some(p) => it.level.min(p + 1) }.min(MAX_LEVEL);
            prev = Some(it.level);
        }
    }

    /// Erster Index nach dem Teilbaum von `i`.
    pub fn subtree_end(&self, i: usize) -> usize {
        let l = self.items[i].level;
        (i + 1..self.items.len()).find(|&j| self.items[j].level <= l).unwrap_or(self.items.len())
    }
    pub fn has_children(&self, i: usize) -> bool { self.items.get(i + 1).is_some_and(|n| n.level > self.items[i].level) }

    /// Sichtbare Indizes (Nachfahren zugeklappter Einträge ausgelassen).
    pub fn visible(&self) -> Vec<usize> {
        let mut v = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            v.push(i);
            i = if self.items[i].collapsed { self.subtree_end(i) } else { i + 1 };
        }
        v
    }

    pub fn parent(&self, i: usize) -> Option<usize> {
        let l = self.items.get(i)?.level;
        (0..i).rev().find(|&j| self.items[j].level < l)
    }

    pub fn contains(&self, r: &Ref) -> bool { self.find(r).is_some() }
    pub fn find(&self, r: &Ref) -> Option<usize> { self.items.iter().position(|it| it.r.same(r)) }

    /// Operation ausführen: `(geändert, Index des betroffenen Eintrags danach)`.
    pub fn apply(&mut self, op: Op) -> (bool, Option<usize>) {
        let n = self.items.len();
        let ok = |i: usize| i < n;
        match op {
            Op::Toggle { i } if ok(i) && self.has_children(i) => { self.toggle(i); (true, Some(i)) }
            Op::SetCollapsed { i, collapsed } if ok(i) => (self.set_collapsed(i, collapsed), Some(i)),
            Op::Indent { i } if ok(i) => (self.indent(i), Some(i)),
            Op::Outdent { i } if ok(i) => (self.outdent(i), Some(i)),
            Op::Up { i } if ok(i) => match self.move_up(i) { Some(j) => (true, Some(j)), None => (false, Some(i)) },
            Op::Down { i } if ok(i) => match self.move_down(i) { Some(j) => (true, Some(j)), None => (false, Some(i)) },
            Op::MoveTo { src, dest, level } if ok(src) => { let before = self.items.clone(); let at = self.move_to(src, dest, level); (self.items != before, at) }
            Op::Insert { r, dest, level } => { let before = self.items.clone(); let at = self.insert(r, dest, level); (self.items != before, at) }
            Op::IntoCat { r, cat } if ok(cat) && self.items[cat].is_cat() && !r.same(&self.items[cat].r) => {
                // Nicht in den eigenen Teilbaum verschieben.
                if let Some(src) = self.find(&r) { if cat > src && cat < self.subtree_end(src) { return (false, Some(src)); } }
                let (dest, level) = (self.subtree_end(cat), self.items[cat].level + 1);
                let id = self.items[cat].r.clone();
                let before = self.items.clone();
                let at = self.insert(r, dest, level);
                if let Some(c) = self.find(&id) { self.items[c].collapsed = false; }
                (self.items != before, at)
            }
            Op::Remove { i } if ok(i) => { self.remove(i); (true, if self.items.is_empty() { None } else { Some(i.min(self.items.len() - 1)) }) }
            Op::AddCat { after, child, name } => {
                let id = new_id(self);
                let at = match after.filter(|&a| ok(a)) {
                    Some(a) if child => {
                        let at = self.subtree_end(a);
                        let l = (self.items[a].level + 1).min(MAX_LEVEL);
                        self.items.insert(at, Item::new(Ref::Cat { id, name: clean(&name) }, l));
                        self.items[a].collapsed = false;
                        at
                    }
                    a => self.add_category(a, &name, id),
                };
                (true, Some(at))
            }
            Op::Rename { i, name } if ok(i) => (self.rename(i, &name), Some(i)),
            Op::ToggleUnsorted { group } => { self.toggle_unsorted(&group); (true, None) }
            _ => (false, None),
        }
    }

    pub fn toggle(&mut self, i: usize) {
        if self.has_children(i) { self.items[i].collapsed = !self.items[i].collapsed; }
    }
    pub fn set_collapsed(&mut self, i: usize, c: bool) -> bool {
        if !self.has_children(i) || self.items[i].collapsed == c { return false; }
        self.items[i].collapsed = c;
        true
    }

    /// Teilbaum eine Ebene tiefer (nur wenn ein Vorgänger als neuer Elternteil existiert).
    pub fn indent(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.items.len() || self.items[i].level > self.items[i - 1].level || self.items[i].level >= MAX_LEVEL { return false; }
        let end = self.subtree_end(i);
        // Neuer Elternteil muss sichtbar bleiben, sonst „verschwindet“ der Eintrag.
        if let Some(p) = (0..i).rev().find(|&j| self.items[j].level == self.items[i].level) { self.items[p].collapsed = false; }
        for it in &mut self.items[i..end] { it.level += 1; }
        true
    }
    /// Teilbaum eine Ebene höher; folgende Geschwister werden dabei zu Kindern (Outliner-üblich).
    pub fn outdent(&mut self, i: usize) -> bool {
        if i >= self.items.len() || self.items[i].level == 0 { return false; }
        let end = self.subtree_end(i);
        for it in &mut self.items[i..end] { it.level -= 1; }
        let l = self.items[i].level;
        if end < self.items.len() && self.items[end].level > l + 1 { self.normalize(); }
        if self.has_children(i) && end < self.items.len() && self.items[end].level > l { self.items[i].collapsed = false; }
        true
    }

    /// Mit vorherigem Geschwister-Teilbaum tauschen; liefert neuen Index.
    pub fn move_up(&mut self, i: usize) -> Option<usize> {
        let l = self.items.get(i)?.level;
        let prev = (0..i).rev().find(|&j| self.items[j].level <= l)?;
        if self.items[prev].level != l { return None; }
        let end = self.subtree_end(i);
        let block: Vec<Item> = self.items.drain(i..end).collect();
        self.items.splice(prev..prev, block);
        Some(prev)
    }
    pub fn move_down(&mut self, i: usize) -> Option<usize> {
        let l = self.items.get(i)?.level;
        let end = self.subtree_end(i);
        if end >= self.items.len() || self.items[end].level != l { return None; }
        let next_end = self.subtree_end(end);
        let block: Vec<Item> = self.items.drain(i..end).collect();
        let at = next_end - block.len();
        self.items.splice(at..at, block);
        Some(at)
    }

    /// Teilbaum `src` vor Index `dest` (Index vor dem Entfernen) auf Ebene `level` verschieben.
    /// Ziel innerhalb des eigenen Teilbaums wird abgelehnt. Liefert neuen Index.
    pub fn move_to(&mut self, src: usize, dest: usize, level: usize) -> Option<usize> {
        if src >= self.items.len() || dest > self.items.len() { return None; }
        let end = self.subtree_end(src);
        if dest > src && dest < end { return None; }
        let mut block: Vec<Item> = self.items.drain(src..end).collect();
        let at = if dest >= end { dest - block.len() } else { dest };
        let delta = level as isize - block[0].level as isize;
        for it in &mut block { it.level = (it.level as isize + delta).max(0) as usize; }
        self.items.splice(at..at, block);
        self.open_parent(at);
        self.normalize();
        self.find(&self.items.get(at)?.r.clone())
    }

    /// Neuen Eintrag vor `dest` auf Ebene `level` einfügen (bereits enthaltene Verweise: verschieben).
    pub fn insert(&mut self, r: Ref, dest: usize, level: usize) -> Option<usize> {
        if let Some(i) = self.find(&r) { return self.move_to(i, dest, level); }
        let at = dest.min(self.items.len());
        self.items.insert(at, Item::new(r.clone(), level));
        self.open_parent(at);
        self.normalize();
        self.find(&r)
    }

    /// Elternteil eines (neu) platzierten Eintrags aufklappen, damit er sichtbar bleibt.
    fn open_parent(&mut self, i: usize) {
        let mut j = i;
        while let Some(p) = self.parent(j) { self.items[p].collapsed = false; j = p; }
    }

    /// Eintrag entfernen; seine Kinder rücken eine Ebene auf (nichts geht verloren).
    pub fn remove(&mut self, i: usize) -> Option<Item> {
        if i >= self.items.len() { return None; }
        let end = self.subtree_end(i);
        for it in &mut self.items[i + 1..end] { it.level = it.level.saturating_sub(1); }
        let it = self.items.remove(i);
        self.normalize();
        Some(it)
    }

    /// Kategorie nach dem Teilbaum von `after` (gleiche Ebene) oder am Ende anlegen.
    pub fn add_category(&mut self, after: Option<usize>, name: &str, id: String) -> usize {
        let name = clean(name);
        let (at, level) = match after.filter(|&i| i < self.items.len()) {
            Some(i) => (self.subtree_end(i), self.items[i].level),
            None => (self.items.len(), 0),
        };
        self.items.insert(at, Item::new(Ref::Cat { id, name }, level));
        at
    }
    pub fn rename(&mut self, i: usize, name: &str) -> bool {
        let name: String = name.trim().chars().take(80).collect();
        match self.items.get_mut(i) {
            Some(Item { r: Ref::Cat { name: n, .. }, .. }) if !name.is_empty() => { *n = name; true }
            _ => false,
        }
    }

    pub fn unsorted_open(&self, group: &str) -> bool { self.open_unsorted.iter().any(|g| g == group) }
    pub fn toggle_unsorted(&mut self, group: &str) {
        if let Some(p) = self.open_unsorted.iter().position(|g| g == group) { self.open_unsorted.remove(p); } else { self.open_unsorted.push(group.into()); }
    }
}

fn clean(name: &str) -> String {
    let n: String = name.trim().chars().filter(|c| !c.is_control()).take(80).collect();
    if n.is_empty() { "Kategorie".into() } else { n }
}

/// Ablageebene aus gewünschter Ebene: zwischen Ebene der Zeile darunter und Ebene darüber + 1
/// (wie AUGMOST `drop_target`); ganz oben nur Ebene 0.
pub fn drop_level(above: Option<usize>, below: Option<usize>, want: usize) -> usize {
    let Some(a) = above else { return 0 };
    let max = (a + 1).min(MAX_LEVEL);
    let min = below.unwrap_or(0).min(max);
    want.clamp(min, max)
}

/// Neue Kategorie-ID (Zeit + Zähler, ohne Zufallsquelle; eindeutig genug je Datei).
pub fn new_id(o: &Outline) -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let mut n = 0u32;
    loop {
        let id = format!("c{t:x}{n}");
        if !o.contains(&Ref::Cat { id: id.clone(), name: String::new() }) { return id; }
        n += 1;
    }
}

pub fn path() -> Result<PathBuf, String> { Ok(crate::shared_bookmarks::path()?.with_file_name("outline.json")) }

pub fn read_at(p: &Path) -> Result<Outline, String> {
    match std::fs::read(p) {
        Ok(b) => {
            let mut o: Outline = serde_json::from_slice(&b).map_err(|e| format!("Gliederungsdatei ungültig ({}): {e}", p.display()))?;
            o.normalize();
            Ok(o)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Outline { version: 1, ..Default::default() }),
        Err(e) => Err(e.to_string()),
    }
}
pub fn read() -> Result<Outline, String> { read_at(&path()?) }
pub fn mtime() -> Option<std::time::SystemTime> { std::fs::metadata(path().ok()?).and_then(|m| m.modified()).ok() }

pub const CONFLICT: &str = "Gliederung wurde inzwischen in einem anderen Fenster geändert – neu geladen, bitte Änderung wiederholen.";

/// Speichern nur, wenn die Datei noch Stand `o.rev` hat (sonst `Err(CONFLICT)`); atomar mit Sperre.
/// Eine unlesbare Datei wird nie überschrieben.
#[allow(dead_code)] // copi/pi-lite nutzen `apply`; Ganzspeichern für Import/Werkzeuge.
pub fn save_at(p: &Path, o: &Outline) -> Result<Outline, String> {
    locked(p, |disk| if disk.rev != o.rev { Err(CONFLICT.into()) } else { Ok((o.clone(), ())) }).map(|(o, _)| o)
}
#[allow(dead_code)]
pub fn save(o: &Outline) -> Result<Outline, String> { save_at(&path()?, o) }

/// Operation auf den Dateistand `rev` anwenden und speichern. Nicht geänderte Gliederungen
/// werden nicht geschrieben. Liefert den neuen Stand und den betroffenen Index.
pub fn apply_at(p: &Path, rev: u64, op: Op) -> Result<(Outline, Option<usize>), String> {
    locked(p, |disk| {
        if disk.rev != rev { return Err(CONFLICT.into()); }
        let mut o = disk.clone();
        let (_, at) = o.apply(op);
        Ok((o, at))
    })
}
pub fn apply(rev: u64, op: Op) -> Result<(Outline, Option<usize>), String> { apply_at(&path()?, rev, op) }

fn locked<T>(p: &Path, f: impl FnOnce(&Outline) -> Result<(Outline, T), String>) -> Result<(Outline, T), String> {
    std::fs::create_dir_all(p.parent().ok_or("Kein Elternordner")?).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(p.with_extension("lock")).map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let disk = read_at(p)?;
    let (o, t) = f(&disk)?;
    if o.items == disk.items && o.open_unsorted == disk.open_unsorted && p.exists() { return Ok((disk, t)); }
    let mut out = o;
    out.normalize();
    out.version = 1;
    out.rev = disk.rev + 1;
    let tmp = p.with_extension(format!("{}.tmp", std::process::id()));
    use std::io::Write;
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(&serde_json::to_vec_pretty(&out).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(tmp, p).map_err(|e| e.to_string())?;
    Ok((out, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cat(id: &str, l: usize) -> Item { Item::new(Ref::Cat { id: id.into(), name: id.into() }, l) }
    fn ses(k: &str, l: usize) -> Item { Item::new(Ref::Session { key: k.into() }, l) }
    fn md(p: &str, l: usize) -> Item { Item::new(Ref::Md { path: p.into() }, l) }
    fn levels(o: &Outline) -> Vec<usize> { o.items.iter().map(|i| i.level).collect() }
    fn names(o: &Outline) -> Vec<String> {
        o.items.iter().map(|i| match &i.r { Ref::Cat { id, .. } => id.clone(), Ref::Session { key } => key.clone(), Ref::Md { path } => path.clone() }).collect()
    }
    fn sample() -> Outline { Outline { items: vec![cat("A", 0), ses("s1", 1), md("m1", 2), cat("B", 0), ses("s2", 1)], ..Default::default() } }

    #[test]
    fn colors_migrate_preserve_and_recolor_subtrees() {
        let mut o=sample();
        let mut b=crate::shared_bookmarks::empty();
        set_marker(&mut b,"s1",Some(3));
        reconcile(&mut o,&b,&[Ref::Session{key:"new".into()}]);
        assert_eq!(o.items.iter().filter(|it|color_root(&it.r).is_some()).count(),11);
        let saved=o.clone(); reconcile(&mut o,&b,&[]); assert_eq!(o,saved,"Refresh ist idempotent");
        let r=Ref::Session{key:"s1".into()};
        let cat=o.items.iter().position(|it|color_root(&it.r)==Some(Some(2))).unwrap();
        apply_colored_model(&mut o,&mut b,Op::IntoCat{r:r.clone(),cat});
        assert_eq!(bookmark_group(&b,&r),Some(2));
        let root=o.items.iter().position(|it|color_root(&it.r)==Some(None)).unwrap();
        apply_colored_model(&mut o,&mut b,Op::IntoCat{r:r.clone(),cat:root});
        assert_eq!(bookmark_group(&b,&r),None);
        let first=o.items[0].clone(); apply_colored_model(&mut o,&mut b,Op::Remove{i:0}); assert_eq!(o.items[0],first);
    }

    #[test]
    fn normalize_repairs_levels_and_duplicates() {
        let mut o = Outline { items: vec![ses("x", 3), ses("y", 5), ses("x", 0), cat("", 0)], ..Default::default() };
        o.normalize();
        assert_eq!(names(&o), ["x", "y"]);
        assert_eq!(levels(&o), [0, 1]);
    }

    #[test]
    fn collapse_hides_descendants_only() {
        let mut o = sample();
        o.toggle(0);
        assert_eq!(o.visible(), [0, 3, 4]);
        o.toggle(2); // Blatt: keine Wirkung
        assert!(!o.items[2].collapsed);
    }

    #[test]
    fn indent_outdent_move_subtrees() {
        let mut o = sample();
        assert!(!o.indent(0));
        assert!(o.indent(3)); // B unter A, s2 wandert mit
        assert_eq!(levels(&o), [0, 1, 2, 1, 2]);
        assert!(o.outdent(3));
        assert_eq!(levels(&o), [0, 1, 2, 0, 1]);
        assert_eq!(o.move_up(3), Some(0));
        assert_eq!(names(&o), ["B", "s2", "A", "s1", "m1"]);
        assert_eq!(o.move_down(0), Some(3));
        assert_eq!(names(&o), ["A", "s1", "m1", "B", "s2"]);
        assert_eq!(o.move_up(1), None); // erstes Kind
    }

    #[test]
    fn indent_opens_collapsed_new_parent() {
        let mut o = sample();
        o.items[0].collapsed = true;
        assert!(o.indent(3));
        assert!(!o.items[0].collapsed);
    }

    #[test]
    fn move_to_rejects_own_subtree_and_shifts_levels() {
        let mut o = sample();
        assert_eq!(o.move_to(0, 2, 0), None);
        let at = o.move_to(0, 5, 1).unwrap(); // A samt Kindern unter B
        assert_eq!(names(&o), ["B", "s2", "A", "s1", "m1"]);
        assert_eq!(at, 2);
        assert_eq!(levels(&o), [0, 1, 1, 2, 3]);
    }

    #[test]
    fn remove_promotes_children_and_insert_places_refs() {
        let mut o = sample();
        o.remove(0);
        assert_eq!(names(&o), ["s1", "m1", "B", "s2"]);
        assert_eq!(levels(&o), [0, 1, 0, 1]);
        let i = o.insert(Ref::Md { path: "neu.md".into() }, 2, 1).unwrap();
        assert_eq!(i, 2);
        assert_eq!(levels(&o), [0, 1, 1, 0, 1]);
        // Bereits enthaltener Verweis wird verschoben, nicht doppelt angelegt.
        o.insert(Ref::Session { key: "s2".into() }, 0, 0);
        assert_eq!(o.items.iter().filter(|i| i.r == Ref::Session { key: "s2".into() }).count(), 1);
        assert_eq!(names(&o)[0], "s2");
    }

    #[test]
    fn add_and_rename_category() {
        let mut o = sample();
        let i = o.add_category(Some(0), "  Kurs  ", new_id(&o));
        assert_eq!(i, 3);
        assert_eq!(o.items[3].level, 0);
        assert!(o.rename(3, "Kurs WS"));
        assert!(!o.rename(1, "Session")); // nur Kategorien
        assert!(matches!(&o.items[3].r, Ref::Cat { name, .. } if name == "Kurs WS"));
    }

    #[test]
    fn drop_level_follows_neighbours() {
        assert_eq!(drop_level(None, Some(0), 3), 0);
        assert_eq!(drop_level(Some(1), Some(1), 0), 1);
        assert_eq!(drop_level(Some(1), Some(0), 5), 2);
        assert_eq!(drop_level(Some(2), None, 0), 0);
    }

    #[test]
    fn save_rejects_stale_rev_and_keeps_invalid_file() {
        let dir = std::env::temp_dir().join(format!("outline-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        let p = dir.join("outline.json");
        let _ = std::fs::remove_dir_all(&dir);
        let a = save_at(&p, &sample()).unwrap();
        assert_eq!(a.rev, 1);
        assert_eq!(read_at(&p).unwrap().items, sample().items);
        assert_eq!(save_at(&p, &sample()).unwrap_err(), CONFLICT); // rev 0 veraltet
        std::fs::write(&p, b"{kaputt").unwrap();
        assert!(save_at(&p, &a).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"{kaputt");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn apply_at_checks_rev_and_skips_noop_writes() {
        let dir = std::env::temp_dir().join(format!("outline-apply-{}-{:?}", std::process::id(), std::thread::current().id()));
        let p = dir.join("outline.json");
        let _ = std::fs::remove_dir_all(&dir);
        let a = save_at(&p, &sample()).unwrap();
        let (b, at) = apply_at(&p, a.rev, Op::Indent { i: 3 }).unwrap();
        assert_eq!((b.rev, at), (2, Some(3)));
        let (c, _) = apply_at(&p, b.rev, Op::Indent { i: 0 }).unwrap(); // nicht möglich → kein Schreiben
        assert_eq!(c.rev, 2);
        assert_eq!(apply_at(&p, 1, Op::Outdent { i: 3 }).unwrap_err(), CONFLICT);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ops_into_cat_add_child_and_serde_shape() {
        let mut o = sample();
        let (c, at) = o.apply(Op::IntoCat { r: Ref::Session { key: "neu".into() }, cat: 3 });
        assert!(c);
        assert_eq!(at, Some(5));
        assert_eq!(o.items[5].level, 1);
        assert_eq!(o.apply(Op::IntoCat { r: Ref::Cat { id: "A".into(), name: String::new() }, cat: 1 }).0, false); // Ziel keine Kategorie
        o.items[0].collapsed = true;
        let (_, at) = o.apply(Op::AddCat { after: Some(0), child: true, name: "\u{7}Unter".into() });
        assert_eq!(at, Some(3));
        assert!(!o.items[0].collapsed);
        assert!(matches!(&o.items[3].r, Ref::Cat { name, .. } if name == "Unter"));
        let op: Op = serde_json::from_str(r#"{"op":"move_to","src":1,"dest":0,"level":0}"#).unwrap();
        assert_eq!(op, Op::MoveTo { src: 1, dest: 0, level: 0 });
        let it = serde_json::to_value(&o.items[1]).unwrap();
        assert_eq!(it["t"], "session");
        assert_eq!(it["key"], "s1");
    }
}
