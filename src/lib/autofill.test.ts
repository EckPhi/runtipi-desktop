import autofillScript from '../../src-tauri/src/autofill.js?raw';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

const fill = eval(`(${autofillScript})`) as (origin: string, credentials: string, mode: string) => { status: string };
const credentials = JSON.stringify({ email: 'user@example.com', username: 'user', password: 'test-only' });
beforeEach(() => {
  document.body.innerHTML = '<form><input type="email"><input type="password"><button>Login</button></form>';
  vi.spyOn(HTMLInputElement.prototype, 'getClientRects').mockImplementation(() => ({ length: 1 }) as DOMRectList);
});
afterEach(() => { document.body.innerHTML = ''; vi.restoreAllMocks(); });
it('fills a login form with framework input events without submitting it', () => {
  const changed = vi.fn(); const submitted = vi.fn();
  document.querySelector('form')!.addEventListener('submit', submitted);
  document.querySelector('input')!.addEventListener('input', changed);
  expect(fill(location.origin, credentials, 'both').status).toBe('filled');
  expect(document.querySelector<HTMLInputElement>('input[type="email"]')!.value).toBe('user@example.com');
  expect(document.querySelector<HTMLInputElement>('input[type="password"]')!.value).toBe('test-only');
  expect(changed).toHaveBeenCalledOnce(); expect(submitted).not.toHaveBeenCalled();
});
it('refuses another origin before touching any fields', () => {
  expect(fill('https://evil.test', credentials, 'both').status).toBe('origin_changed');
  expect([...document.querySelectorAll('input')].every(input => input.value === '')).toBe(true);
});
it('rejects ambiguous password fields without partially filling a username', () => {
  document.body.insertAdjacentHTML('beforeend', '<input type="password">');
  expect(fill(location.origin, credentials, 'both').status).toBe('ambiguous');
  expect([...document.querySelectorAll('input')].every(input => input.value === '')).toBe(true);
});
it('does not fill hidden or account-creation password fields', () => {
  document.querySelector<HTMLInputElement>('input[type="password"]')!.autocomplete = 'new-password';
  expect(fill(location.origin, credentials, 'both').status).toBe('no_fields');
  expect(document.querySelector<HTMLInputElement>('input')!.value).toBe('');
});
it('supports separate username and password steps', () => {
  document.body.innerHTML = '<input autocomplete="username">';
  expect(fill(location.origin, credentials, 'username').status).toBe('filled');
  expect(document.querySelector<HTMLInputElement>('input')!.value).toBe('user');
  document.body.innerHTML = '<input type="password">';
  expect(fill(location.origin, credentials, 'password').status).toBe('filled');
  expect(document.querySelector<HTMLInputElement>('input')!.value).toBe('test-only');
});
