const assert=require('node:assert/strict');
const {createDurationEstimate}=require('../ui/duration-estimate.js');
const {createCategoryColorNames}=require('../ui/category-colors.js');
const d=new Map(),s={getItem:k=>d.get(k),setItem:(k,v)=>d.set(k,v)};
const e=createDurationEstimate(s);assert.equal(e.estimate().seconds,180);
[10,20,30].forEach(n=>e.record(n));assert.equal(e.estimate().seconds,20);assert(e.percent(21).overdue);assert(e.label(21).includes('+'));assert(e.label(10).includes('≈'));
for(let i=0;i<50;i++)e.record(40);assert.equal(e.history.length,30);assert.equal(createDurationEstimate(s).history.length,30);
const c=createCategoryColorNames(s,['Blau','Grün']);c.rename(0,'Projekte');assert.equal(createCategoryColorNames(s,['Blau','Grün']).nameOf(0),'Projekte');c.rename(0,'');assert.equal(c.nameOf(0),'Blau');
console.log('Laufzeitlernen, Überschreitung und persistente Kategorien geprüft.');
