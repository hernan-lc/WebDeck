<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { useTranslations } from '../../lib/i18n';
  import { request } from '../../lib/api/client';
  import { onConnectionChange } from '../../lib/api/realtime';
  import { credentials } from '../../lib/api/http';
  import type {
    ConnectionStatus,
    ConnectionInvitation,
    ConnectionProgress,
  } from '../../lib/contracts';
  import NetworkSettings from '../settings/NetworkSettings.svelte';
  import DeviceSettings from '../settings/DeviceSettings.svelte';
  const t = useTranslations();
  let connected = $state(false);
  onMount(() => onConnectionChange((value) => (connected = value)));
  let {
    attempt,
    transport = $bindable('http'),
    trigger = 0,
  }: {
    trigger?: number;
    transport?: 'http' | 'socket';
    attempt: (work: () => Promise<void>) => Promise<void>;
  } = $props();
  let status = $state<ConnectionStatus | null>(null),
    invitation = $state<ConnectionInvitation | null>(null);
  let progress = $state('waiting'),
    working = $state(false),
    qr = $state(''),
    expired = $state(false),
    error = $state('');
  let destroyed = false,
    generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let refresh: ReturnType<typeof setTimeout> | undefined;
  onDestroy(() => {
    destroyed = true;
    generation++;
    clearTimeout(timer);
    clearTimeout(refresh);
    if (qr) URL.revokeObjectURL(qr);
  });
  async function load() {
    try {
      const value = await request<ConnectionStatus>('connection/status', 'ConnectionStatus');
      if (destroyed) return;
      if (status && invitation && status.network.url !== value.network.url) {
        expired = true;
        if (qr) URL.revokeObjectURL(qr);
        qr = '';
        if (progress !== 'approved' && !working && value.listener_started) {
          await request(
            `connection/invitations/${encodeURIComponent(invitation.id)}`,
            'ConnectionProgress',
            { method: 'DELETE' },
          );
          await start();
        }
      }
      status = value;
      if (invitation && !expired) {
        const current = await request<ConnectionProgress>(
          `connection/invitations/${encodeURIComponent(invitation.id)}`,
          'ConnectionProgress',
        );
        if (!destroyed) {
          progress = current.state;
          if (['approved', 'rejected', 'expired'].includes(progress) && qr) {
            URL.revokeObjectURL(qr);
            qr = '';
          }
        }
      }
      error = '';
    } catch (e) {
      if (!destroyed) error = e instanceof Error ? e.message : String(e);
    }
    if (!destroyed) refresh = setTimeout(() => void load(), 5000);
  }
  async function start() {
    if (working) return;
    working = true;
    const current = ++generation;
    clearTimeout(timer);
    try {
      const value = await request<ConnectionStatus>('connection/start', 'ConnectionStatus', {
        method: 'POST',
      });
      if (destroyed || current !== generation) return;
      status = value;
      const invite = await request<ConnectionInvitation>(
        'connection/invitations',
        'ConnectionInvitation',
        { method: 'POST' },
      );
      const response = await fetch(
        `/api/v2/connection/invitations/${encodeURIComponent(invite.id)}/qr`,
        { headers: credentials(), cache: 'no-store' },
      );
      if (!response.ok) throw new Error(t('ui_phone_qr_failed'));
      const blob = await response.blob();
      if (destroyed || current !== generation) return;
      if (qr) URL.revokeObjectURL(qr);
      qr = URL.createObjectURL(blob);
      invitation = invite;
      progress = 'waiting';
      expired = false;
      timer = setTimeout(
        () => {
          expired = true;
          if (qr) URL.revokeObjectURL(qr);
          qr = '';
        },
        Math.max(0, invite.expires_at * 1000 - Date.now()),
      );
    } finally {
      if (!destroyed) working = false;
    }
  }
  let lastTrigger = 0;
  $effect(() => {
    if (trigger > lastTrigger) {
      lastTrigger = trigger;
      void attempt(start);
    }
  });
  onMount(() => void load());
</script>

<section>
  <h2>{t('ui_devices_connections')}</h2>
  <p>{t('ui_connect_disclosure')}</p>
  <p role="status">
    {status?.listener_started ? t('ui_connection_ready') : t('ui_phone_access_disabled')}
  </p>
  {#if error}<p role="alert">{error}</p>{/if}
  <button class="primary" disabled={working} onclick={() => attempt(start)}
    >{working ? t('ui_connecting') : t('ui_connect_device')}</button
  >
  {#if invitation}
    <div class="invitation" aria-live="polite">
      {#if progress === 'approved'}<p role="status">
          {t('ui_device_connected_success')}
        </p>{:else if progress === 'rejected'}<p>
          {t('ui_pair_rejected')}
        </p>{:else if expired || progress === 'expired'}<p>{t('ui_pair_expired')}</p>{:else}
        <h3>{t('ui_scan_approve')}</h3>
        {#if qr}<img src={qr} alt={t('ui_phone_qr_alt')} width="290" height="290" />{/if}
        <details>
          <summary>{t('ui_connection_link')}</summary><a
            href={invitation.url}
            target="_blank"
            rel="noreferrer">{invitation.url}</a
          ><button onclick={() => attempt(() => navigator.clipboard.writeText(invitation!.url))}
            >{t('ui_copy_link')}</button
          >
        </details>
        <p>{t(progress === 'pending' ? 'ui_waiting_host' : 'ui_pair_compare_code')}</p>
      {/if}
      <button disabled={working} onclick={() => attempt(start)}>{t('ui_refresh_connection')}</button
      >
      <button
        onclick={() =>
          attempt(async () => {
            if (invitation)
              await request(
                `connection/invitations/${encodeURIComponent(invitation.id)}`,
                'ConnectionProgress',
                { method: 'DELETE' },
              );
            invitation = null;
            clearTimeout(timer);
            if (qr) URL.revokeObjectURL(qr);
            qr = '';
          })}>{t('ui_close')}</button
      >
    </div>
  {/if}
  <DeviceSettings {attempt} />
  <details>
    <summary>{t('ui_advanced_connection')}</summary>
    {#if typeof Notification !== 'undefined'}<button
        onclick={() =>
          attempt(async () => {
            await Notification.requestPermission();
          })}>{t('ui_pairing_notifications')}</button
      >{/if}
    <NetworkSettings {attempt} />
    <div id="settings-connection">
      <p aria-label={t('ui_realtime_connection')}>{t(connected ? 'ui_connected' : 'ui_offline')}</p>
      <label
        >{t('ui_connection')}<select bind:value={transport}
          ><option value="http">{t('ui_http')}</option><option value="socket"
            >{t('ui_realtime')}</option
          ></select
        ></label
      >
    </div>
  </details>
  <details>
    <summary>{t('ui_connection_diagnostics')}</summary>
    <p>{t('ui_phone_troubleshooting')}</p>
    <p>{t('ui_origin_recovery')}</p>
    {#if status}<dl>
        <dt>{t('ui_computer_address')}</dt>
        <dd>{status.network.settings.address}</dd>
        <dt>{t('ui_phone_port')}</dt>
        <dd>{status.network.settings.port}</dd>
      </dl>
      <ul>
        {#each status.addresses as address}<li>{address.interface}: {address.address}</li>{/each}
      </ul>
    {/if}
  </details>
</section>

<style>
  .invitation {
    margin: var(--space-4) 0;
    padding: var(--space-4);
    background: var(--canvas);
    border-radius: var(--radius-panel);
  }
  img {
    display: block;
    max-width: 100%;
    height: auto;
  }
  a,
  dd {
    overflow-wrap: anywhere;
  }
  details {
    margin-top: var(--space-4);
  }
  button {
    min-height: 44px;
  }
</style>
