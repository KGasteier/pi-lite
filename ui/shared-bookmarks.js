// Gemeinsame Datei über Tauri; localStorage ist nur Cache und einmalige Migrationsquelle.
function createSharedBookmarkSync(invoke, storage, changed, error) {
  const keys = ['pi-lite.session-bookmarks.v1','pi-lite.session-bookmark-colors.v1','pi-lite.category-color-names.v1'];
  const empty = () => ({bookmarks:[],bookmark_colors:{},category_names:[]});
  const snapshot = () => {
    const names=JSON.parse(storage.getItem(keys[2])||'{}');
    return {bookmarks:JSON.parse(storage.getItem(keys[0])||'[]'),bookmark_colors:JSON.parse(storage.getItem(keys[1])||'{}'),category_names:Array.from({length:10},(_,i)=>names[i]||'')};
  };
  const apply = v => {
    if (!v || !Array.isArray(v.bookmarks)) throw Error('Ungültige gemeinsame Bookmark-Antwort');
    storage.setItem(keys[0],JSON.stringify(v.bookmarks));
    storage.setItem(keys[1],JSON.stringify(v.bookmark_colors));
    storage.setItem(keys[2],JSON.stringify(Object.fromEntries(v.category_names.map((n,i)=>[i,n]).filter(([,n])=>n))));
    changed();
  };
  let chain=Promise.resolve(), ready=false, base=empty();
  const queue = task => { chain=chain.then(task).catch(e=>error(String(e))); return chain; };
  const adapter={getItem:k=>storage.getItem(k),setItem:(k,v)=>{
    const before=snapshot();
    storage.setItem(k,v);
    if (ready && keys.includes(k)) {
      const local=snapshot();
      queue(async()=>{ const v=await invoke('shared_bookmarks_update',{base:before,local}); base=v; });
    }
  }};
  return {
    storage:adapter,
    initialize:()=>queue(async()=>{
      const marker='pi-lite.shared-bookmarks-migrated.v1';
      const migrated=storage.getItem(marker)==='1';
      const v=await invoke(migrated?'shared_bookmarks_read':'shared_bookmarks_seed',{local:snapshot()});
      apply(v); base=v; storage.setItem(marker,'1'); ready=true;
    }),
    refresh:()=>queue(async()=>{ if(!ready)return; const v=await invoke('shared_bookmarks_read');
      if(JSON.stringify(v)!==JSON.stringify(snapshot())){ apply(v); } base=v;
    }),
  };
}
if(typeof module!=='undefined')module.exports={createSharedBookmarkSync};
