// Jede Eingabe wird genau einmal abgeschickt. Unklare Versandlage bleibt gesperrt.
export class SessionActions {
  constructor(transport, changed = () => {}) { this.transport = transport; this.changed = changed; this.states = new Map(); }
  state(id) { return this.states.get(id) || { status: 'idle' }; }
  clear() { this.states.clear(); }
  async run(id, kind, ...args) {
    if (this.state(id).status !== 'idle' && !(kind === 'stopp' && this.state(id).status === 'uncertain')) return;
    const state = { status: 'pending', kind, client_id: kind === 'senden' ? crypto.randomUUID() : undefined };
    this.states.set(id, state); this.changed();
    try {
      const result = kind === 'senden' ? await this.transport.senden(id, args[0], state.client_id) : await this.transport[kind](id, ...args);
      if (this.states.get(id) !== state) return;
      this.states.delete(id); this.changed(); return result;
    } catch (error) {
      if (this.states.get(id) === state) {
        // HTTP-4xx ist eine bestätigte Ablehnung; Netzwerk/5xx kann bereits ausgeführt sein.
        if (error.status >= 400 && error.status < 500) this.states.delete(id);
        else state.status = 'uncertain';
        this.changed();
      }
      throw error;
    }
  }
}
export function personalResults(packet) {
  return (packet.persoenlich || []).filter(row => typeof row.pfad === 'string'
    && row.pfad.startsWith('Ergebnisse/') && row.pfad.split('/').every(part => part && part !== '.' && part !== '..')
    && !/[\\\u0000-\u001f?#]/.test(row.pfad));
}
export function previewKind(file) {
  const name = file.toLowerCase();
  if (/\.(png|jpe?g|gif|webp|avif)$/.test(name)) return 'image';
  if (name.endsWith('.pdf')) return 'pdf';
  if (/\.(txt|md|csv|json|ya?ml|css|js|rs|py|html|svg)$/.test(name)) return 'text';
  return null;
}
export function attribution(message) {
  return message.von?.rolle === 'seminarleitung' ? `${message.von.person || 'Seminarleitung'} · Seminarleitung`
    : ({ user: 'Du', assistant: 'Agent', toolResult: 'Werkzeug' })[message.role] || 'Nachricht';
}
