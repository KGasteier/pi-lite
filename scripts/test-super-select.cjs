const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('ui/app.js', 'utf8');
const block = source.slice(source.indexOf('let superHeld = false;'), source.indexOf('$("prompt").addEventListener("keydown"'));
const handlers = {};
const context = { document: { addEventListener: (type, fn) => handlers[type] = fn }, window: { addEventListener: (type, fn) => handlers[type] = fn } };
vm.createContext(context); vm.runInContext(block, context);
function check(extra, expected) {
  context.event = {key:'a', code:'KeyA', getModifierState:()=>false, ...extra};
  assert.equal(vm.runInContext('isSelectAllShortcut(event)', context), expected);
}
check({metaKey:true}, true);
check({getModifierState:k=>k==='Super'}, true);
check({}, false);
handlers.keydown({key:'Super'}); check({}, true);
check({altKey:true}, false); check({shiftKey:true}, false); check({ctrlKey:true}, false);
handlers.keyup({key:'Super'}); check({}, false);
handlers.keydown({key:'OS'}); check({}, true);
handlers.blur(); check({}, false);
console.log('Super+A: Meta, Super/OS modifiers, held-key fallback and blur reset passed');
