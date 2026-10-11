import { afterEach, beforeEach, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({
  request: vi.fn(),
  connect: vi.fn(),
  disconnect: vi.fn(),
  credential: vi.fn(() => 'saved-device'),
  connection: undefined as undefined | ((online: boolean) => void),
  device: undefined as undefined | ((event: string) => void),
  remove: vi.fn(),
}));
vi.mock('./api/http', () => ({
  request: mocks.request,
  credential: mocks.credential,
  ApiError: class extends Error {
    constructor(
      public status: number,
      message: string,
      public code?: string,
    ) {
      super(message);
    }
  },
}));
vi.mock('./api/realtime', () => ({
  connect: mocks.connect,
  disconnect: mocks.disconnect,
  onConnectionChange: (listener: (online: boolean) => void) => {
    mocks.connection = listener;
    return mocks.remove;
  },
  onDeviceEvent: (listener: (event: string) => void) => {
    mocks.device = listener;
    return mocks.remove;
  },
}));
import { ConnectionController } from './connection.svelte';
import { ApiError } from './api/http';
let controller: ConnectionController;
beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mocks.credential.mockReturnValue('saved-device');
  mocks.request.mockResolvedValue({
    id: 'phone',
    capabilities: ['read'],
    revoked: false,
    expires_at: 9007199254740991,
  });
  controller = new ConnectionController();
});
afterEach(() => {
  controller.dispose();
  vi.useRealTimers();
});
it('restores a remembered credential and waits for authenticated realtime', async () => {
  controller.start();
  await vi.advanceTimersByTimeAsync(0);
  expect(mocks.request).toHaveBeenCalledWith('device/self', 'Device');
  expect(controller.state).toBe('reconnecting');
  mocks.connection?.(true);
  expect(controller.state).toBe('connected');
  expect(controller.device?.id).toBe('phone');
});
it('temporary outages use bounded retries of authorization reads only', async () => {
  mocks.request.mockRejectedValue(new TypeError('offline'));
  controller.start();
  await vi.advanceTimersByTimeAsync(65000);
  expect(controller.state).toBe('reconnecting');
  expect(mocks.request.mock.calls.length).toBeGreaterThan(2);
  expect(mocks.request.mock.calls.every(([path]) => path === 'device/self')).toBe(true);
  expect(mocks.connect).not.toHaveBeenCalled();
});
it('expired access stops retries and remains distinct from offline', async () => {
  mocks.request.mockRejectedValue(new ApiError(401, 'expired'));
  controller.start();
  await vi.advanceTimersByTimeAsync(60000);
  expect(controller.state).toBe('authorization_expired');
  expect(mocks.request).toHaveBeenCalledTimes(1);
  expect(mocks.disconnect).toHaveBeenCalled();
});
it('revocation disconnects and cancels retries', async () => {
  controller.start();
  await vi.advanceTimersByTimeAsync(0);
  mocks.device?.('device.revoked');
  expect(controller.state).toBe('revoked');
  const count = mocks.request.mock.calls.length;
  await vi.advanceTimersByTimeAsync(60000);
  expect(mocks.request).toHaveBeenCalledTimes(count);
});
it('disposal removes subscriptions and ignores late reads', async () => {
  let complete!: (value: unknown) => void;
  mocks.request.mockImplementation(() => new Promise((resolve) => (complete = resolve)));
  controller.start();
  controller.dispose();
  complete({ id: 'late' });
  await vi.advanceTimersByTimeAsync(0);
  expect(controller.device).toBeNull();
  expect(mocks.connect).not.toHaveBeenCalled();
  expect(mocks.remove).toHaveBeenCalledTimes(2);
});
it('missing credentials use the local boot boundary rather than inventing identity', async () => {
  mocks.credential.mockReturnValue('');
  mocks.request.mockRejectedValue(new ApiError(401, 'approval required'));
  controller.start();
  await vi.advanceTimersByTimeAsync(0);
  expect(controller.state).toBe('unpaired');
  expect(mocks.request).toHaveBeenCalledWith('boot', 'DeckBoot');
});

it('revoked credentials remain distinct from expired credentials after reload', async () => {
  mocks.request.mockRejectedValue(new ApiError(401, 'revoked', 'device_revoked'));
  controller.start();
  await vi.advanceTimersByTimeAsync(0);
  expect(controller.state).toBe('revoked');
  await vi.advanceTimersByTimeAsync(60000);
  expect(mocks.request).toHaveBeenCalledTimes(1);
});
