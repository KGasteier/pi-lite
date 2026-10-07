const assert = require('node:assert/strict');
const {editorReference: parse} = require('../ui/editor.js');
assert.deepEqual(parse('ui/app.js:630:5'), {path:'ui/app.js',line:630,column:5});
assert.deepEqual(parse('/tmp/file name.md#L12'), {path:'/tmp/file name.md',line:12,column:1});
assert.deepEqual(parse('file:///tmp/file%20name.md#L3'), {path:'/tmp/file name.md',line:3,column:1});
assert.deepEqual(parse('README.md'), {path:'README.md',line:null,column:null});
for (const value of ['https://example.com/a.md','javascript:alert(1)','file://remote/a.md','hello','a.md\ncommand','*.md']) assert.equal(parse(value),null,value);
console.log('Editor references: paths, line/column, file URLs and invalid inputs passed');
