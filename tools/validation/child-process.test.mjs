import assert from 'node:assert/strict';
import { EventEmitter, once } from 'node:events';
import { spawn } from 'node:child_process';
import { test } from 'node:test';
import { stopChild } from './child-process.mjs';

test('already signal-terminated children need no second exit event', async () => {
  await stopChild({ pid: 1, exitCode: null, signalCode: 'SIGINT', kill() { assert.fail('already exited'); } });
});

test('immediate signal exit is observed before kill returns', async () => {
  const child = Object.assign(new EventEmitter(), { pid: 1, exitCode: null, signalCode: null });
  child.kill = signal => { child.signalCode = signal; child.emit('exit', null, signal); return true; };
  await stopChild(child);
  assert.equal(child.listenerCount('exit'), 0);
});

test('unresponsive graceful shutdown escalates and reaps the child', async () => {
  const child = Object.assign(new EventEmitter(), { pid: 1, exitCode: null, signalCode: null });
  const signals = [];
  child.kill = signal => {
    signals.push(signal);
    if (signal === 'SIGKILL') { child.signalCode = signal; child.emit('exit', null, signal); }
    return true;
  };
  await stopChild(child, { graceMs: 5 });
  assert.deepEqual(signals, ['SIGINT', 'SIGKILL']);
});

test('failed termination rejects within a bounded wait', async () => {
  const child = Object.assign(new EventEmitter(), { pid: 1, exitCode: null, signalCode: null, kill: () => false });
  await assert.rejects(stopChild(child, { graceMs: 5, killMs: 5 }), /did not exit/);
  assert.equal(child.listenerCount('exit'), 0);
});

test('a real child is reaped', async () => {
  const child = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { stdio: 'ignore' });
  await once(child, 'spawn');
  await stopChild(child);
  assert(child.exitCode !== null || child.signalCode !== null);
});
