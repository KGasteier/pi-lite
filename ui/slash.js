function slashMatches(commands, value) {
  if (!/^\/[^\s]*$/.test(value)) return null;
  const query = value.slice(1).toLowerCase();
  return commands.filter(c => (c.name + ' ' + (c.description || '')).toLowerCase().includes(query))
    .sort((a,b) => Number(!a.name.toLowerCase().startsWith(query)) - Number(!b.name.toLowerCase().startsWith(query)) || a.name.localeCompare(b.name));
}
function createSlashPicker(field, popup, list) {
  let commands = [], matches = [], cursor = 0, dismissed = null;
  function close() { popup.hidden = true; field.setAttribute('aria-expanded', 'false'); field.removeAttribute('aria-activedescendant'); }
  function mark(scroll = true) {
    [...list.children].forEach((row,i) => row.setAttribute('aria-selected', String(i === cursor)));
    const row = list.children[cursor];
    if (row) { field.setAttribute('aria-activedescendant', row.id); if (scroll) row.scrollIntoView({block:'nearest'}); }
  }
  function choose() {
    if (!matches[cursor]) return;
    field.value = '/' + matches[cursor].name + ' '; close();
    field.focus(); field.setSelectionRange(field.value.length, field.value.length);
    field.dispatchEvent(new Event('input', {bubbles:true}));
  }
  function update() {
    const result = slashMatches(commands, field.value);
    if (document.activeElement !== field || field.selectionStart !== field.selectionEnd || field.selectionStart !== field.value.length || result === null || dismissed === field.value) { close(); return; }
    matches = result; cursor = 0; list.replaceChildren();
    matches.forEach((command,i) => {
      const row = document.createElement('div'); row.id = 'slash-option-' + i; row.setAttribute('role','option');
      const title = document.createElement('strong'); title.textContent = '/' + command.name;
      const source = document.createElement('small'); source.textContent = ({extension:'Extension',prompt:'Vorlage',skill:'Skill'})[command.source] || command.source;
      const description = document.createElement('span'); description.textContent = command.description || '';
      row.append(title, source, description);
      row.onmouseenter = () => { cursor = i; mark(false); };
      row.onmousedown = e => e.preventDefault(); row.onclick = () => { cursor = i; choose(); };
      list.appendChild(row);
    });
    if (!matches.length) { const empty = document.createElement('div'); empty.textContent = 'Keine passenden Befehle'; list.appendChild(empty); }
    popup.hidden = false; field.setAttribute('aria-expanded','true'); mark();
  }
  field.addEventListener('input', () => { dismissed = null; update(); });
  field.addEventListener('focus', update);
  field.addEventListener('blur', close);
  field.addEventListener('click', update);
  return {
    setCommands(value) { commands = value.filter(c => typeof c.name === 'string'); update(); },
    close,
    key(e) {
      if (popup.hidden || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return false;
      if (!['ArrowUp','ArrowDown','Tab','Enter','Escape'].includes(e.key)) return false;
      e.preventDefault(); e.stopPropagation();
      if (e.key === 'Escape') { dismissed = field.value; close(); }
      else if (e.key === 'Tab' || e.key === 'Enter') { if (matches.length) choose(); else { dismissed = field.value; close(); } }
      else if (matches.length) { cursor = (cursor + (e.key === 'ArrowDown' ? 1 : -1) + matches.length) % matches.length; mark(); }
      return true;
    }
  };
}
if (typeof module !== 'undefined') module.exports = {slashMatches};
