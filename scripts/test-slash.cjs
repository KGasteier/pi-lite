const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const {slashMatches} = require('../ui/slash.js');
const commands = [{name:'endpoint-custom',source:'extension',description:'Wizard'}, {name:'skill:meshdev',source:'skill',description:'Endpoint research'}, {name:'implement',source:'prompt'}, {name:'model',source:'local'}, {name:'llama',source:'extension',description:'Manage router models'}];
assert.equal(slashMatches(commands,'/en')[0].name,'endpoint-custom');
assert.equal(slashMatches(commands,'/').length,5);
assert.deepEqual(slashMatches(commands,'/mo').map(c=>c.name),['model']);
assert.deepEqual(slashMatches(commands,'/model').map(c=>c.name),['model']);
assert.deepEqual(slashMatches(commands,'/mesh').map(c=>c.name),['skill:meshdev']);
assert.deepEqual(slashMatches(commands,'/').map(c=>c.name),['endpoint-custom','implement','llama','model','skill:meshdev']);
for (const text of ['hello /en','/endpoint-custom arg','/en\n']) assert.equal(slashMatches(commands,text),null);
function el() { return {children:[],hidden:true,value:'/',selectionStart:1,selectionEnd:1,attrs:{},listeners:{},setAttribute(k,v){this.attrs[k]=v;},removeAttribute(k){delete this.attrs[k];},addEventListener(k,v){this.listeners[k]=v;},replaceChildren(){this.children=[];},append(...c){this.children.push(...c);},appendChild(c){this.children.push(c);},scrollIntoView(){},focus(){doc.activeElement=this;},setSelectionRange(a,b){this.selectionStart=a;this.selectionEnd=b;},dispatchEvent(e){this.listeners[e.type]?.(e);} }; }
const field=el(), popup=el(), list=el(), doc={activeElement:field,createElement:el};
const context=vm.createContext({document:doc,Event:class {constructor(type){this.type=type;}},field,popup,list,commands});
vm.runInContext(fs.readFileSync('ui/slash.js','utf8')+'\nlet executed=0; const picker=createSlashPicker(field,popup,list,()=>executed++); picker.setCommands(commands);',context);
function key(k){return vm.runInContext(`picker.key({key:${JSON.stringify(k)},preventDefault(){},stopPropagation(){}})`,context);}
assert.equal(popup.hidden,false);
key('ArrowUp'); assert.equal(list.children.at(-1).attrs['aria-selected'],'true');
key('Tab'); assert.ok(field.value.startsWith('/skill:')); assert.equal(popup.hidden,true);
field.value='/en';field.selectionStart=field.selectionEnd=3;field.dispatchEvent({type:'input'});
assert.equal(list.children[0].children[0].textContent,'/endpoint-custom');
list.children[0].onmouseenter();key('Enter');assert.equal(field.value,'/endpoint-custom ');
assert.equal(vm.runInContext('executed',context),1);
field.value='/mo';field.selectionStart=field.selectionEnd=3;field.dispatchEvent({type:'input'});list.children[0].onclick();assert.equal(field.value,'/model ');assert.equal(vm.runInContext('executed',context),2);
field.value='/';field.selectionStart=field.selectionEnd=1;field.dispatchEvent({type:'input'});key('Escape');assert.equal(popup.hidden,true);
assert.equal(key('ArrowUp'),false,'Closed picker leaves history keys alone');
console.log('Slash picker: filtering, wrap, hover, acceptance, Escape and history isolation passed');
