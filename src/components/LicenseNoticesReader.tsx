import { useEffect, useRef, useState } from 'react';
import { useI18n } from '../i18n/I18nContext';

export function LicenseNoticesReader() {
  const { t } = useI18n();
  const [pages, setPages] = useState<string[] | null>(null);
  const [page, setPage] = useState(0);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const textRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    let cancelled = false;
    setFailed(false);
    // Vite bundles this chunk locally; reading notices never requires internet.
    void import('../services/licenses').then(({ licensePages }) => {
      if (!cancelled) setPages(licensePages);
    }).catch(() => {
      if (!cancelled) setFailed(true);
    });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => { textRef.current?.scrollTo(0, 0); }, [page]);

  if (!pages) return <div className="credits-license-status" role="status">
    <p>{t('credits', failed ? 'loadError' : 'loadingNotices')}</p>
    {failed && <button className="credits-license-back" onClick={() => setAttempt(value => value + 1)}>{t('credits', 'retryNotices')}</button>}
  </div>;

  return <>
    <nav className="credits-license-pagination" aria-label={t('credits', 'licensePages')}>
      <button onClick={() => setPage(0)} disabled={page === 0} aria-label={t('credits', 'firstPage')} title={t('credits', 'firstPage')}>«</button>
      <button onClick={() => setPage(value => value - 1)} disabled={page === 0} aria-label={t('credits', 'previousPage')} title={t('credits', 'previousPage')}>‹</button>
      <span aria-live="polite" aria-atomic="true">{t('credits', 'pageOf').replace('{page}', String(page + 1)).replace('{total}', String(pages.length))}</span>
      <button onClick={() => setPage(value => value + 1)} disabled={page === pages.length - 1} aria-label={t('credits', 'nextPage')} title={t('credits', 'nextPage')}>›</button>
      <button onClick={() => setPage(pages.length - 1)} disabled={page === pages.length - 1} aria-label={t('credits', 'lastPage')} title={t('credits', 'lastPage')}>»</button>
    </nav>
    <pre ref={textRef} className="credits-license-text" tabIndex={0} aria-label={t('credits', 'licenses')}>{pages[page]}</pre>
  </>;
}
