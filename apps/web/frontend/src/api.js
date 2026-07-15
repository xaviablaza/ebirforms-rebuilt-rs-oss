let csrfToken = "";

export async function api(path, method, body) {
  const headers = { Accept: "application/json" };
  if (body != null) headers["Content-Type"] = "application/json";
  if (method && method !== "GET") headers["X-CSRF-Token"] = csrfToken;
  const response = await fetch(`/api${path}`, {
    method: method || "GET",
    credentials: "same-origin",
    headers,
    body: body == null ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let value = null;
  if (text) {
    try {
      value = JSON.parse(text);
    } catch {
      if (!response.ok) throw new Error(text);
      throw new Error(`Server returned an invalid response (${response.status})`);
    }
  }
  if (!response.ok) throw new Error(value?.error || `Request failed (${response.status})`);
  if (value?.csrf_token) csrfToken = value.csrf_token;
  return value;
}
