// Spike B3/B5 fixture：三层进程链（npm node → server node → grandchild node）。
// 各进程把 PID 追加写入 pids.txt，供测试断言「停止后 0 残留」。
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const pidsFile = path.join(__dirname, 'pids.txt');
fs.appendFileSync(pidsFile, `server:${process.pid}\n`);
if (process.ppid) {
  fs.appendFileSync(pidsFile, `npm-cli:${process.ppid}\n`);
}

const child = spawn(process.execPath, [path.join(__dirname, 'child.js')], {
  stdio: 'ignore',
});
if (typeof child.pid === 'number') {
  fs.appendFileSync(pidsFile, `grandchild:${child.pid}\n`);
}

process.stdout.write('SQUIRL_DEV_READY\n');
setInterval(() => {}, 1000);
