// Selection dialogs use one synchronized keyboard/mouse cursor, not a static primary button.
function setupDialogSelection(box, hint, options, choose) {
  let cursor = 0;
  const buttons = options.map((option, index) => {
    const button = document.createElement('button'); button.textContent = option;
    button.setAttribute('role', 'option'); button.setAttribute('aria-selected', 'false');
    button.onclick = () => choose(option);
    button.onfocus = () => mark(index);
    button.onmouseenter = () => { button.focus({preventScroll:true}); };
    box.appendChild(button); return button;
  });
  box.setAttribute('role', 'listbox'); box.setAttribute('aria-label', 'Auswahl');
  function mark(index) {
    cursor = index;
    buttons.forEach((button, i) => button.setAttribute('aria-selected', String(i === index)));
    hint.textContent = `${options.length ? index + 1 : 0} / ${options.length} Einträge · ↑↓ wählen · Home/End · Bild↑/↓ · Enter bestätigen · Esc abbrechen`;
    buttons[index]?.scrollIntoView({block:'nearest'});
  }
  mark(0);
  return {
    first: buttons[0],
    key(e) {
      if (!['ArrowUp','ArrowDown','Home','End','PageUp','PageDown'].includes(e.key) || e.altKey || e.ctrlKey || e.metaKey) return false;
      if (!buttons.length || !box.contains(document.activeElement)) return false;
      const page = Math.max(1, Math.floor(box.clientHeight / (buttons[cursor].offsetHeight + 6)));
      const delta = {ArrowUp:-1,ArrowDown:1,PageUp:-page,PageDown:page}[e.key];
      const next = e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : Math.max(0, Math.min(buttons.length - 1, cursor + delta));
      e.preventDefault(); e.stopImmediatePropagation(); buttons[next].focus(); return true;
    }
  };
}
