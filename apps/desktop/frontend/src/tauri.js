export async function invoke(command, args) {
  const payload = args || {};

  if (globalThis.__TAURI__?.core?.invoke) {
    return await globalThis.__TAURI__.core.invoke(command, payload);
  }

  if (globalThis.__TAURI_INTERNALS__?.invoke) {
    return await globalThis.__TAURI_INTERNALS__.invoke(command, payload);
  }

  throw new Error(`Tauri command bridge is unavailable outside the desktop app: ${command}`);
}
