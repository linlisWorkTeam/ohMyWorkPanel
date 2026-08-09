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

// Responses API tools -> Chat Completions tools.
// Upstream (OpenCode Zen Go / DeepSeek) only accepts type=function.
// Drop host tools like web_search / web_search_call that Codex CLI injects —
// otherwise upstream 400: unknown variant `web_search`, expected `function`.
function convertTools(tools) {
  if (!Array.isArray(tools) || tools.length === 0) return undefined;
  const out = [];
  let dropped = 0;
  for (const t of tools) {
    if (!t || typeof t !== 'object') {
      dropped++;
      continue;
    }
    // Already Chat Completions shape
    if (t.type === 'function' && t.function && t.function.name) {
      out.push({
        type: 'function',
        function: {
          name: t.function.name,
          description: t.function.description || '',
          parameters: t.function.parameters || { type: 'object', properties: {} }
        }
      });
      continue;
    }
    // Responses API shape: { type: 'function', name, description, parameters }
    if (t.type === 'function' && t.name) {
      out.push({
        type: 'function',
        function: {
          name: t.name,
          description: t.description || '',
          parameters: t.parameters || { type: 'object', properties: {} }
        }
      });
      continue;
    }
    dropped++;
  }
  if (dropped > 0) {
    console.log(`  → convertTools: kept ${out.length} function tools, dropped ${dropped} non-function (e.g. web_search)`);
  }
  return out.length > 0 ? out : undefined;
}

// Responses API tool_choice -> Chat Completions tool_choice
function convertToolChoice(tc) {
  if (tc == null) return undefined;
  if (typeof tc === 'string') return tc; // auto | none | required
  if (typeof tc === 'object' && tc.type === 'function' && tc.name) {
    return { type: 'function', function: { name: tc.name } };
  }
  return tc;
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
      if (item.type === 'function_call') {
        // Assistant tool call -> chat tool_calls
        const call = {
          id: item.call_id || `call_${Date.now()}_${messages.length}`,
          type: 'function',
          function: {
            name: item.name || '',
            arguments: typeof item.arguments === 'string' ? item.arguments : JSON.stringify(item.arguments || {})
          }
        };
        const last = messages[messages.length - 1];
        if (last && last.role === 'assistant' && Array.isArray(last.tool_calls)) {
          last.tool_calls.push(call); // merge with preceding assistant text
        } else {
          messages.push({ role: 'assistant', content: null, tool_calls: [call] });
        }
      }
      if (item.type === 'function_call_output') {
        // Tool result -> chat tool message
        messages.push({
          role: 'tool',
          tool_call_id: item.call_id || '',
          content: typeof item.output === 'string' ? item.output : JSON.stringify(item.output ?? '')
        });
      }
      // reasoning items are intentionally skipped
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
      const chatTools = convertTools(reqBody.tools);
      const chatToolChoice = convertToolChoice(reqBody.tool_choice);
      if (chatTools) chatReq.tools = chatTools;
      if (chatToolChoice) chatReq.tool_choice = chatToolChoice;

      console.log(`  → upstream: model=${chatReq.model}, stream=${chatReq.stream}, msgs=${chatMessages.length}, roles=${chatMessages.map(m=>m.role).join(',')}, tools=${chatTools ? chatTools.length : 0}`);

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
          let toolCalls = []; // {index, id, name, arguments}
          let toolItemsEmitted = new Set();
          let finishReason = null;

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
                  const delta = parsed.choices?.[0]?.delta || {};
                  if (parsed.choices?.[0]?.finish_reason) {
                    finishReason = parsed.choices[0].finish_reason;
                  }
                  if (delta.content) {
                    fullContent += delta.content;
                    sendSSE(res, {
                      type: 'response.output_text.delta',
                      item_id: `msg_${Date.now()}`,
                      output_index: 0, content_index: 0, delta: delta.content
                    });
                  }
                  if (Array.isArray(delta.tool_calls)) {
                    for (const tc of delta.tool_calls) {
                      const idx = tc.index || 0;
                      if (!toolCalls[idx]) {
                        toolCalls[idx] = { index: idx, id: tc.id || `call_${Date.now()}_${idx}`, name: '', arguments: '' };
                      }
                      if (tc.id) toolCalls[idx].id = tc.id;
                      if (tc.function?.name) toolCalls[idx].name += tc.function.name;
                      if (tc.function?.arguments) toolCalls[idx].arguments += tc.function.arguments;

                      if (!toolItemsEmitted.has(idx)) {
                        toolItemsEmitted.add(idx);
                        sendSSE(res, {
                          type: 'response.output_item.added',
                          output_index: toolCalls.length,
                          item: { id: toolCalls[idx].id, type: 'function_call', call_id: toolCalls[idx].id, name: toolCalls[idx].name, arguments: '' }
                        });
                      }
                      sendSSE(res, {
                        type: 'response.function_call_arguments.delta',
                        item_id: toolCalls[idx].id,
                        output_index: toolCalls.length,
                        delta: tc.function?.arguments || ''
                      });
                    }
                  }
                } catch(e) {}
              }
            }
          });

          proxyRes.on('end', () => {
            const output = [];
            if (fullContent) {
              output.push({ id: `msg_${Date.now()}`, type: 'message', role: 'assistant',
                content: [{ type: 'output_text', text: fullContent }] });
            }
            for (const tc of toolCalls) {
              const item = { id: tc.id, type: 'function_call', call_id: tc.id, name: tc.name, arguments: tc.arguments, status: 'completed' };
              output.push(item);
              sendSSE(res, { type: 'response.output_item.done', output_index: output.length - 1, item });
            }
            const stopReason = toolCalls.length > 0 ? 'tool_use' : (finishReason === 'tool_calls' ? 'tool_use' : 'end_of_turn');
            if (fullContent && toolCalls.length === 0) {
              sendSSE(res, {
                type: 'response.output_item.done',
                item: { id: `msg_${Date.now()}`, type: 'message', role: 'assistant',
                  content: [{ type: 'output_text', text: fullContent }] }
              });
            }
            sendSSE(res, {
              type: 'response.completed',
              response: { id: responseId, object: 'response', model, created,
                status: 'completed', output,
                stop_reason: stopReason, usage: { input_tokens: 0, output_tokens: 0, total_tokens: 0 } }
            });
            console.log(`  ✓ done, ${fullContent.length} chars, toolCalls=${toolCalls.length}, stop=${stopReason}`);
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
              const msg = chatResp.choices?.[0]?.message || {};
              const outputText = msg.content || '';
              const chatToolCalls = Array.isArray(msg.tool_calls) ? msg.tool_calls : [];
              const output = [];
              if (outputText) {
                output.push({ type: 'message', id: `msg_${Date.now()}`, role: 'assistant',
                  content: [{ type: 'output_text', text: outputText }] });
              }
              for (const tc of chatToolCalls) {
                output.push({ type: 'function_call', id: tc.id || `call_${Date.now()}`, call_id: tc.id || `call_${Date.now()}`,
                  name: tc.function?.name || '', arguments: tc.function?.arguments || '{}', status: 'completed' });
              }
              const stopReason = chatToolCalls.length > 0 ? 'tool_use' : 'end_of_turn';
              const resp = {
                id: chatResp.id || `resp_${Date.now()}`,
                object: 'response', model: chatResp.model || model,
                created: chatResp.created || Math.floor(Date.now() / 1000),
                output,
                stop_reason: stopReason,
                usage: chatResp.usage ? {
                  input_tokens: chatResp.usage.prompt_tokens || 0,
                  output_tokens: chatResp.usage.completion_tokens || 0,
                  total_tokens: chatResp.usage.total_tokens || 0
                } : null
              };
              res.writeHead(200, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify(resp));
              console.log(`  ✓ done (non-stream), ${outputText.length} chars, toolCalls=${chatToolCalls.length}, stop=${stopReason}`);
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
