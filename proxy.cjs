// ohMyWorkPanel proxy — adds Authorization header for WebSocket connections
const http = require('http');
const httpProxy = require('http');

const BACKEND = 'http://127.0.0.1:8080';
const PORT = 9090;

const server = http.createServer((req, res) => {
  const u = new URL(req.url, `http://${req.headers.host || 'x'}`);
  const options = {
    hostname: '127.0.0.1',
    port: 8080,
    path: req.url,
    method: req.method,
    headers: { ...req.headers },
  };

  // Forward Authorization header from client
  if (req.headers.authorization) {
    options.headers['authorization'] = req.headers.authorization;
  }

  const pr = http.request(options, (pres) => {
    res.writeHead(pres.statusCode, pres.headers);
    pres.pipe(res);
  });
  pr.on('error', e => { res.writeHead(502); res.end(e.message); });
  req.pipe(pr);
});

// WebSocket upgrade handling
server.on('upgrade', (req, socket, head) => {
  const u = new URL(req.url, `http://${req.headers.host || 'x'}`);
  const token = u.searchParams.get('token') || '';
  
  // Rewrite path without token param
  u.searchParams.delete('token');
  const path = u.pathname + u.search;

  const opts = {
    host: '127.0.0.1',
    port: 8080,
    path: path,
    method: 'GET',
    headers: {
      'Host': req.headers.host || '127.0.0.1:8080',
      'Upgrade': 'websocket',
      'Connection': 'Upgrade',
      'Sec-WebSocket-Key': req.headers['sec-websocket-key'],
      'Sec-WebSocket-Version': req.headers['sec-websocket-version'],
      'Sec-WebSocket-Extensions': req.headers['sec-websocket-extensions'] || '',
      'Sec-WebSocket-Protocol': req.headers['sec-websocket-protocol'] || '',
      'Authorization': 'Bearer ' + token,
    },
  };

  const preq = http.request(opts);
  preq.on('upgrade', (pres, psocket, phead) => {
    socket.write(
      'HTTP/1.1 101 Switching Protocols\r\n' +
      'Upgrade: websocket\r\n' +
      'Connection: Upgrade\r\n' +
      (pres.headers['sec-websocket-protocol'] ? 'Sec-WebSocket-Protocol: ' + pres.headers['sec-websocket-protocol'] + '\r\n' : '') +
      'Sec-WebSocket-Accept: ' + pres.headers['sec-websocket-accept'] + '\r\n' +
      '\r\n'
    );
    psocket.pipe(socket);
    socket.pipe(psocket);
  });
  preq.on('error', e => socket.destroy());
  preq.end();
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`ohMyWorkPanel proxy on :${PORT} -> ${BACKEND}`);
});
