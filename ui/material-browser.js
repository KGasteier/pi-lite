// Lokaler Browser: Originalpfade statt Kopien. Kein Netzwerk-/KI-Aufruf.
async function chooseLocalMaterials(invoke) {
  return new Promise((resolve, reject) => {
    const dialog = document.createElement('dialog');
    dialog.style.cssText='background:var(--panel);color:var(--fg);border:1px solid var(--acc);border-radius:8px;width:min(850px,90vw);max-height:88vh;overflow:auto';
    dialog.innerHTML='<h3>Material · Originaldateien auswählen</h3><div style="display:flex;gap:6px"><input aria-label="Ordnerpfad" style="flex:1"><button class="up">Hoch</button><select aria-label="Sortierung"><option value="name">Name</option><option value="date">Datum ↓</option></select></div><div class="entries" style="height:min(260px,32vh);overflow:auto;margin:8px 0"></div><section class="preview" style="max-height:min(240px,26vh);overflow:auto"></section><p>Nur Originalverweise · Vorschau bleibt lokal · keine Kopien</p><button class="accept">Auswahl übernehmen</button> <button class="cancel">Abbrechen</button>';
    document.body.appendChild(dialog); dialog.showModal();
    const field=dialog.querySelector('input'), sort=dialog.querySelector('select'), entries=dialog.querySelector('.entries'), preview=dialog.querySelector('.preview');
    let selected=new Set(), current=null, sequence=0, previewSequence=0, closed=false;
    const close=value=>{if(closed)return;closed=true;sequence++;previewSequence++;dialog.close();dialog.remove();resolve(value);};
    dialog.querySelector('.cancel').onclick=()=>close([]);
    dialog.querySelector('.accept').onclick=()=>close([...selected]);
    dialog.oncancel=e=>{e.preventDefault();close([]);};
    async function show(path){
      const seq=++previewSequence, label=path.replaceAll('\\','/').split('/').pop();
      preview.textContent='Vorschau: '+label+' wird geladen …'; preview.scrollTop=0;
      try {const data=await invoke('material_preview',{path});if(closed||seq!==previewSequence)return;
        preview.replaceChildren();
        const heading=document.createElement('strong');heading.textContent='Vorschau: '+label;preview.appendChild(heading);
        const text=document.createElement('div');text.style.cssText='white-space:pre-wrap;overflow-wrap:anywhere';text.textContent=data.text;preview.appendChild(text);
        if(data.png){const img=document.createElement('img');img.alt='Lokale Vorschau: '+label;img.style.cssText='max-width:100%;max-height:min(200px,20vh)';img.src='data:image/png;base64,'+data.png;preview.appendChild(img);}
      }catch(e){if(!closed&&seq===previewSequence)preview.textContent='Vorschau: '+label+' · '+String(e);}
    }
    async function browse(path){
      const seq=++sequence;
      previewSequence++; preview.replaceChildren(); current=null;
      entries.textContent='Ordner wird geladen …';
      try {const result=await invoke('material_browser',{path:path||null,byDate:sort.value==='date'});if(closed||seq!==sequence)return;
        current=result;field.value=result.path;entries.replaceChildren();
        for(const entry of result.entries){
          const row=document.createElement('div');row.style.cssText='display:flex;gap:8px;padding:3px';
          const b=document.createElement('button');b.style.cssText='flex:1;text-align:left';
          b.textContent=(entry.is_dir?'Ordner: ':selected.has(entry.path)?'✓ ':'')+entry.name;
          b.onclick=()=>{if(entry.is_dir)browse(entry.path);else {previewSequence++;preview.replaceChildren();selected.has(entry.path)?selected.delete(entry.path):selected.add(entry.path);b.textContent=(selected.has(entry.path)?'✓ ':'')+entry.name;}};
          row.appendChild(b);
          if(!entry.is_dir){const v=document.createElement('button');v.textContent='Vorschau';v.onclick=()=>show(entry.path);row.appendChild(v);}
          entries.appendChild(row);
        }
      }catch(e){if(!closed&&seq===sequence)entries.textContent=String(e);}
    }
    dialog.querySelector('.up').onclick=()=>current?.parent&&browse(current.parent);
    sort.onchange=()=>browse(field.value);field.onkeydown=e=>{if(e.key==='Enter'){e.preventDefault();browse(field.value);}};
    browse(null);
  });
}
