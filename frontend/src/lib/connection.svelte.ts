import { ApiError, credential, request } from './api/http';
import { connect, disconnect, onConnectionChange, onDeviceEvent } from './api/realtime';
import type { Device } from './contracts';
export type ConnectionState =
  | 'initializing'
  | 'unpaired'
  | 'pairing'
  | 'connected'
  | 'reconnecting'
  | 'offline'
  | 'permission_required'
  | 'authorization_expired'
  | 'revoked';
/** One controller per application. Recovery only retries authorization reads, never actions. */
export class ConnectionController {
  state = $state<ConnectionState>('initializing');
  device = $state<Device | null>(null);
  private timer: ReturnType<typeof setTimeout> | undefined;
  private attempt = 0;
  private online = false;
  private disposed = false;
  private started = false;
  private generation = 0;
  private subscriptions: Array<() => void> = [];
  start() {
    if (this.started || this.disposed) return;
    this.started = true;
    this.subscriptions.push(
      onConnectionChange((online) => {
        this.online = online;
        if (online) {
          this.attempt = 0;
          this.state = 'connected';
          void this.refresh();
        } else if (
          !['revoked', 'authorization_expired', 'unpaired', 'pairing'].includes(this.state)
        ) {
          this.state = navigator.onLine ? 'reconnecting' : 'offline';
          this.schedule();
        }
      }),
    );
    this.subscriptions.push(
      onDeviceEvent((event) => {
        if (event === 'device.revoked' || event === 'device.authorization_expired') {
          this.generation++;
          this.state = event === 'device.revoked' ? 'revoked' : 'authorization_expired';
          clearTimeout(this.timer);
          disconnect();
        }
        if (event === 'device.permissions_changed') void this.refresh();
      }),
    );
    window.addEventListener('online', this.wake);
    window.addEventListener('offline', this.offline);
    document.addEventListener('visibilitychange', this.visible);
    void this.refresh();
  }
  private offline = () => {
    if (this.state !== 'revoked' && this.state !== 'authorization_expired') this.state = 'offline';
  };
  private visible = () => {
    if (document.visibilityState === 'visible') this.wake();
  };
  private wake = () => {
    if (!['revoked', 'authorization_expired', 'unpaired', 'pairing'].includes(this.state))
      void this.refresh();
  };
  private schedule() {
    clearTimeout(this.timer);
    if (this.disposed) return;
    const delay =
      Math.min(30000, 1000 * 2 ** Math.min(this.attempt++, 5)) * (0.8 + Math.random() * 0.4);
    this.timer = setTimeout(() => void this.refresh(), delay);
  }
  async refresh() {
    if (this.disposed || this.state === 'revoked') return;
    const generation = ++this.generation;
    try {
      if (credential()) {
        const device = await request<Device>('device/self', 'Device');
        if (this.disposed || generation !== this.generation) return;
        this.device = device;
      } else {
        await request('boot', 'DeckBoot');
        if (this.disposed || generation !== this.generation) return;
      }
      connect();
      // Socket connection events establish connected state.
      this.state = this.online ? 'connected' : 'reconnecting';
      clearTimeout(this.timer);
      this.timer = setTimeout(() => void this.refresh(), 30000);
    } catch (error) {
      if (this.disposed || generation !== this.generation) return;
      if (error instanceof ApiError && error.status === 401) {
        this.state =
          error.code === 'device_revoked'
            ? 'revoked'
            : credential()
              ? 'authorization_expired'
              : 'unpaired';
        clearTimeout(this.timer);
        disconnect();
      } else {
        this.state = navigator.onLine ? 'reconnecting' : 'offline';
        this.schedule();
      }
    }
  }
  retry() {
    this.state = 'initializing';
    void this.refresh();
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.generation++;
    clearTimeout(this.timer);
    this.subscriptions.forEach((remove) => remove());
    window.removeEventListener('online', this.wake);
    window.removeEventListener('offline', this.offline);
    document.removeEventListener('visibilitychange', this.visible);
  }
}
