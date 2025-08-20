// Format bytes into KB with one decimal place
export function formatKB(bytes) {
  if (bytes == null) return null;
  const kb = bytes / 1024;
  if (!isFinite(kb)) return null;
  return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
}
