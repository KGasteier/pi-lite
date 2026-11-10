// Isolierter DOM-Interaktionstest: echte outline.js, Demodaten, kein Tauri/pi/Sessionzugriff.
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const {spawnSync} = require('node:child_process');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-outline-dom-'));
const source = fs.readFileSync(path.join(__dirname, '../ui/outline.js'), 'utf8');
const css = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1];
const test = async function () {
  const calls = [], opens = [], names = [];
  const o = {version:1,rev:1,items:[
    {t:'cat',id:'bookmark-color:0',name:'Blau',level:0,collapsed:false},
    {t:'session',key:'/demo/a',level:1,collapsed:false},
    {t:'session',key:'/demo/b',level:1,collapsed:false}
  ],open_unsorted:[]};
  const sessions = ['a','b'].map(x=>({path:'/demo/'+x,name:x,cwd:'/demo'}));
  const clone = x=>JSON.parse(JSON.stringify(x));
  const box = document.getElementById('outline');
  window.BookmarkPalette = {hex:()=> '#76b9ff'};
  const env = {
    invoke:async(c,a)=>{ if(c==='outline_read') return clone(o); calls.push(a.op); return {outline:clone(o)}; },
    toast:m=>{throw Error(m);}, ask:async()=>null,
    openSession:s=>opens.push(s.path), renameSession:s=>names.push(s.path),
    sessionCard:s=>{const d=document.createElement('div');d.className='sess';d.innerHTML='<div class="t">'+s.name+'</div><div class="m">Datum</div><button>Aktion</button>';d.onclick=()=>opens.push(s.path);return d;}
  };
  const v = Outline.createView(box,env); v.setData(sessions,[]); await v.load();
  const assert=(b,msg)=>{if(!b)throw Error(msg);};
  const tick=async()=>{await Promise.resolve(); await Promise.resolve(); await Promise.resolve();};
  const row=()=>box.querySelector('.ol-row[data-item="1"]');
  const key=async(k,extra={})=>{row().dispatchEvent(new KeyboardEvent('keydown',{key:k,bubbles:true,cancelable:true,...extra}));await tick();};
  assert(box.querySelector('.ol-head button[aria-pressed="true"]').textContent==='Detail','Detail ist Standard');
  for(const detail of [true,false]) {
    if(!detail) [...box.querySelectorAll('.ol-head button')].find(b=>b.textContent==='Kompakt').click();
    assert(!!row().classList.contains('sess')===detail,'Kartendarstellung nur in Detail');
    if(detail) assert(row().getBoundingClientRect().height>=52,'Detailkarte nicht auf 22px gestaucht');
    row().focus(); assert(document.activeElement===row(),'Baumzeile fokussierbar');
    for(const [k,extra,op] of [['Tab',{},'indent'],['Tab',{shiftKey:true},'outdent'],['ArrowUp',{altKey:true},'up'],['ArrowDown',{altKey:true},'down']]) {
      await key(k,extra); assert(calls.at(-1).op===op,'Strukturtaste '+op+' in '+detail);
    }
    await key('F2'); assert(names.at(-1)==='/demo/a','Umbenennen in beiden Sichten');
    row().dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true,clientX:50,clientY:100}));
    const menu=[...document.querySelectorAll('.ol-menu button')];
    assert(menu.some(b=>b.textContent==='Einrücken (Tab)'),'Strukturmenü in beiden Sichten');
    menu.find(b=>b.textContent==='Umbenennen …').click(); await tick();
    const r=row().getBoundingClientRect(), n=calls.length;
    row().dispatchEvent(new PointerEvent('pointerdown',{button:0,clientX:r.left+20,clientY:r.top+10,bubbles:true}));
    window.dispatchEvent(new PointerEvent('pointermove',{clientX:r.left+20,clientY:r.bottom+10}));
    assert(box.querySelector('.ol-drop'),'Einfügelinie in beiden Sichten');
    window.dispatchEvent(new PointerEvent('pointerup',{clientX:r.left+20,clientY:r.bottom+10}));
    assert(calls.length===n+1 && calls.at(-1).op==='move_to','Drag verschiebt Teilbaum');
    assert(calls.at(-1).level===1,'Kein Ebenensprung durch unterschiedliche Einrückung');
    const before=opens.length;
    row().click(); assert(opens.length===before,'Drop öffnet nicht versehentlich die Session');
    await tick();
  }
  document.body.dataset.result='PASS';
};
fs.writeFileSync(path.join(dir,'index.html'), '<!doctype html><style>'+css+'</style><div id="outline" style="display:block;width:240px"></div><script>'+source+'</script><script>('+test.toString()+')().catch(e=>{document.body.dataset.result="FAIL: "+e.message;});</script>');
const r=spawnSync('chromium',['--headless=new','--no-sandbox','--no-zygote','--disable-gpu','--disable-dev-shm-usage','--user-data-dir='+dir+'/profile','--virtual-time-budget=2000','--dump-dom','file://'+dir+'/index.html'],{encoding:'utf8',timeout:90000,maxBuffer:2e6});
if(r.error?.code==='ENOENT') {console.log('SKIP: Chromium fehlt');process.exit(0);}
fs.writeFileSync(path.join(dir,'result.html'),r.stdout||'');
if(!r.stdout?.includes('data-result="PASS"')) {console.error('FAIL: '+(r.stdout?.match(/data-result="([^"]*)"/)?.[1]||r.error||r.stderr));console.error(dir);process.exit(1);}
console.log('outline DOM ok: Detail-Standard, Kompakt/Detail-Tasten, Fokus, Menü, Drag/Drop und Klickschutz (nur Demodaten).');
