const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const report=JSON.parse(fs.readFileSync(path.join(process.argv[2],'comparison.json'),'utf8'));
const template=fs.readFileSync(path.join(__dirname,'treemap.fragment.html'),'utf8');
const markup=template.replace('__PROFILE_DATA__',()=>JSON.stringify(report).replace(/</g,'\\u003c'));
assert(!markup.includes('__PROFILE_DATA__'));
for(const [,script]of markup.matchAll(/<script>([\s\S]*?)<\/script>/g))new vm.Script(script);
assert(Buffer.byteLength(markup)<1024*1024,'Inline view must stay under 1 MiB');
const output=path.resolve(process.argv[3]);fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,markup);
console.log(output+' ('+Buffer.byteLength(markup)+' bytes)');
