// Nur Anzeige: Zustände kommen vom Hub, niemals aus lokalen Agentenzählern.
const labels = Object.freeze({ ruht: 'ruht', arbeitet: 'arbeitet', wartet: 'wartet auf dich', fertig: 'fertig', bereit: '', fehler: 'Fehler' });
export function sessionLabel(state) { return labels[state] ?? 'Zustand unbekannt'; }
export function connectionLabel(state) {
  return ({ verbunden: 'verbunden', unterbrochen: 'Verbindung unterbrochen', abgemeldet: 'abgemeldet', offline: 'offline' })[state] || 'Verbindung wird geprüft …';
}
export function errorText(error, art = 'k') {
  const texts = {
    abgelaufen: art === 'e' ? 'Diese Einladung ist abgelaufen. Bitte deine Seminarleitung um eine neue.' : 'Der Code ist abgelaufen. Tippe am Rechner noch einmal auf ‚Handy koppeln‘.',
    verbraucht: art === 'e' ? 'Diese Einladung wurde schon benutzt. Jede Einladung gilt nur einmal. Wenn du das nicht warst, sag deiner Seminarleitung Bescheid.' : 'Dieser Code wurde schon benutzt. Erzeuge am Rechner einen neuen.',
    abgelehnt: 'Der Rechner hat abgelehnt. Es wurde nichts gespeichert.',
    zu_viele_versuche: 'Zu viele Versuche. Warte eine Minute und versuche es dann noch einmal.',
    abgemeldet: 'Dieses Gerät wurde abgemeldet. Melde dich mit einer neuen Einladung oder durch Koppeln wieder an.',
  };
  return texts[error.code] || error.message || [error.was, error.tun].filter(Boolean).join(' ') || 'R2 ist gerade nicht erreichbar. Prüfe deine Internetverbindung.';
}
export function messageText(message) {
  if (typeof message === 'string') return message;
  if (typeof message?.content === 'string') return message.content;
  return (message?.content || []).filter(part => part.type === 'text').map(part => part.text).join('\n');
}
