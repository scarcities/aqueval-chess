#!/usr/bin/env node
// Writes a random 768 -> 256 -> 1 demo net using the loader inside netviz.html (single source of truth).
// Usage: node tools/netviz/gen_demo_net.js [out=tools/netviz/demo.qnet] [seed=1]
const fs = require('fs');
const path = require('path');

const html = fs.readFileSync(path.join(__dirname, 'netviz.html'), 'utf8');
const code = html.match(/<script id="netio">([\s\S]*?)<\/script>/)[1];
const NetIO = new Function(`${code}; return NetIO;`)();

const out = process.argv[2] || path.join(__dirname, 'demo.qnet');
const net = NetIO.demo(Number(process.argv[3] || 1));
fs.writeFileSync(out, Buffer.from(NetIO.serialize(net)));
NetIO.parse(new Uint8Array(fs.readFileSync(out)).buffer); // round-trip check
console.log(`wrote ${out}`);
