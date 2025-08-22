#!/usr/bin/env node
const fs = require('fs').promises;
const path = require('path');

async function generateIndex(dir) {
  let entries;
  try {
    entries = await fs.readdir(dir, { withFileTypes: true });
  } catch (e) {
    return;
  }
  entries.sort((a,b)=>{
    if(a.isDirectory() && !b.isDirectory()) return -1;
    if(!a.isDirectory() && b.isDirectory()) return 1;
    return a.name.localeCompare(b.name);
  });

  const rel = path.relative(process.cwd(), dir) || '.';
  let html = `<!doctype html>\n<html><head><meta charset="utf-8"><title>Index of ${rel}</title></head><body>\n`;
  html += `<h1>Index of ${rel}</h1>\n<ul>\n`;
  for (const e of entries) {
    const name = e.name;
    const href = encodeURIComponent(name).replace(/%2F/g, '/');
    if (e.isDirectory()) html += `<li><a href="${href}/">${name}/</a></li>\n`;
    else html += `<li><a href="${href}">${name}</a></li>\n`;
  }
  html += `</ul>\n</body></html>`;
  await fs.writeFile(path.join(dir, 'index.html'), html, 'utf8');
}

async function walk(base) {
  let stat;
  try { stat = await fs.stat(base); } catch(e){ return; }
  if (!stat.isDirectory()) return;
  await generateIndex(base);
  const entries = await fs.readdir(base, { withFileTypes: true });
  for (const e of entries) {
    if (e.isDirectory()) await walk(path.join(base, e.name));
  }
}

async function main(){
  const targets = process.argv.slice(2);
  if (targets.length === 0) {
    console.error('Usage: generate_indexes.js <dir> [<dir>...]');
    process.exit(2);
  }
  for (const t of targets) await walk(t);
}

main();
