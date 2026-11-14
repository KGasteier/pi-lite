// Hermes-REST-Adapter. Keine Python-/CLI-Interna, keine automatischen Freigaben.
// Vertrag: docs/HERMES.md (Upstream-Referenz und bewusste Funktionsgrenzen).
import crypto from 'node:crypto';

export const isHermesSession = s => typeof s === 'string' && s.startsWith('hermes:');
const idOf = s => {
  const id = isHermesSession(s) ? s.slice(7) : s;
  if (typeof id !== 'string' || !id || id.length > 256 || /[\s/\\?#\x00-\x1f]/.test(id)) throw new Error('Ungültige Hermes-ID');
  return id;
};
const enc = s => encodeURIComponent(idOf(s));
const text = c => typeof c === 'string' ? c : Array.isArray(c) ? c.filter(x=>['text','output_text'].includes(x.type)).map(x=>x.text||'').join('') : '';
const message = (role, content) => ({role,content:[{type:'text',text:text(content)}]});
const terminal = new Set(['completed','cancelled','failed','interrupted']);

export class HermesApi {
  constructor(env = process.env) {
    this.key = env.HERMES_API_KEY || '';
    this.url = env.HERMES_API_URL || 'http://127.0.0.1:8642';
    this.enabled = !!env.HERMES_API_KEY;
    const u = new URL(this.url);
    if (u.protocol !== 'http:' || !['127.0.0.1','localhost','[::1]'].includes(u.hostname) || u.username || u.password || u.search || u.hash) {
      throw new Error('Hermes muss am Hub auf Loopback-HTTP liegen (ggf. SSH-Tunnel); keine URL-Zugangsdaten');
    }
    this.url = this.url.replace(/\/$/,'');
  }
  async request(route, body, options = {}) {
    if (!this.enabled) throw new Error('Hermes ist nicht konfiguriert (HERMES_API_KEY am Hub)');
    const r = await fetch(this.url + route, {
      method:body === undefined?'GET':'POST', redirect:'error',
      headers:{authorization:'Bearer '+this.key,'content-type':'application/json',...options.headers},
      body:body === undefined?undefined:JSON.stringify(body), signal:options.signal || AbortSignal.timeout(15000)
    });
    if (!r.ok) throw Object.assign(new Error('Hermes antwortet mit HTTP '+r.status),{status:r.status});
    if(options.stream) return r;
    const len = Number(r.headers.get('content-length'));
    if(len > 8e6) throw new Error('Hermes-Antwort zu groß');
    const parts=[]; let size=0;
    for await(const b of r.body) { size+=b.length; if(size>8e6) throw new Error('Hermes-Antwort zu groß'); parts.push(b); }
    return JSON.parse(Buffer.concat(parts).toString('utf8'));
  }
  async sessions(cwd) {
    if(!this.enabled) return [];
    const r=await this.request('/api/sessions?source=api_server&limit=200');
    if(!Array.isArray(r.data)) throw new Error('Hermes: ungültige Sessionliste');
    return r.data.filter(s=>s.source==='api_server').map(s=>({
      path:'hermes:'+idOf(s.id), harness:'hermes', cwd,
      modified:Math.round(Number(s.last_active||s.started_at||0)*1000), title:'Hermes · '+(s.title||s.id)
    }));
  }
}

// SSE: benannte Events, CRLF, Mehrzeilen-data, UTF-8 über Chunkgrenzen, feste Speichergrenze.
export async function readHermesSse(response, on, signal) {
  const reader=response.body.getReader(), decoder=new TextDecoder();
  let acc='', name='', data=[];
  const dispatch=()=>{if(data.length) on(name,JSON.parse(data.join('\n')));name='';data=[];};
  try {
    while(true) {
      let timer;
      const next=await Promise.race([reader.read(),new Promise((_,no)=>{timer=setTimeout(()=>no(new Error('Hermes-Stream: keine Daten seit 90 Sekunden')),90000);})]).finally(()=>clearTimeout(timer));
      if(signal.aborted) throw new Error('Hermes-Stream beendet');
      acc+=decoder.decode(next.value,{stream:!next.done});
      if(acc.length+data.join('').length>2e6) throw new Error('Hermes-SSE-Ereignis zu groß');
      let n;
      while((n=acc.indexOf('\n'))>=0) {
        const l=acc.slice(0,n).replace(/\r$/,'');acc=acc.slice(n+1);
        if(!l) dispatch(); else if(l.startsWith('event:')) name=l.slice(6).trim();
        else if(l.startsWith('data:')) data.push(l.slice(5).replace(/^ /,''));
      }
      if(next.done) {if(acc.trim()) throw new Error('Hermes-SSE unvollständig');dispatch();return;}
    }
  } finally {await reader.cancel().catch(()=>{}); reader.releaseLock();}
}

export function hermesHarness(Instance, api, persist = ()=>{}) {
  return class HermesInstance extends Instance {
    constructor(o) {super(o);this.resumeRun=o.hermesRunId;this.admissionPending=o.hermesAdmissionPending;}
    start() {
      this.closed=false; this.active=false; this.runId=null; this.controls=new Map(); this.history=[];
      this.ready=this.initialize().catch(e=>{this.error(e);this.emit({type:'hub_exit',code:-1});});
    }
    error(e) { this.emit({type:'hub_stderr',text:(api.key ? String(e.message||e).split(api.key).join('[redacted]') : String(e.message||e))}); }
    summary() {return {...super.summary(),capabilities:{prompt:true,abort:true,history:true,approval:this.features?.run_approval_response===true,clarify:this.features?.run_approval_response===true,fork:false,clone:false,model:false,thinking:false},runId:this.runId||null};}
    async initialize() {
      const cap=await api.request('/v1/capabilities');
      for(const f of ['run_submission','run_events_sse','run_status','run_stop']) if(cap.features?.[f]!==true) throw new Error('Hermes unterstützt '+f+' nicht');
      this.features=cap.features;
      if(this.session) {
        const r=await api.request('/api/sessions/'+enc(this.session));
        if(r.session?.source!=='api_server') throw new Error('Nur Hermes-API-Sessions steuerbar; CLI/TUI wird nicht übernommen');
      } else {
        const r=await api.request('/api/sessions',{title:this.name,source:'api_server'});
        this.session='hermes:'+idOf(r.session?.id);
      }
      const r=await api.request('/api/sessions/'+enc(this.session)+'/messages?inline_images=false&limit=500&order=latest');
      if(!Array.isArray(r.data)) throw new Error('Hermes: ungültiger Verlauf');
      this.history=r.data.filter(m=>['user','assistant'].includes(m.role)).map(m=>message(m.role,m.content));
      if(this.closed) return;
      // Nur Verlauf, kein neuer Modelllauf. Ein späteres subscribe kann alles wiedergeben.
      for(const m of this.history) {this.emit({type:'message_start',message:m});this.emit({type:'message_end',message:m});}
      if(this.resumeRun) {
        const r=await api.request('/v1/runs/'+enc(this.resumeRun));
        if(!terminal.has(r.status)) {
          this.active=true;this.runId=this.resumeRun;this.output='';this.tools=new Map();this.counter=0;
          this.streamController=new AbortController();this.emit({type:'agent_start'});
          void this.observe().catch(e=>this.error(e));
        }
        this.resumeRun=null;
      } else if(this.admissionPending) {
        this.active=true;this.setState('waiting');
        this.error(new Error('Vor Hub-Neustart war die Hermes-Auftragsannahme unklar. Kein Neuversand; am Host klären.'));
      }
      this.stateReply('hub-state'); if(!this.active) this.setState('idle'); persist();
    }
    remember(m) {this.history.push(m); if(this.history.length>500) this.history.splice(0,this.history.length-500);}
    stateReply(id) {
      this.emit({type:'response',id,command:'get_state',success:true,data:{sessionFile:this.session,sessionName:this.name,isStreaming:this.active,harness:'hermes',model:{id:'Hermes-Konfiguration',provider:'hermes'},capabilities:this.summary().capabilities}});
    }
    write(cmd) {void this.command(cmd).catch(e=>{this.error(e);if(cmd.id)this.emit({type:'response',id:cmd.id,command:cmd.type,success:false,error:String(e.message||e)});});}
    async command(cmd) {
      await this.ready;
      if(this.closed || this.state==='exited') throw new Error('Hermes-Instanz ist beendet');
      const reply=data=>this.emit({type:'response',id:cmd.id,command:cmd.type,success:true,data});
      if(cmd.type==='get_state') return this.stateReply(cmd.id);
      if(cmd.type==='get_messages') return reply({messages:this.history});
      if(cmd.type==='get_available_models') return reply({models:[]});
      if(cmd.type==='get_commands') return reply({commands:[]});
      if(cmd.type==='abort') {this.cancelRequested=true; if(this.runId) await api.request('/v1/runs/'+enc(this.runId)+'/stop',{});return reply({});}
      if(cmd.type==='extension_ui_response') return this.answer(cmd);
      if(cmd.type!=='prompt') throw new Error('Hermes: '+cmd.type+' wird nicht unterstützt');
      if(this.active) throw new Error('Hermes arbeitet bereits; kein zweiter Prompt/Steer');
      if(cmd.images?.length || cmd.attachments?.length) throw new Error('Hermes: Anhänge sind noch nicht implementiert');
      if(typeof cmd.message!=='string' || !cmd.message.trim()) throw new Error('Prompt fehlt');
      this.active=true;this.cancelRequested=false;this.output='';this.tools=new Map();this.counter=0;
      persist();
      this.emit({type:'agent_start'});
      const m=message('user',cmd.message); this.remember(m);
      this.emit({type:'message_start',message:m});this.emit({type:'message_end',message:m});
      // Admission nicht automatisch wiederholen. Key erlaubt spätere sichere Diagnose desselben Auftrags.
      try {
        const r=await api.request('/v1/runs',{input:cmd.message,session_id:idOf(this.session)},{headers:{'Idempotency-Key':crypto.randomUUID()}});
        this.runId=idOf(r.run_id); persist();
        reply({disposition:'started'});
        this.emit({type:'message_start',message:{role:'assistant',content:[]}});
        this.streamController=new AbortController();
        void this.observe().catch(e=>this.error(e));
        if(this.cancelRequested) {
          // Stopfehler nach erfolgreicher Admission sind kein fehlgeschlagener Prompt.
          // Lauf weiter beobachten und sperren, keinesfalls vorzeitig idle melden.
          try {await api.request('/v1/runs/'+enc(this.runId)+'/stop',{});} catch(e) {this.error(e);}
        }
      } catch(e) {
        if(e.status) this.finish('failed',{});
        else {this.error(new Error('Ausführungszustand unklar; kein automatischer Neuversand. Hermes am Host prüfen.'));this.setState('waiting');}
        throw e;
      }
    }
    async observe() {
      try {
        const r=await api.request('/v1/runs/'+enc(this.runId)+'/events',undefined,{stream:true,signal:this.streamController.signal});
        await readHermesSse(r,(name,p)=>this.event(name||p.event||p.type,p),this.streamController.signal);
      } catch(e) {if(!this.closed)this.error(e);}
      // Verbindungsende ist KEIN Erfolg. Polling versendet weder Prompt noch Freigabe erneut.
      while(this.active && !this.closed) {
        try {
          const r=await api.request('/v1/runs/'+enc(this.runId));
          if(terminal.has(r.status)) {this.finish(r.status,r);break;}
          for(const p of r.pending_controls||[]) this.control(p.kind==='clarify'?'clarify.request':'approval.request',p);
          if(r.approval) this.control('approval.request',r.approval);
        } catch(e) {this.error(e);}
        await new Promise(ok=>setTimeout(ok,2000));
      }
    }
    event(name,p) {
      if(!this.active || this.closed) return;
      if(p.run_id && p.run_id!==this.runId) return;
      if(name==='message.delta' || name==='assistant.delta') {
        this.output+=p.delta||'';
        if(this.output.length>4e6) throw new Error('Hermes-Textpuffer zu groß; Abschluss wird per Status nachgeladen');
        this.emit({type:'message_update',assistantMessageEvent:{type:'text_delta',delta:p.delta||''},raw:{event:name,data:p}});
      } else if(name==='tool.started') {
        const name=p.tool||p.tool_name||'tool', id=p.tool_call_id||'hermes-tool-'+(++this.counter);
        this.tools.set(id,name);
        this.emit({type:'tool_execution_start',toolCallId:id,toolName:name,args:p.args||{preview:p.preview||''},raw:{event:'tool.started',data:p}});
      } else if(['tool.completed','tool.failed'].includes(name)) {
        const tool=p.tool||p.tool_name||'tool';
        const id=p.tool_call_id||[...this.tools].find(([,n])=>n===tool)?.[0]||'hermes-tool-'+(++this.counter);
        this.tools.delete(id);
        this.emit({type:'tool_execution_end',toolCallId:id,toolName:tool,result:{content:[{type:'text',text:p.preview||''}]},isError:name==='tool.failed'||p.error===true,raw:{event:name,data:p}});
      } else if(name==='approval.request'||name==='clarify.request') this.control(name,p);
      else if(name==='approval.settled'||name==='clarify.settled') {
        const id=p.prompt_id||p.request_id;
        for(const [key,c] of this.controls) if(c.request_id===id) {this.controls.delete(key);this.asks.delete(key);}
        this.setState(this.asks.size?'waiting':'busy');
      } else if(name.startsWith('run.') && terminal.has(name.slice(4))) this.finish(name.slice(4),p);
      else if(name==='error') this.error(new Error('Hermes meldet einen Streamfehler; Status wird geprüft'));
    }
    control(kind,p) {
      // Upstream nennt das Feature run_approval_response (run_approval ist ein Endpoint-Name).
      if(this.features?.run_approval_response!==true || (kind==='clarify.request' && !Number.isInteger(p.execution_generation))) {
        this.error(new Error('Hermes bietet die Rückfrage-Steuerung nicht an; bitte Lauf stoppen')); return;
      }
      const request_id=p.prompt_id||p.request_id;
      if(!request_id || typeof request_id!=='string') {this.error(new Error('Hermes-Rückfrage ohne eindeutige ID; nur Abbruch möglich'));return;}
      const generation=p.execution_generation;
      const id=this.runId+':'+request_id+':'+(generation??'legacy');
      if(this.controls.has(id)) return;
      this.controls.set(id,{kind,request_id,...(Number.isInteger(generation)?{execution_generation:generation}:{})});
      this.emit({type:'extension_ui_request',id,method:kind==='approval.request'?'confirm':'input',
        title:kind==='approval.request'?'Hermes: einmalig freigeben?':'Hermes: Rückfrage',
        message:[p.description,p.command,p.question,p.prompt].filter(x=>typeof x==='string').join('\n')||'Hermes wartet auf eine Entscheidung.',raw:{event:kind,data:p}});
    }
    async answer(cmd) {
      const c=this.controls.get(cmd.id); if(!c || !this.active) throw new Error('Hermes-Rückfrage veraltet oder bereits beantwortet');
      const route=c.kind==='approval.request'?'approval':'clarify';
      if(route==='clarify' && (cmd.cancelled || typeof cmd.value!=='string')) throw new Error('Rückfrage abbrechen: bitte den Lauf stoppen');
      const {kind,...identity}=c;
      try {await api.request('/v1/runs/'+enc(this.runId)+'/'+route,{...identity,...(route==='approval'?{choice:cmd.confirmed===true&&!cmd.cancelled?'once':'deny'}:{answer:cmd.value})});}
      catch(e) { // Instance.send hat den Dialog voreilig entfernt: ohne Bestätigung wieder anbieten.
        const old=this.buf.findLast(r=>r.ev.type==='extension_ui_request'&&r.ev.id===cmd.id)?.ev;
        if(old) this.emit(old); throw e;
      }
      this.controls.delete(cmd.id);this.asks.delete(cmd.id);this.setState(this.asks.size?'waiting':'busy');
    }
    finish(status,p) {
      if(!this.active) return;
      const full=typeof p.output==='string'?p.output:this.output;
      if(full) {const m=message('assistant',full);this.remember(m);this.emit({type:'message_end',message:m});}
      if(status==='failed'||status==='interrupted') this.error(new Error('Hermes-Lauf '+status));
      this.active=false;this.controls.clear();this.runId=null;persist();
      this.emit({type:'agent_end',messages:[],willRetry:false});this.emit({type:'agent_settled',aborted:status!=='completed'});
    }
    async stop() {
      this.cancelRequested=true;
      if(this.active && !this.runId) throw new Error('Hermes-Zustand unklar; Instanz bis zur Klärung nicht entfernen');
      if(this.runId) {
        const run=this.runId;
        await api.request('/v1/runs/'+enc(run)+'/stop',{});
        const deadline=Date.now()+5000,signal=AbortSignal.timeout(5000);
        while(this.active && Date.now()<deadline) {
          const p=await api.request('/v1/runs/'+enc(run),undefined,{signal});
          if(terminal.has(p.status)) {this.finish(p.status,p);break;}
          await new Promise(ok=>setTimeout(ok,250));
        }
        if(this.active) throw new Error('Hermes-Abbruch angefordert, Run noch nicht beendet; Instanz bleibt erhalten');
      }
      this.closed=true;this.streamController?.abort();this.emit({type:'hub_exit',code:0});
    }
  };
}
