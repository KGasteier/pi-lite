const assert=require('node:assert/strict');
const {createSharedBookmarkSync}=require('../ui/shared-bookmarks.js');
(async()=>{
const values=new Map([['pi-lite.shared-bookmarks-migrated.v1','1']]);
const storage={getItem:k=>values.get(k)||null,setItem:(k,v)=>values.set(k,v)};
let disk={bookmarks:['/synthetic/copi'],bookmark_colors:{'/synthetic/copi':6},category_names:['Kurs']};
let calls=[];
const invoke=async(cmd,args)=>{calls.push(cmd); if(cmd==='shared_bookmarks_update'){
 // Test nur einzelnes Delta; echte gesperrte Zusammenführung im gemeinsamen Rust-Test.
 disk=args.local;
} return structuredClone(disk);};
const sync=createSharedBookmarkSync(invoke,storage,()=>{},e=>{throw Error(e)});
await sync.initialize();
assert.equal(JSON.parse(storage.getItem('pi-lite.session-bookmarks.v1'))[0],'/synthetic/copi');
sync.storage.setItem('pi-lite.session-bookmarks.v1',JSON.stringify(['/synthetic/copi','/synthetic/pi-lite']));
await sync.refresh(); assert.equal(disk.bookmarks.length,2);
disk.bookmark_colors['/synthetic/pi-lite']=3;
await sync.refresh(); assert.equal(JSON.parse(storage.getItem('pi-lite.session-bookmark-colors.v1'))['/synthetic/pi-lite'],3);
assert(!calls.includes('shared_bookmarks_seed'),'Migration nicht wiederholen');
console.log('Gemeinsame Bookmark-Brücke: Initialisierung, Schreiben, Fremdänderung und Migration geprüft.');
})().catch(e=>{console.error(e);process.exit(1)});
