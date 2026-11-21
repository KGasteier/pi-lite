// Isoliertes Chromium mit Tauri-Stub. Keine echten Sessions, kein Modell, kein Clipboardzugriff.
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const {spawnSync} = require('node:child_process');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pi-chat-material-dom-'));
fs.cpSync(path.join(__dirname,'../ui'),dir,{recursive:true});
const stub = () => {
  const listeners = {}, calls = [], prompts = [], pending = {}, previews = {};
  let sessionFile = '/demo/a.jsonl';
  const model = {provider:'demo',id:'vision',name:'Vision',input:['text','image']};
  window.fixture = { calls, prompts, pending, previews };
  window.__TAURI__ = {
    event: {listen:async (n, fn) => {listeners[n]=fn;return ()=>{};}},
    core: {invoke:async (c,p={}) => {
      calls.push([c,p]);
      if(c==='start_pi') {sessionFile=p.args?.[1] || '/demo/a.jsonl';return;}
      if(c==='send') {
        const q=JSON.parse(p.line);
        if(q.type==='prompt') prompts.push(q);
        const data=q.type==='get_state'?{sessionFile,model}:q.type==='get_available_models'?{models:[model]}:q.type==='get_messages'?{messages:[]}:{};
        queueMicrotask(()=>listeners['pi-line']?.({payload:JSON.stringify({type:'response',id:q.id,command:q.type,success:true,data})})); return;
      }
      if(c==='prompt_history') return await new Promise(ok=>{pending[p.path]=ok;});
      if(c==='resume_session') return '/demo/a.jsonl';
      if(c==='startup_info') return {cwd:'/demo'};
      if(c==='work_locations') return [];
      if(c==='prepare_material') return {text:'',images:[]};
      if(c==='material_browser') return {path:'/demo',entries:['a.png','b.png'].map(name=>({name,path:'/demo/'+name,is_dir:false}))};
      if(c==='material_preview') return await new Promise(ok=>{previews[p.path]=ok;});
      return [];
    }}
  };
};
const test = async () => {
  const assert = (b,m) => { if(!b) throw Error(m); };
  const until = async f => { for(let i=0;i<100;i++){if(f())return;await new Promise(ok=>setTimeout(ok,10));}throw Error('Timeout'); };
  const tick = () => new Promise(ok=>setTimeout(ok,15));
  const {prompts,pending,previews} = window.fixture;
  await until(()=>state.sessionFile==='/demo/a.jsonl');
  await tick();
  const field=$('prompt'); field.value='Entwurf';
  const first=recallInput(-1);
  await until(()=>pending['/demo/a.jsonl']);
  pending['/demo/a.jsonl'](['Erster vor Komprimierung\nzweite Zeile','Letzter\nzweite Zeile']);
  await first; assert(field.value==='Letzter\nzweite Zeile','letzter Prompt');
  const arrow = key => field.dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true}));
  arrow('ArrowUp'); assert(field.value==='Erster vor Komprimierung\nzweite Zeile','erster Prompt trotz mehrzeiliger Eingabe');
  arrow('ArrowDown'); arrow('ArrowDown'); assert(field.value==='Entwurf','Entwurf beim Rückweg');
  delete pending['/demo/a.jsonl'];
  const slow=recallInput(-1); await until(()=>pending['/demo/a.jsonl']);
  await start({session:'/demo/b.jsonl'});
  pending['/demo/a.jsonl'](['FREMDE SESSION']); await slow;
  assert(field.value!=='FREMDE SESSION','verspätete fremde Session bleibt unsichtbar');
  const b=recallInput(-1); await until(()=>pending['/demo/b.jsonl']); pending['/demo/b.jsonl'](['Nur B']); await b;
  assert(field.value==='Nur B','richtiger Sessionpfad');
  const log=$('log');
  const long=document.createElement('div'); long.style='flex:none; height:18000px'; log.append(long); log.scrollTop=0;
  $('jumpend').click(); assert(log.scrollTop > 10000,'sprung ans Ende ohne Scrollschritte');
  const data=new DataTransfer(); data.items.add(new File([Uint8Array.of(137,80,78,71,13,10,26,10,0)],'clipboard.png',{type:'image/png'}));
  const ev=new ClipboardEvent('paste',{clipboardData:data,cancelable:true,bubbles:true}); field.dispatchEvent(ev);
  await until(()=>$('materialchips').textContent.includes('Zwischenablagebild'));
  assert(ev.defaultPrevented && prompts.length===0,'Paste legt nur einen Chip an und sendet nichts');
  field.value='Was ist das?'; await submit();
  assert(prompts.length===1 && prompts[0].images?.[0]?.mimeType==='image/png','Senden nutzt gemeinsamen Material-Pfad');
  assert(!$('materialchips').textContent,'Chip nach bestätigtem Senden entfernt');
  const choose=chooseLocalMaterials(invoke);
  await until(()=>document.querySelectorAll('dialog .entries button').length===4);
  const buttons=()=>Array.from(document.querySelectorAll('dialog .entries button'));
  buttons()[1].click(); buttons()[3].click();
  previews['/demo/b.png']({text:'Bild B'}); await tick();
  previews['/demo/a.png']({text:'Veraltetes A'}); await tick();
  assert(document.querySelector('dialog .preview').textContent.includes('Bild B'),'neue Vorschau gewinnt');
  assert(!document.querySelector('dialog .preview').textContent.includes('Veraltetes A'),'alter Preview-Reply verworfen');
  buttons()[1].click(); document.querySelector('dialog select').dispatchEvent(new Event('change')); await tick();
  previews['/demo/a.png']({text:'falscher Ordner'}); await tick();
  assert(!document.querySelector('dialog .preview').textContent,'Ordnerwechsel leert und invalidiert Vorschau');
  buttons()[0].click(); document.querySelector('dialog .accept').click();
  assert((await choose)[0]==='/demo/a.png','Auswahl nach Vorschau weiterhin möglich');
  document.body.dataset.result='PASS';
};
const file=path.join(dir,'index.html');
fs.writeFileSync(file,fs.readFileSync(file,'utf8').replace('<script src="vendor/marked.js">','<script>('+stub+')()</script><script src="vendor/marked.js">').replace('</body>','<script>('+test+')().catch(e=>{document.body.dataset.result="FAIL: "+e.message;});</script></body>'));
const run=spawnSync('chromium',['--headless=new','--no-sandbox','--no-zygote','--disable-gpu','--disable-dev-shm-usage','--enable-logging=stderr','--user-data-dir='+dir+'/profile','--virtual-time-budget=4500','--dump-dom','file://'+file],{encoding:'utf8',timeout:90000,maxBuffer:3e6});
if(run.error?.code==='ENOENT'){console.log('SKIP: Chromium fehlt');process.exit(0);}
if(!run.stdout?.includes('data-result="PASS"')){fs.writeFileSync(path.join(dir,'result.html'),run.stdout||'');console.error(run.stdout?.match(/data-result="([^"]*)"/)?.[1]||run.error,run.stderr,dir);process.exit(1);}
console.log('Chat/Material-DOM: Sprung, sessiongebundene Prompts, Paste→Material→Senden und Vorschau-Race grün.');
