<script lang="ts">
  import { capabilityNames } from '../../lib/capability-labels';
  import { useTranslations } from '../../lib/i18n';
  const t = useTranslations();

  import { onMount, onDestroy } from 'svelte';
  import PermissionProfiles from '../connections/PermissionProfiles.svelte';
  import { request } from '../../lib/api/client';
  import type {
    DevicePresenceList,
    PermissionProfiles as Profiles,
    DeviceUpdate,
  } from '../../lib/contracts';
  import type { Device, DeviceApproval, Capability } from '../../lib/contracts';
  import { devices as listDevices, approve, revoke } from '../../lib/api/client';
  let { attempt }: { attempt: (work: () => Promise<void>) => Promise<void> } = $props();
  let presence = $state<DevicePresenceList | null>(null),
    profiles = $state<Profiles | null>(null),
    selected = $state<Device | null>(null);
  let newName = $state(''),
    newCapabilities = $state<Capability[]>([]),
    newTtl = $state(2592000);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let devices = $state<Device[]>([]),
    grant = $state<DeviceApproval | null>(null),
    deviceName = $state('');
  let grantCapabilities = $state<Capability[]>(['read', 'input']),
    grantTtl = $state(86400);
  let loading = $state(true),
    loadError = $state(''),
    copied = $state(false),
    working = $state(false);
  let destroyed = false;
  let loadGeneration = 0;
  let revoking = $state<Record<string, boolean>>({});
  onDestroy(() => {
    clearTimeout(timer);
    destroyed = true;
    loadGeneration++;
    grant = null;
  });
  async function getDevices() {
    const generation = ++loadGeneration;
    loading = true;
    loadError = '';
    try {
      const [result, state, options] = await Promise.all([
        listDevices(),
        request<DevicePresenceList>('devices/presence', 'DevicePresenceList').catch(() => null),
        request<Profiles>('connection/profiles', 'PermissionProfiles').catch(() => null),
      ]);
      if (!destroyed) {
        presence = state;
        profiles = options;
      }
      if (!destroyed && generation === loadGeneration) devices = result.devices;
    } catch (e) {
      if (!destroyed && generation === loadGeneration)
        loadError = e instanceof Error ? e.message : String(e);
    } finally {
      if (!destroyed && generation === loadGeneration) {
        loading = false;
        clearTimeout(timer);
        timer = setTimeout(() => void getDevices(), 5000);
      }
    }
  }
  function profileName(device: Device) {
    const same = (values: Capability[]) =>
      device.capabilities.length === values.length &&
      values.every((cap) => device.capabilities.includes(cap));
    return same(['read'])
      ? 'ui_profile_view'
      : profiles && same(profiles.full)
        ? 'ui_profile_full'
        : profiles && same(profiles.recommended)
          ? 'ui_profile_deck'
          : 'ui_profile_custom';
  }
  function edit(device: Device) {
    selected = device;
    newName = device.name;
    newCapabilities = [...device.capabilities];
  }
  async function saveDevice() {
    if (!selected || working) return;
    if (
      newCapabilities.some(
        (cap) =>
          ['power', 'script', 'network', 'plugin'].includes(cap) &&
          !selected!.capabilities.includes(cap),
      ) &&
      !window.confirm(t('ui_full_controller_help'))
    )
      return;
    working = true;
    try {
      const update: DeviceUpdate = {
        name: newName,
        capabilities: newCapabilities,
        ttl_seconds: newTtl,
      };
      await request(`devices/${encodeURIComponent(selected.id)}`, 'Device', {
        method: 'PATCH',
        body: JSON.stringify(update),
      });
      selected = null;
      await getDevices();
    } finally {
      working = false;
    }
  }
  async function addDevice() {
    if (working) return;
    working = true;
    try {
      const approved = await approve({
        name: deviceName,
        capabilities: grantCapabilities,
        ttl_seconds: grantTtl,
      });
      if (destroyed) return;
      grant = approved;
      copied = false;
      await getDevices();
    } finally {
      working = false;
    }
  }
  async function copyToken() {
    const candidate = grant;
    if (!candidate) return;
    await navigator.clipboard.writeText(candidate.token);
    if (!destroyed && grant === candidate) {
      copied = true;
      grant = null;
    }
  }
  async function revokeDevice(id: string) {
    if (revoking[id]) return;
    revoking[id] = true;
    try {
      await revoke(id);
      if (!destroyed) await getDevices();
    } finally {
      if (!destroyed) revoking[id] = false;
    }
  }
  onMount(() => {
    void getDevices();
  });
</script>

<section id="settings-devices">
  <h2>{t('ui_paired_devices')}</h2>
  {#if loading}<p role="status">{t('ui_loading_devices')}</p>{:else if loadError}<p role="alert">
      {t(loadError)}
    </p>
    <button onclick={getDevices}>{t('ui_retry')}</button>{:else if !devices.length}<p>
      {t('ui_no_devices_paired_yet')}
    </p>{/if}
  {#if selected}
    <fieldset disabled={working}>
      <legend>{t('ui_manage_device')}</legend>
      <label>{t('ui_device_name')}<input bind:value={newName} maxlength="128" /></label>
      <PermissionProfiles
        bind:selected={newCapabilities}
        recommended={profiles?.recommended ?? ['read']}
      />
      <label
        >{t('ui_trust_duration')}<select bind:value={newTtl}
          ><option value={900}>{t('ui_current_session')}</option><option value={2592000}
            >{t('ui_30_days')}</option
          ><option value={315360000}>{t('ui_until_revoked')}</option></select
        ></label
      >
      <button
        disabled={!newName.trim() || !newCapabilities.length}
        onclick={() => attempt(saveDevice)}>{t('ui_save_changes')}</button
      >
      <button onclick={() => (selected = null)}>{t('ui_cancel')}</button>
    </fieldset>
  {/if}
  {#each devices as device}<div class="row">
      <div>
        <strong>{device.name}</strong> ·
        <span id={`device-state-${device.id}`}
          >{device.revoked
            ? t('ui_revoked')
            : revoking[device.id]
              ? t('ui_revoking_device')
              : presence?.devices.find((p) => p.device.id === device.id)?.online
                ? t('ui_connected')
                : t('ui_offline')}</span
        >
        <p>{t('ui_permission_profile')}: {t(profileName(device))}</p>
        <p class="device-details">
          {t('ui_device_expiry', {
            expiry:
              device.expires_at === 9007199254740991
                ? t('ui_until_revoked')
                : new Date(device.expires_at * 1000).toLocaleString(document.documentElement.lang),
          })} · {device.capabilities
            .map((capability) => t(capabilityNames[capability] ?? capability))
            .join(', ')}
        </p>
        {#if presence?.devices.find((p) => p.device.id === device.id)?.last_seen}<p>
            {t('ui_last_seen')}: {new Date(
              presence!.devices.find((p) => p.device.id === device.id)!.last_seen * 1000,
            ).toLocaleString()}
          </p>{/if}
      </div>
      <button disabled={device.revoked} onclick={() => edit(device)}>{t('ui_manage_device')}</button
      >
      <button
        aria-describedby={`device-state-${device.id}`}
        disabled={device.revoked || revoking[device.id]}
        onclick={() => attempt(() => revokeDevice(device.id))}>{t('ui_revoke')}</button
      >
    </div>{/each}
  <details>
    <summary>{t('ui_use_pairing_token')}</summary>
    <label>{t('ui_device_name')}<input bind:value={deviceName} maxlength="128" /></label>
    <PermissionProfiles
      bind:selected={grantCapabilities}
      recommended={profiles?.recommended ?? ['read']}
    />
    <p id="device-approval-help">{t('ui_device_approval_help')}</p>
    <button
      aria-describedby="device-approval-help"
      disabled={working || !deviceName.trim() || !grantCapabilities.length}
      onclick={() => attempt(addDevice)}>{t('ui_approve_device')}</button
    >
    {#if grant}<p>{t('ui_copy_this_token_now_it_will_not_be_shown_again')}</p>
      <code class="token">{grant.token}</code><button onclick={() => attempt(copyToken)}
        >{t('ui_copy_token')}</button
      ><button onclick={() => (grant = null)}>{t('ui_hide_token')}</button>{/if}
    {#if copied}<p role="status">{t('ui_token_copied_keep_it_somewhere_safe')}</p>{/if}
  </details>
</section>

<style>
  .token {
    display: block;
    overflow-wrap: anywhere;
    background: var(--canvas);
    padding: 12px;
    font-size: 0.85rem;
  }
</style>
