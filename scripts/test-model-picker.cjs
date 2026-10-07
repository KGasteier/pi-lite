const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const source = fs.readFileSync('ui/app.js', 'utf8');
const elements = {};
function el() { return { children: [], hidden: true, setAttribute() {}, focus() {}, replaceChildren() { this.children = []; }, appendChild(c) { this.children.push(c); }, scrollIntoView() {}, dispatchEvent() {}, clientHeight: 100 }; }
for (const name of ['model', 'modelbutton', 'modellist']) elements[name] = el();
elements.model.options = Array.from({length: 80}, (_, i) => ({textContent: `Model ${i}`}));
elements.model.selectedIndex = 0;
const context = vm.createContext({ $: id => elements[id], document: {createElement: el, addEventListener() {}}, Event: class {} });
vm.runInContext(source.slice(source.indexOf('let modelCursor = 0;'), source.indexOf('$("model").onchange =')), context);
vm.runInContext('openModelPicker()', context);
function key(key) { elements.modellist.onkeydown({key, preventDefault() {}, stopPropagation() {}}); }
for (let i=0;i<100;i++) key('ArrowDown');
assert.equal(vm.runInContext('modelCursor', context),79);
assert.equal(elements.model.selectedIndex,0, 'Navigation must not change the model');
for (let i=0;i<100;i++) key('ArrowUp');
assert.equal(vm.runInContext('modelCursor', context),0);
key('End'); assert.equal(vm.runInContext('modelCursor', context),79);
key('Home'); key('PageDown'); assert.ok(vm.runInContext('modelCursor', context)>0);
key('Escape'); assert.equal(elements.modellist.hidden,true);
console.log('Model picker: navigation reaches both ends, no switch until confirmation, Escape closes');
