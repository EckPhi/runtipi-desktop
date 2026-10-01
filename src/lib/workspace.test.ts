import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Page from '../routes/+page.svelte';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const home = { id: '11111111-1111-4111-8111-111111111111', name: 'Home', url: 'http://home.local/' };
const lab = { id: '22222222-2222-4222-8222-222222222222', name: 'Lab', url: 'http://lab.local/' };

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    // A tab strip can change the header's layout after rendering.
    const bottom = this.tagName === 'HEADER' ? (this.querySelector('.tab') ? 181.5 : 158) : 0;
    return { bottom, top: 0, left: 0, right: 1200, width: 1200, height: bottom, x: 0, y: 0, toJSON() {} };
  });
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  invoke.mockReset();
  invoke.mockImplementation(async command => command === 'load_settings' ? { instances: [home, lab], defaultInstance: home.id } : undefined);
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

async function boot() {
  render(Page);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('create_tab', expect.objectContaining({ instanceId: home.id })));
  await waitFor(() => expect((screen.getByRole('button', { name: 'Settings' }) as HTMLButtonElement).disabled).toBe(false));
}

describe('desktop workspace', () => {
  it('positions the browser below the rendered tab strip and full navigation header', async () => {
    await boot();
    const creation = invoke.mock.calls.find(([command]) => command === 'create_tab');
    expect(creation?.[1].top).toBe(159);
    const resizes = invoke.mock.calls.filter(([command, args]) => command === 'control_tab' && args.action === 'resize');
    expect(resizes.at(-1)?.[1].top).toBe(183);
  });

  it('restores existing tabs when switching instances instead of creating duplicate dashboards', async () => {
    await boot();
    const selector = screen.getByRole('combobox');
    await fireEvent.change(selector, { target: { value: lab.id } });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('create_tab', expect.objectContaining({ instanceId: lab.id })));
    await waitFor(() => expect((selector as HTMLSelectElement).disabled).toBe(false));
    await fireEvent.change(selector, { target: { value: home.id } });
    await waitFor(() => expect((selector as HTMLSelectElement).disabled).toBe(false));
    expect(invoke.mock.calls.filter(([command]) => command === 'create_tab')).toHaveLength(2);
    expect(invoke).toHaveBeenCalledWith('control_tab', expect.objectContaining({ action: 'hide' }));
  });

  it('saves named instances and hides remote views while settings are open', async () => {
    await boot();
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await screen.findByRole('heading', { name: 'Add an instance' });
    await fireEvent.input(screen.getByLabelText('Name'), { target: { value: 'Office' } });
    await fireEvent.input(screen.getByLabelText('Dashboard URL'), { target: { value: 'https://office.example.com' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Add instance' }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: { defaultInstance: home.id, instances: [home, lab, expect.objectContaining({ name: 'Office', url: 'https://office.example.com/' })] },
    }));
    expect(invoke).toHaveBeenCalledWith('control_tab', expect.objectContaining({ action: 'hide' }));
    await screen.findByText('Office', { selector: 'strong' });
  });

  it('selects a Proton login by metadata and restores the page after filling', async () => {
    await boot();
    invoke.mockImplementation(async command => command === 'list_logins' ? { origin: 'http://home.local', items: [{ id: 'login', share_id: 'vault', title: 'Home login' }] } : undefined);
    await fireEvent.click(screen.getByRole('button', { name: 'Proton Pass' }));
    const login = await screen.findByRole('button', { name: 'Home login' });
    expect(invoke).toHaveBeenCalledWith('control_tab', expect.objectContaining({ action: 'hide' }));
    await fireEvent.click(login);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('fill_login', expect.objectContaining({ shareId: 'vault', itemId: 'login', origin: 'http://home.local', mode: 'both' })));
    await screen.findByRole('status');
    expect(invoke).toHaveBeenCalledWith('control_tab', expect.objectContaining({ action: 'focus' }));
    expect(screen.queryByRole('heading', { name: 'Choose a login' })).toBeNull();
  });

  it('keeps configuration intact and shows an error when persistence fails', async () => {
    await boot();
    invoke.mockImplementation(async command => { if (command === 'save_settings') throw new Error('Disk is read-only'); });
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await screen.findByRole('heading', { name: 'Add an instance' });
    await fireEvent.input(screen.getByLabelText('Name'), { target: { value: 'Office' } });
    await fireEvent.input(screen.getByLabelText('Dashboard URL'), { target: { value: 'https://office.example.com' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Add instance' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Disk is read-only'));
    expect(screen.queryByText('Office', { selector: 'strong' })).toBeNull();
  });
});
