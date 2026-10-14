// exitCode stays null after signal termination; signalCode records that exit.
const ended = child => child.exitCode !== null || child.signalCode !== null;

export async function stopChild(child, { graceMs = 2000, killMs = 5000 } = {}) {
  if (!child || !child.pid || ended(child)) return;
  let resolveExit;
  const exited = new Promise(resolve => resolveExit = resolve);
  const onExit = () => resolveExit();
  child.once('exit', onExit);
  const waitForExit = async milliseconds => {
    let timer;
    try {
      return await Promise.race([
        exited.then(() => true),
        new Promise(resolve => timer = setTimeout(() => resolve(false), milliseconds)),
      ]);
    } finally { clearTimeout(timer); }
  };
  try {
    // Subscribe before signalling: Windows can terminate immediately.
    child.kill('SIGINT');
    if (await waitForExit(graceMs)) return;
    child.kill('SIGKILL');
    if (!await waitForExit(killMs)) throw Error('Validation host did not exit after SIGKILL');
  } finally { child.removeListener('exit', onExit); }
}
