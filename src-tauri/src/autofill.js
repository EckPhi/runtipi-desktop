function (expectedOrigin, credentialJson, mode) {
  try {
    if (window.location.origin !== expectedOrigin) return { status: 'origin_changed' };
    const credential = JSON.parse(credentialJson);
    const visible = input => !input.disabled && !input.readOnly && input.type !== 'hidden' &&
      input.autocomplete !== 'new-password' && input.getClientRects().length > 0 &&
      getComputedStyle(input).display !== 'none' && getComputedStyle(input).visibility !== 'hidden';
    const passwords = [...document.querySelectorAll('input[type="password"]')].filter(visible);
    if (mode !== 'username' && passwords.length > 1) return { status: 'ambiguous' };
    const password = passwords[0];
    const scope = password?.closest('form') ?? document;
    let usernames = [...scope.querySelectorAll('input[autocomplete="username"]')].filter(visible);
    if (!usernames.length) usernames = [...scope.querySelectorAll('input[type="email"]')].filter(visible);
    if (!usernames.length) usernames = [...scope.querySelectorAll('input[type="text"], input:not([type])')].filter(visible);
    if (mode !== 'password' && usernames.length > 1) return { status: 'ambiguous' };
    const username = usernames[0];
    if ((mode !== 'password' && !username) || (mode !== 'username' && !password)) return { status: 'no_fields' };
    const login = username?.type === 'email' ? credential.email || credential.username : credential.username || credential.email;
    if ((mode !== 'password' && !login) || (mode !== 'username' && !credential.password)) return { status: 'no_fields' };
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    const fill = (input, value) => {
      if (window.location.origin !== expectedOrigin) throw new Error('origin_changed');
      setter.call(input, value);
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    };
    if (mode !== 'password') fill(username, login);
    if (mode !== 'username') fill(password, credential.password);
    // Filling is deliberately separate from submitting the page's form.
    return { status: 'filled' };
  } catch (_) {
    return { status: 'no_fields' };
  }
}
