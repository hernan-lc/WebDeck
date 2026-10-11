<script lang="ts">
  import { useTranslations } from '../../lib/i18n';
  import { capabilityNames } from '../../lib/capability-labels';
  import type { Capability } from '../../lib/contracts';
  const t = useTranslations();
  let {
    selected = $bindable<Capability[]>(['read']),
    recommended = ['read'] as Capability[],
    full = [
      'read',
      'input',
      'audio',
      'window',
      'power',
      'script',
      'network',
      'plugin',
    ] as Capability[],
    profile = $bindable('custom'),
  } = $props();
  function choose(value: string) {
    profile = value;
    if (value === 'view') selected = ['read'];
    if (value === 'deck') selected = [...recommended];
    if (value === 'full') selected = [...full];
  }
</script>

<label
  >{t('ui_permission_profile')}<select
    value={profile}
    onchange={(e) => choose(e.currentTarget.value)}
  >
    <option value="view">{t('ui_profile_view')}</option>
    <option value="deck">{t('ui_profile_deck')}</option>
    <option value="full">{t('ui_profile_full')}</option>
    <option value="custom">{t('ui_profile_custom')}</option>
  </select></label
>
{#if profile === 'full'}<p>{t('ui_full_controller_help')}</p>{/if}
{#if profile === 'custom'}<fieldset>
    <legend>{t('ui_permissions')}</legend>
    {#each full as capability}<label class="check"
        ><input type="checkbox" value={capability} bind:group={selected} />{t(
          capabilityNames[capability] ?? capability,
        )}</label
      >{/each}
  </fieldset>{:else}<p>{selected.map((c) => t(capabilityNames[c] ?? c)).join(', ')}</p>{/if}
