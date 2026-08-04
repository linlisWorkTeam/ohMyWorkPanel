const http = require('http');
const https = require('https');

const TARGET = process.env.CODEX_UPSTREAM_HOST || 'opencode.ai';
const TARGET_PATH = process.env.CODEX_UPSTREAM_PATH || '/zen/go/v1/chat/completions';

let API_KEY;
try {
  API_KEY = require('/root/.codex/auth.json').OPENAI_API_KEY;
} catch(e) {
  API_KEY = process.env.OPENAI_API_KEY;
}

function extractText(content) {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content.map(c => {
      if (c.type === 'input_text' || c.type === 'output_text') return c.text || '';
      return '';
    }).join('\n');
  }
  return '';
}

function responsesToChat(input) {
  if (!input) return [{ role: 'user', content: 'hello' }];
  if (typeof input === 'string') return [{ role: 'user', content: input }];
  if (Array.isArray(input)) {
    const messages = [];
    for (const item of input) {
      if (item.type === 'message' && item.role && item.content) {
        // Convert developer role to system for upstream compatibility
        let role = item.role;
        if (role === 'developer') role = 'system';
        messages.push({ role, content: extractText(item.content) });
      }
      if (item.type === 'input_item' && item.content) {
        messages.push({ role: 'user', content: extractText(item.content) });
      }
    }
    return messages.length > 0 ? messages : [{ role: 'user', content: 'hello' }];
  }
  if (typeof input === 'object' && input.input) {
    return responsesToChat(input.input);
  }
  return [{ role: 'user', content: 'hello' }];
}

function sendSSE(res, data) {
  res.write(`data: ${JSON.stringify(data)}\n\n`);
}

const server = http.createServer((req, res) => {
  console.log(`[${new Date().toISOString()}] ${req.method} ${req.url}`);
  
  if (req.url === '/status' || req.url === '/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({ status: 'ok' }));
  }
  
  if (req.method !== 'POST') {
    res.writeHead(405);
    return res.end('Method Not Allowed');
  }

  let body = '';
  req.on('data', chunk => body += chunk);
  req.on('end', () => {
    try {
      const reqBody = JSON.parse(body);
      const chatMessages = responsesToChat(reqBody.input || reqBody.messages);
      const model = reqBody.model || 'deepseek-v4-flash';
      const maxTokens = Math.max(reqBody.max_output_tokens || 4096, 4096);
      const isStream = reqBody.stream === true || req.headers.accept === 'text/event-stream';
      
      const chatReq = {
        model: model,
        messages: chatMessages,
        max_tokens: maxTokens,
        stream: isStream
      };

      console.log(`  → upstream: model=${chatReq.model}, stream=${chatReq.stream}, msgs=${chatMessages.length}, roles=${chatMessages.map(m=>m.role).join(',')}`);

      if (!API_KEY) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          error: 'OPENAI_API_KEY missing (set env or /root/.codex/auth.json)',
        }));
      }

      const options = {
        hostname: TARGET,
        port: 443,
        path: TARGET_PATH,
        method: 'POST',
        timeout: Number(process.env.CODEX_UPSTREAM_TIMEOUT_MS || 120000),
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${API_KEY}`,
          'Content-Length': Buffer.byteLength(JSON.stringify(chatReq))
        }
      };

      const proxyReq = https.request(options, (proxyRes) => {
        console.log(`  ← upstream status=${proxyRes.statusCode}`);

        if (proxyRes.statusCode !== 200) {
          let errBody = '';
          proxyRes.on('data', c => errBody += c);
          proxyRes.on('end', () => {
            console.log(`  ← error: ${errBody.substring(0, 200)}`);
            res.writeHead(502, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: `upstream ${proxyRes.statusCode}: ${errBody.substring(0, 300)}` }));
          });
          return;
        }

        if (isStream) {
          const responseId = `resp_${Date.now()}`;
          const created = Math.floor(Date.now() / 1000);
          
          res.writeHead(200, {
            'Content-Type': 'text/event-stream',
            'Cache-Control': 'no-cache',
            'Connection': 'keep-alive',
            'X-Accel-Buffering': 'no'
          });

          sendSSE(res, {
            type: 'response.created',
            response: { id: responseId, object: 'response', model, created, status: 'in_progress', output: [], usage: null }
          });

          let fullContent = '';
          let buffer = '';

          proxyRes.on('data', (chunk) => {
            buffer += chunk.toString();
            const lines = buffer.split('\n');
            buffer = lines.pop() || '';
            
            for (const line of lines) {
              if (line.startsWith('data: ')) {
                const data = line.slice(6).trim();
                if (data === '[DONE]') continue;
                try {
                  const parsed = JSON.parse(data);
                  const delta = parsed.choices?.[0]?.delta?.content || '';
                  if (delta) {
                    fullContent += delta;
                    sendSSE(res, {
                      type: 'response.output_text.delta',
                      item_id: `msg_${Date.now()}`,
                      output_index: 0, content_index: 0, delta
                    });
                  }
                } catch(e) {}
              }
            }
          });

          proxyRes.on('end', () => {
            sendSSE(res, {
              type: 'response.output_item.done',
              item: { id: `msg_${Date.now()}`, type: 'message', role: 'assistant',
                content: [{ type: 'output_text', text: fullContent }] }
            });
            sendSSE(res, {
              type: 'response.completed',
              response: { id: responseId, object: 'response', model, created,
                status: 'completed', output: [{ id: `msg_${Date.now()}`, type: 'message', role: 'assistant',
                  content: [{ type: 'output_text', text: fullContent }] }],
                stop_reason: 'end_of_turn', usage: { input_tokens: 0, output_tokens: 0, total_tokens: 0 } }
            });
            console.log(`  ✓ done, ${fullContent.length} chars`);
            res.end();
          });

          proxyRes.on('error', (e) => {
            console.error('  ✗ stream error:', e.message);
            res.end();
          });

        } else {
          let data = '';
          proxyRes.on('data', chunk => data += chunk);
          proxyRes.on('end', () => {
            try {
              const chatResp = JSON.parse(data);
              const outputText = chatResp.choices?.[0]?.message?.content || '';
              const resp = {
                id: chatResp.id || `resp_${Date.now()}`,
                object: 'response', model: chatResp.model || model,
                created: chatResp.created || Math.floor(Date.now() / 1000),
                output: [{ type: 'message', id: `msg_${Date.now()}`, role: 'assistant',
                  content: [{ type: 'output_text', text: outputText }] }],
                stop_reason: 'end_of_turn',
                usage: chatResp.usage ? {
                  input_tokens: chatResp.usage.prompt_tokens || 0,
                  output_tokens: chatResp.usage.completion_tokens || 0,
                  total_tokens: chatResp.usage.total_tokens || 0
                } : null
              };
              res.writeHead(200, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify(resp));
              console.log(`  ✓ done (non-stream), ${outputText.length} chars`);
            } catch(e) {
              console.error('  ✗ parse error:', e.message);
              res.writeHead(500, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify({ error: e.message }));
            }
          });
        }
      });

      proxyReq.on('timeout', () => {
        console.error('  ✗ upstream timeout');
        proxyReq.destroy(new Error('upstream timeout'));
      });

      proxyReq.on('error', (e) => {
        console.error('  ✗ connection error:', e.message);
        if (!res.headersSent) {
          res.writeHead(502, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: e.message }));
        } else {
          res.end();
        }
      });

      proxyReq.write(JSON.stringify(chatReq));
      proxyReq.end();

    } catch (e) {
      console.error('  ✗ parse error:', e.message);
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: e.message }));
    }
  });
});

process.on('uncaughtException', (err) => {
  console.error(`[${new Date().toISOString()}] uncaughtException:`, err && err.stack ? err.stack : err);
});
process.on('unhandledRejection', (err) => {
  console.error(`[${new Date().toISOString()}] unhandledRejection:`, err && err.stack ? err.stack : err);
});

const PORT = Number(process.env.LINLIS_CODEX_PROXY_PORT || 18888);
server.listen(PORT, '127.0.0.1', () => {
  console.log(`[${new Date().toISOString()}] Codex Responses↔ChatCompletions proxy on 127.0.0.1:${PORT} → ${TARGET}${TARGET_PATH}`);
});
