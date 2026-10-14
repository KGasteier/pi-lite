const assert = require('node:assert/strict');
const fs = require('node:fs');
const html = fs.readFileSync('ui/index.html', 'utf8');
assert(!/<option[^>]*value="harnesses"/.test(html));
const { createDurationEstimate } = require('../ui/duration-estimate.js');
// Format bleibt bewusst eine Schätzung; Rust prüft zusätzlich die Regular-Glyphabdeckung.
assert(fs.readFileSync('ui/duration-estimate.js','utf8').includes('`≈${pct}%'));
console.log('Sidebar ohne andere Harnesses, Schätzung mit ≈.');
