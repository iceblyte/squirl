// 孙进程：常驻直到被整组回收。
const fs = require('fs');
const path = require('path');
fs.appendFileSync(path.join(__dirname, 'pids.txt'), `grandchild-self:${process.pid}\n`);
setInterval(() => {}, 1000);
