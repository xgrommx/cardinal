import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import ThemeSwitcher from './ThemeSwitcher';
import LanguageSwitcher from './LanguageSwitcher';

type PreferencesOverlayProps = {
  open: boolean;
  onClose: () => void;
};

export function PreferencesOverlay({ open, onClose }: PreferencesOverlayProps): React.JSX.Element | null {
  const { t } = useTranslation();

  useEffect(() => {
    if (!open) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  const handleOverlayClick = (event: React.MouseEvent<HTMLDivElement>): void => {
    if (event.target === event.currentTarget) {
      onClose();
    }
  };

  return (
    <div className="preferences-overlay" role="dialog" aria-modal="true" onClick={handleOverlayClick}>
      <div className="preferences-card">
        <header className="preferences-card__header">
          <div>
            <p className="preferences-card__eyebrow">{t('preferences.title')}</p>
            <h1 className="preferences-card__title">{t('preferences.heading')}</h1>
            <p className="preferences-card__subtitle">{t('preferences.subtitle')}</p>
          </div>
          <button
            type="button"
            className="preferences-close"
            aria-label={t('preferences.close')}
            onClick={onClose}
          >
            ×
          </button>
        </header>

        <div className="preferences-section">
          <div className="preferences-row">
            <div>
              <p className="preferences-label">{t('preferences.appearance')}</p>
              <p className="preferences-hint">{t('preferences.themeHint')}</p>
            </div>
            <ThemeSwitcher className="preferences-control" />
          </div>
          <div className="preferences-row">
            <div>
              <p className="preferences-label">{t('preferences.language')}</p>
              <p className="preferences-hint">{t('preferences.languageHint')}</p>
            </div>
            <LanguageSwitcher className="preferences-control" />
          </div>
        </div>

        <footer className="preferences-footer">
          <button type="button" onClick={onClose}>
            {t('preferences.close')}
          </button>
        </footer>
      </div>
    </div>
  );
}

export default PreferencesOverlay;
