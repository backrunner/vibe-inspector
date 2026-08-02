import { readFile } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const runtimeDirectory =
  process.env.VIBE_INSPECTOR_RUNTIME_DIR ||
  path.join(os.homedir(), 'Library', 'Application Support', 'com.vibeinspector.desktop');
const endpointPath = path.join(runtimeDirectory, 'mcp-endpoint.json');
const socketPath = path.join(runtimeDirectory, 'tray.sock');
const endpoint = JSON.parse(await readFile(endpointPath, 'utf8'));
const token = (await readFile(endpoint.tokenPath, 'utf8')).trim();

const healthResponse = await controlRequest(socketPath, { command: 'health' });
assert(healthResponse.ok, 'tray health request failed');
assert(healthResponse.health?.tray === 'online', 'tray is not online');
assert(healthResponse.health?.mcp === 'online', 'MCP is not online');
assert(
  typeof healthResponse.health?.loginItemEnabled === 'boolean',
  'login-item health is missing',
);

const initializeBody = {
  jsonrpc: '2.0',
  id: 1,
  method: 'initialize',
  params: {
    protocolVersion: '2025-03-26',
    capabilities: {},
    clientInfo: { name: 'vibe-inspector-tray-verifier', version: '1.0' },
  },
};
const unauthorized = await fetch(endpoint.url, {
  method: 'POST',
  headers: mcpHeaders(),
  body: JSON.stringify(initializeBody),
});
assert(unauthorized.status === 401, `unauthorized request returned ${unauthorized.status}`);

const initialized = await fetch(endpoint.url, {
  method: 'POST',
  headers: mcpHeaders(token),
  body: JSON.stringify(initializeBody),
});
const sessionId = initialized.headers.get('mcp-session-id');
const initializeResponse = parseMcpResponse(await initialized.text());
assert(initialized.status === 200, `initialize returned ${initialized.status}`);
assert(sessionId, 'initialize did not return an MCP session ID');
assert(
  initializeResponse.result?.serverInfo?.name === 'vibe-inspector',
  'server identity mismatch',
);

const sessionHeaders = mcpHeaders(token, sessionId);
const notification = await fetch(endpoint.url, {
  method: 'POST',
  headers: sessionHeaders,
  body: JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }),
});
assert(notification.ok, `initialized notification returned ${notification.status}`);

const listed = await fetch(endpoint.url, {
  method: 'POST',
  headers: sessionHeaders,
  body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'tools/list', params: {} }),
});
const listResponse = parseMcpResponse(await listed.text());
const toolNames = listResponse.result?.tools?.map((tool) => tool.name) || [];
assert(listed.status === 200, `tools/list returned ${listed.status}`);
assert(toolNames.length === 30, `expected 30 tools, received ${toolNames.length}`);
assert(toolNames.includes('inspector.source.read'), 'read-only source tool is missing');
assert(toolNames.includes('inspector.command.run'), 'Inspector command tool is missing');

if (process.argv.includes('--quit')) {
  await controlRequest(socketPath, { command: 'quit' });
}

console.log(
  JSON.stringify({
    tray: healthResponse.health.tray,
    mcp: healthResponse.health.mcp,
    loginItemEnabled: healthResponse.health.loginItemEnabled,
    unauthorizedStatus: unauthorized.status,
    initializeStatus: initialized.status,
    toolCount: toolNames.length,
  }),
);

function mcpHeaders(bearerToken, sessionId) {
  const headers = {
    Accept: 'application/json, text/event-stream',
    'Content-Type': 'application/json',
    'MCP-Protocol-Version': '2025-03-26',
  };
  if (bearerToken) headers.Authorization = `Bearer ${bearerToken}`;
  if (sessionId) headers['MCP-Session-Id'] = sessionId;
  return headers;
}

function parseMcpResponse(body) {
  const event = body
    .split('\n')
    .filter((line) => line.startsWith('data:'))
    .map((line) => line.slice('data:'.length).trim())
    .find((data) => data.startsWith('{') || data.startsWith('['));
  return JSON.parse(event || body);
}

function controlRequest(socket, request) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(socket);
    let response = '';
    client.setEncoding('utf8');
    client.setTimeout(1000);
    client.on('connect', () => client.end(`${JSON.stringify(request)}\n`));
    client.on('data', (chunk) => {
      response += chunk;
    });
    client.on('end', () => {
      try {
        resolve(JSON.parse(response));
      } catch (error) {
        reject(error);
      }
    });
    client.on('timeout', () => client.destroy(new Error('tray control request timed out')));
    client.on('error', reject);
  });
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
