<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  type Instance = { id: string; name: string; url: string };
  type Settings = { instances: Instance[]; defaultInstance: string | null };
  type Tab = { id: string; instanceId: string; name: string; url: string; dashboard: boolean };
  let settings = $state<Settings>({ instances: [], defaultInstance: null });
  let selected = $state('');
  let tabs = $state<Tab[]>([]);
  let activeByInstance = $state<Record<string, string>>({});
  let showSettings = $state(false);
  let newTab = $state(false);
  let ready = $state(false);
  let busy = $state(false);
  let error = $state('');
  let editing = $state('');
  let name = $state('');
  let url = $state('');
  let tabUrl = $state('');
  let header: HTMLElement;
  let headerHeight = $state(150);
  let sequence = Promise.resolve();
  const visibleTabs = $derived(tabs.filter(t => t.instanceId === selected));
  const active = $derived(tabs.find(t => t.id === activeByInstance[selected]));
  const instance = $derived(settings.instances.find(i => i.id === selected));
  const bounds = () => header?.getBoundingClientRect().bottom ?? 142;

  function enqueue(work: () => Promise<void>) {
    sequence = sequence.then(async () => {
      busy = true;
      error = '';
      try { await work(); } catch (e) { error = String(e); }
      finally { busy = false; }
    });
    return sequence;
  }
  async function control(id: string, action: string) {
    await invoke('control_tab', { tabId: id, action, top: bounds() });
  }
  async function hideAll() {
    for (const tab of tabs) await control(tab.id, 'hide');
  }
  async function activate(id: string) {
    await hideAll();
    const tab = tabs.find(t => t.id === id);
    if (!tab) return;
    selected = tab.instanceId;
    activeByInstance[selected] = id;
    showSettings = false;
    newTab = false;
    await control(id, 'resize');
    await control(id, 'show');
  }
  async function open(url: string, instanceId = selected, dashboard = false) {
    const id = crypto.randomUUID();
    await hideAll();
    await invoke('create_tab', { instanceId, tabId: id, url, top: bounds() });
    tabs.push({ id, instanceId, name: dashboard ? 'Dashboard' : new URL(url).hostname, url, dashboard });
    await activate(id);
  }
  async function switchInstance(id: string) {
    selected = id;
    const previous = activeByInstance[id];
    if (previous) await activate(previous);
    else {
      const item = settings.instances.find(i => i.id === id);
      if (item) await open(item.url, id, true);
    }
  }
  async function closeTab(tab: Tab) {
    await control(tab.id, 'close');
    tabs = tabs.filter(t => t.id !== tab.id);
    if (activeByInstance[tab.instanceId] === tab.id) {
      const next = tabs.find(t => t.instanceId === tab.instanceId);
      if (next) await activate(next.id);
      else delete activeByInstance[tab.instanceId];
    }
  }
  async function persist(next: Settings) {
    await invoke('save_settings', { settings: next });
    settings = next;
  }
  function edit(item?: Instance) {
    editing = item?.id ?? '';
    name = item?.name ?? '';
    url = item?.url ?? '';
  }
  async function saveInstance() {
    const normalized = new URL(url.trim());
    if (!['http:', 'https:'].includes(normalized.protocol)) throw new Error('Use an HTTP or HTTPS URL.');
    const id = editing || crypto.randomUUID();
    const item = { id, name: name.trim(), url: normalized.toString() };
    const instances = editing ? settings.instances.map(i => i.id === id ? item : i) : [...settings.instances, item];
    const previousUrl = settings.instances.find(i => i.id === id)?.url;
    await persist({ instances, defaultInstance: settings.defaultInstance ?? id });
    // Recreate tabs after an endpoint change rather than leaving the old dashboard open.
    if (editing && previousUrl !== item.url) {
      for (const tab of tabs.filter(t => t.instanceId === id)) await control(tab.id, 'close');
      tabs = tabs.filter(t => t.instanceId !== id);
      delete activeByInstance[id];
    }
    edit();
    if (!selected) selected = id;
  }
  async function removeInstance(item: Instance) {
    const instances = settings.instances.filter(i => i.id !== item.id);
    await persist({ instances, defaultInstance: settings.defaultInstance === item.id ? instances[0]?.id ?? null : settings.defaultInstance });
    for (const tab of tabs.filter(t => t.instanceId === item.id)) await control(tab.id, 'close');
    tabs = tabs.filter(t => t.instanceId !== item.id);
    delete activeByInstance[item.id];
    if (selected === item.id) selected = instances[0]?.id ?? '';
    if (editing === item.id) edit();
  }
  async function move(index: number, offset: number) {
    const instances = [...settings.instances];
    [instances[index], instances[index + offset]] = [instances[index + offset], instances[index]];
    await persist({ ...settings, instances });
  }
  onMount(() => {
    const unlisten: UnlistenFn[] = [];
    let disposed = false;
    const resize = () => {
      headerHeight = bounds();
      sequence = sequence.then(async () => {
        if (active && !showSettings && !newTab) await control(active.id, 'resize');
      }).catch(e => { error = String(e); });
    };
    const observer = new ResizeObserver(resize);
    observer.observe(header);
    window.addEventListener('resize', resize);
    enqueue(async () => {
      const listeners = await Promise.all([
        listen<{ label: string; title: string }>('tab-title', event => {
          const tab = tabs.find(t => `tab-${t.id}` === event.payload.label);
          if (tab && !tab.dashboard && event.payload.title) tab.name = event.payload.title;
        }),
        listen<{ label: string; url: string }>('tab-url', event => {
          const tab = tabs.find(t => `tab-${t.id}` === event.payload.label);
          if (tab) tab.url = event.payload.url;
        }),
        listen<{ label: string; url: string }>('tab-popup', event => {
          const tab = tabs.find(t => `tab-${t.id}` === event.payload.label);
          if (tab) void enqueue(() => open(event.payload.url, tab.instanceId));
        }),
      ]);
      if (disposed) { listeners.forEach(fn => fn()); return; }
      unlisten.push(...listeners);
      settings = await invoke<Settings>('load_settings');
      selected = settings.defaultInstance ?? settings.instances[0]?.id ?? '';
      ready = true;
      if (selected) await switchInstance(selected);
      else showSettings = true;
    });
    return () => { disposed = true; unlisten.forEach(fn => fn()); observer.disconnect(); window.removeEventListener('resize', resize); };
  });
</script>

<svelte:head><title>Runtipi Desktop</title></svelte:head>

<header bind:this={header}>
  <div class="topbar">
    <button class="brand" disabled={busy || !selected} onclick={() => selected && enqueue(() => switchInstance(selected))}>◈ <span>Runtipi Desktop</span></button>
    <label class="selector">Instance
      <select value={selected} disabled={!ready || busy || !settings.instances.length} onchange={e => { const id = e.currentTarget.value; void enqueue(() => switchInstance(id)); }}>
        {#if !settings.instances.length}<option value="">Add an instance</option>{/if}
        {#each settings.instances as item}<option value={item.id}>{item.name}</option>{/each}
      </select>
    </label>
    <button class:chosen={showSettings} disabled={!ready || busy} onclick={() => enqueue(async () => { await hideAll(); showSettings = true; newTab = false; })}>Settings</button>
  </div>
  <nav aria-label="Browser tabs" class="tabs">
    {#each visibleTabs as tab}
      <div class:active={active?.id === tab.id && !showSettings && !newTab} class="tab">
        <button class="tab-title" disabled={busy} onclick={() => enqueue(() => activate(tab.id))} title={tab.name}>{tab.name}</button>
        {#if !tab.dashboard}<button class="close" disabled={busy} aria-label={`Close ${tab.name}`} onclick={() => enqueue(() => closeTab(tab))}>×</button>{/if}
      </div>
    {/each}
    <button class="add-tab" disabled={!selected || busy} aria-label="New tab" onclick={() => enqueue(async () => { await hideAll(); showSettings = false; newTab = true; tabUrl = ''; })}>+</button>
  </nav>
  <div class="navigation">
    <button aria-label="Back" disabled={!active || showSettings || newTab || busy} onclick={() => active && enqueue(() => control(active.id, 'back'))}>←</button>
    <button aria-label="Forward" disabled={!active || showSettings || newTab || busy} onclick={() => active && enqueue(() => control(active.id, 'forward'))}>→</button>
    <button disabled={!active || showSettings || newTab || busy} onclick={() => active && enqueue(() => control(active.id, 'reload'))}>Reload</button>
    <span class="address" title={active?.url}>{showSettings ? 'Workspace settings' : newTab ? 'New application tab' : active?.url ?? 'Welcome'}</span>
    <button disabled={!active || showSettings || newTab || busy} onclick={() => active && enqueue(() => control(active.id, 'external'))}>Open externally ↗</button>
  </div>
  {#if error}<div class="error" role="alert">{error}<button onclick={() => error = ''} aria-label="Dismiss error">×</button></div>{/if}
</header>

<main style:height={`calc(100vh - ${headerHeight}px)`}>
  {#if showSettings}
    <section class="settings">
      <div class="intro"><span class="eyebrow">YOUR WORKSPACE</span><h1>Every server. One place.</h1><p>Add your Runtipi dashboards and keep their apps in separate workspaces.</p></div>
      <div class="settings-grid">
        <div class="card"><h2>Instances <span class="count">{settings.instances.length}</span></h2>
          {#if !settings.instances.length}<p class="muted">Start with your first server. Enter its dashboard address in the form.</p>{/if}
          {#each settings.instances as item, index}
            <div class="instance-row"><div><strong>{item.name}</strong><p>{item.url}</p>{#if settings.defaultInstance === item.id}<span class="badge">Default</span>{/if}</div>
              <div class="row-actions">
                <button aria-label={`Move ${item.name} up`} disabled={index === 0 || busy} onclick={() => enqueue(() => move(index, -1))}>↑</button>
                <button aria-label={`Move ${item.name} down`} disabled={index === settings.instances.length - 1 || busy} onclick={() => enqueue(() => move(index, 1))}>↓</button>
                <button disabled={busy} onclick={() => edit(item)}>Edit</button>
                <button disabled={busy || settings.defaultInstance === item.id} onclick={() => enqueue(() => persist({ ...settings, defaultInstance: item.id }))}>Set default</button>
                <button class="danger" disabled={busy} onclick={() => enqueue(() => removeInstance(item))}>Remove</button>
              </div>
            </div>
          {/each}
        </div>
        <form class="card" onsubmit={e => { e.preventDefault(); void enqueue(saveInstance); }}>
          <h2>{editing ? 'Edit instance' : 'Add an instance'}</h2>
          <label>Name<input bind:value={name} placeholder="Home server" required maxlength="100" /></label>
          <label>Dashboard URL<input bind:value={url} type="url" placeholder="http://192.168.1.20:80" required /></label>
          <p class="muted">Use the address you normally open in your browser. Sign in directly through Runtipi; your session is saved for this instance.</p>
          <div class="form-actions"><button class="primary" disabled={busy}>{editing ? 'Save changes' : 'Add instance'}</button>{#if editing}<button type="button" onclick={() => edit()}>Cancel</button>{/if}</div>
        </form>
      </div>
      {#if selected}<button class="primary" disabled={busy} onclick={() => enqueue(() => switchInstance(selected))}>Open {instance?.name ?? 'dashboard'} →</button>{/if}
      <p class="muted footnote">Removing an instance closes its tabs. Its saved browser profile remains on this device.</p>
    </section>
  {:else if newTab}
    <section class="new-tab card"><span class="eyebrow">{instance?.name}</span><h1>Open an application</h1><p>Paste an app URL, or open apps from the dashboard.</p>
      <form onsubmit={e => { e.preventDefault(); void enqueue(() => open(tabUrl.trim())); }}><label>Application URL<input type="url" required bind:value={tabUrl} placeholder="https://cloud.example.com" /></label><button class="primary" disabled={busy}>Open tab</button></form>
    </section>
  {:else if !ready}<section class="empty"><h1>Opening your workspace…</h1></section>
  {:else if !active}<section class="empty"><h1>Your Runtipi workspace</h1><p>Choose an instance or add one in Settings.</p></section>{/if}
</main>

<style>
  :global(*){box-sizing:border-box} :global(body){margin:0;background:#0e1420;color:#e5ecf6;font-family:Inter,ui-sans-serif,system-ui,sans-serif;font-size:14px} :global(button),:global(input),:global(select){font:inherit} :global(button){cursor:pointer;border:1px solid #303e52;background:#1b2738;color:#dbe5f3;border-radius:7px;padding:8px 12px} :global(button:hover:not(:disabled)){background:#2b3c53} :global(button:disabled){opacity:.45;cursor:default} :global(button:focus-visible),:global(input:focus-visible),:global(select:focus-visible){outline:2px solid #66d6bc;outline-offset:2px} :global(input),:global(select){background:#101a28;color:#e5ecf6;border:1px solid #34465c;border-radius:7px;padding:10px} header{background:#141e2c;border-bottom:1px solid #304055} .topbar{display:flex;gap:24px;align-items:center;padding:14px 20px} .brand{background:none;border:0;padding:0;color:#67dfc2;font-size:22px;font-weight:750;text-decoration:none;white-space:nowrap}.brand span{color:#e5ecf6;font-size:16px;margin-left:8px}.selector{display:flex;align-items:center;gap:10px;color:#9aabc0;margin-left:auto}.selector select{min-width:180px;padding:7px}.chosen{border-color:#67dfc2} .tabs{display:flex;align-items:center;gap:5px;padding:0 20px;overflow-x:auto;min-height:38px}.tab{display:flex;max-width:220px;border:1px solid transparent;border-radius:8px 8px 0 0;background:#1b2738}.tab.active{background:#293c50;border-color:#456677;border-bottom:2px solid #67dfc2}.tab-title{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;border:0;background:none}.close{border:0;background:none;padding:6px}.add-tab{border:0;background:none;font-size:21px;padding:3px 12px}.navigation{display:flex;gap:7px;align-items:center;padding:9px 20px;background:#101925}.navigation button{font-size:12px;padding:6px 9px}.address{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:#9aabc0;padding:0 10px}.error{display:flex;justify-content:space-between;align-items:center;gap:10px;background:#512c34;color:#ffd5d7;padding:8px 20px}.error button{background:none;border:0;padding:2px 8px} main{height:calc(100vh - 150px);overflow:auto}.settings{max-width:1150px;margin:0 auto;padding:45px 35px}.eyebrow{font-size:11px;letter-spacing:2px;font-weight:700;color:#67dfc2}h1{font-size:30px;letter-spacing:-1px;margin:10px 0}p{line-height:1.6;color:#9aabc0}h2{font-size:17px;margin:0 0 22px}.intro{margin-bottom:30px}.settings-grid{display:grid;grid-template-columns:1.3fr 1fr;gap:22px;margin-bottom:24px}.card{background:#172233;border:1px solid #2c3b50;border-radius:14px;padding:25px}.count{font-size:12px;background:#2a3c50;padding:3px 8px;border-radius:12px;margin-left:6px}.instance-row{padding:17px 0;border-top:1px solid #2c3b50}.instance-row p{font-size:12px;margin:4px 0;overflow-wrap:anywhere}.row-actions{display:flex;flex-wrap:wrap;gap:6px;margin-top:12px}.row-actions button{font-size:11px;padding:5px 8px}.badge{display:inline-block;margin-top:5px;color:#67dfc2;background:#1c3b3a;padding:3px 7px;border-radius:5px;font-size:10px}.danger{color:#ffafb8}.card label{display:flex;flex-direction:column;gap:8px;margin:18px 0;font-weight:550}.muted{font-size:12px}.primary{background:#65dabc;color:#092a23;border-color:#65dabc;font-weight:700}.primary:hover:not(:disabled){background:#83e7ce}.form-actions{display:flex;gap:10px}.footnote{margin-top:20px}.new-tab{max-width:540px;margin:70px auto}.empty{text-align:center;margin:100px 25px}@media(max-width:850px){.settings-grid{grid-template-columns:1fr}.topbar{gap:12px}.brand span{display:none}.settings{padding:25px}}
</style>
