// Bound both wrapped text and explicit lines so large notices stay cheap to lay out.
export const LICENSE_PAGE_CHARACTERS = 8192;
export const LICENSE_PAGE_LINES = 128;

export function paginateLicenseNotices(text: string): string[] {
  const pages: string[] = [];
  let start = 0;
  while (start < text.length) {
    let end = Math.min(start + LICENSE_PAGE_CHARACTERS, text.length);
    // Never cut a Unicode surrogate pair or a Windows line ending in half.
    const before = text.charCodeAt(end - 1);
    const after = text.charCodeAt(end);
    if ((before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff)
      || (text[end - 1] === '\r' && text[end] === '\n')) end--;
    let lines = 0;
    for (let i = start; i < end; i++) {
      if (text[i] === '\n' && ++lines === LICENSE_PAGE_LINES) {
        end = i + 1;
        break;
      }
    }
    pages.push(text.slice(start, end));
    start = end;
  }
  return pages.length ? pages : [''];
}
