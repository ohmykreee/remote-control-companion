#!/usr/bin/env node
// Minimal VRChat OSC chatbox receiver for local testing. This is a developer
// tool, not part of the application test suite.
//
// History mode (default):
//   node scripts/dsh-osc-mock.mjs
//
// Live mode: clear the terminal on every message and redraw only the latest
// chatbox content, which mirrors how VRChat replaces the chatbox text.
//   node scripts/dsh-osc-mock.mjs --live
//
// VRChat must be closed (or configured to another port) because it also binds
// UDP 9000.

import dgram from 'node:dgram';
import process from 'node:process';

const args = process.argv.slice(2);

function option(names, fallback) {
  for (let index = 0; index < args.length; index += 1) {
    if (names.includes(args[index]) && args[index + 1]) return args[index + 1];
  }
  return fallback;
}

if (args.includes('--help') || args.includes('-h')) {
  console.log(`Usage: node scripts/dsh-osc-mock.mjs [options]

Options:
  -p, --port <port>   UDP port to listen on (default: 9000)
      --host <host>   bind address (default: 127.0.0.1)
      --live          clear the terminal and redraw only the latest message
      --compact       print one line per message (history mode) or one line
                      per refresh (live mode)
  -h, --help          show this help

The companion sends to 127.0.0.1:9000 by default. Close VRChat first, or
change the port in the companion settings, because VRChat also uses 9000.`);
  process.exit(0);
}

const port = Number.parseInt(option(['-p', '--port'], '9000'), 10);
const host = option(['--host'], '127.0.0.1');
const compact = args.includes('--compact');
const live = args.includes('--live');

if (!Number.isInteger(port) || port < 1 || port > 65535) {
  console.error(`Invalid port: ${option(['-p', '--port'], '')}`);
  process.exit(1);
}

function readString(buffer, offset) {
  const end = buffer.indexOf(0, offset);
  if (end < 0) return null;
  return {
    value: buffer.subarray(offset, end).toString('utf8'),
    next: (end + 4) & ~3,
  };
}

function timestamp() {
  const now = new Date();
  return [now.getHours(), now.getMinutes(), now.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');
}

const socket = dgram.createSocket('udp4');
let sequence = 0;

function clearTerminal() {
  if (process.stdout.isTTY) {
    process.stdout.write('\x1b[2J\x1b[H');
  }
}

function renderLive(text, lines, chars, stamp) {
  clearTerminal();
  if (compact) {
    console.log(
      `[${stamp}] #${String(sequence).padStart(3, '0')} ` +
        `${String(lines.length).padStart(2, '0')}L/${String(chars).padStart(3, '0')}C ` +
        JSON.stringify(text)
    );
    return;
  }

  console.log('DSH OSC mock - live chatbox');
  console.log(`udp://${host}:${port}  |  #${String(sequence).padStart(3, '0')}  |  ${stamp}`);
  console.log('----------------------------------------');
  for (const line of lines) console.log(line);
  console.log('----------------------------------------');
  console.log('Waiting for the next /chatbox/input ...');
}

socket.on('message', (buffer) => {
  const address = readString(buffer, 0);
  if (!address) return;
  const tags = readString(buffer, address.next);
  if (!tags || !tags.value.startsWith(',')) return;

  // /chatbox/input ,sT <text> T
  let text = '';
  if (tags.value[1] === 's') {
    const value = readString(buffer, tags.next);
    if (!value) return;
    text = value.value;
  }

  if (address.value !== '/chatbox/input') {
    if (!live && !compact) {
      console.log(`[${timestamp()}] ${address.value} ${tags.value} (ignored)`);
    }
    return;
  }

  sequence += 1;
  const lines = text.split('\n');
  const chars = [...text].length;
  const stamp = timestamp();

  if (live) {
    renderLive(text, lines, chars, stamp);
    return;
  }

  if (compact) {
    console.log(
      `[${stamp}] #${String(sequence).padStart(3, '0')} ` +
        `${String(lines.length).padStart(2, '0')}L/${String(chars).padStart(3, '0')}C ` +
        JSON.stringify(text)
    );
    return;
  }

  console.log(
    `[${stamp}] #${String(sequence).padStart(3, '0')}  ` +
      `${lines.length} line${lines.length === 1 ? '' : 's'} / ${chars} chars`
  );
  for (const line of lines) console.log(`  ${line}`);
  console.log('');
});

socket.on('error', (error) => {
  if (error.code === 'EADDRINUSE') {
    console.error(
      `UDP ${host}:${port} is already in use. Close VRChat or use --port <other> ` +
        `and change the companion port to match.`
    );
  } else {
    console.error(`OSC mock error: ${error.message}`);
  }
  try {
    socket.close();
  } catch {}
  process.exitCode = 1;
});

socket.bind(port, host, () => {
  if (live) {
    clearTerminal();
    console.log(`DSH OSC mock - live listening on udp://${host}:${port}`);
    console.log(`Companion settings: address 127.0.0.1, port ${port}`);
    console.log('Waiting for /chatbox/input ...');
    return;
  }
  console.log(`OSC mock listening on udp://${host}:${port}`);
  console.log(`Companion settings: address 127.0.0.1, port ${port}`);
  console.log('Press Ctrl+C to stop.\n');
});

process.on('SIGINT', () => {
  socket.close(() => process.exit(0));
});
