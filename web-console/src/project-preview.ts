// Keep previews on the download's media origin and signed static route.
// Frames MUST remain sandboxed without allow-same-origin.
export function projectPreview(download: URL, candidate?: string | null): URL | null {
  if (!candidate) return null;
  try {
    const url = new URL(candidate, download.origin);
    if (!['http:', 'https:'].includes(url.protocol) || url.origin !== download.origin
        || url.username || url.password || url.search || url.hash
        || !/^\/projects\/artifact-[a-f0-9]{64}\/[a-f0-9]{64}\/.+\.html?$/.test(url.pathname)
        || /%(?:2e|2f|5c)/i.test(url.pathname)) return null;
    return url;
  } catch { return null; }
}
