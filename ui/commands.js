// Eingabe wie im pi-Terminal: "pi update" (CLI), "!cmd" / "!!cmd" (Shell), "/builtin" lokal, sonst Prompt.
const PI_CLI_SUBCOMMANDS = ['update', 'install', 'remove', 'uninstall', 'list', 'auth', 'mcp'];
const PI_CLI_FLAGS = ['--version', '-v', '--help', '-h'];
// Eingebaute pi-Befehle, die über RPC-Funktionen nachgebildet werden (get_commands liefert sie nicht).
const LOCAL_COMMANDS = [
  { name: 'model', description: 'Modell auswählen' },
  { name: 'thinking', description: 'Denkstufe setzen oder anzeigen' },
  { name: 'new', description: 'Neue Session starten' },
  { name: 'name', description: 'Session benennen' },
  { name: 'session', description: 'Sessioninfo und Statistik' },
  { name: 'compact', description: 'Kontext komprimieren' },
  { name: 'copy', description: 'Letzte Antwort kopieren' },
  { name: 'export', description: 'Session als HTML exportieren' },
  { name: 'clone', description: 'Session an dieser Stelle duplizieren' },
  { name: 'fork', description: 'Ab früherer Nachricht abzweigen' },
  { name: 'resume', description: 'Andere Session öffnen (Seitenleiste)' },
  { name: 'hotkeys', description: 'Tastaturhilfe' },
  { name: 'changelog', description: 'pi-Changelog im Browser' },
  { name: 'quit', description: 'pi-lite beenden' },
].map(c => ({ ...c, source: 'local' }));
// Nur im Terminal-UI sinnvoll; ehrlicher Hinweis statt Weiterleitung an das Modell.
const TERMINAL_ONLY = ['settings', 'scoped-models', 'login', 'logout', 'llama', 'tree', 'import', 'share', 'bug', 'trust', 'reload'];

function parseInput(text) {
  const t = text.trim();
  if (t.startsWith('!')) {
    const exclude = t.startsWith('!!');
    const command = t.slice(exclude ? 2 : 1).trim();
    return command ? { kind: 'bash', command, exclude } : { kind: 'prompt' };
  }
  const words = t.split(/\s+/);
  if (words[0] === 'pi' && words.length > 1 && (PI_CLI_SUBCOMMANDS.includes(words[1]) || PI_CLI_FLAGS.includes(words[1]))) {
    return { kind: 'cli', args: words.slice(1) };
  }
  const slash = /^\/([^\s]+)\s*([\s\S]*)$/.exec(t);
  if (slash) {
    const name = slash[1].toLowerCase(), arg = slash[2].trim();
    if (LOCAL_COMMANDS.some(c => c.name === name)) return { kind: 'local', name, arg };
    if (TERMINAL_ONLY.includes(name)) return { kind: 'terminal', name };
  }
  return { kind: 'prompt' };
}

// Semver-Vergleich ohne Abhängigkeit; Vorabversionen zählen als älter als die Release.
function isNewerVersion(latest, current) {
  const parse = v => { const [core, pre] = String(v || '').replace(/^v/, '').split('-'); return { n: core.split('.').map(x => parseInt(x, 10) || 0), pre: !!pre }; };
  const a = parse(latest), b = parse(current);
  for (let i = 0; i < 3; i++) { if ((a.n[i] || 0) !== (b.n[i] || 0)) return (a.n[i] || 0) > (b.n[i] || 0); }
  return b.pre && !a.pre;
}
if (typeof module !== 'undefined') module.exports = { parseInput, isNewerVersion, LOCAL_COMMANDS, TERMINAL_ONLY };
