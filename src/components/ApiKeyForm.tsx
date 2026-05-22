import { useState, type FormEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Key, ExternalLink } from 'lucide-react';
import { Button, Input } from './ui';

interface Props {
  onSuccess: () => void;
  /** Override the submit button label (defaults to common.getStarted). */
  submitLabel?: string;
  /** Tightens vertical rhythm when embedded inside a wizard step. */
  compact?: boolean;
}

/**
 * Shared API-key input + validation form. Used in both the standalone
 * Setup window and inside the onboarding wizard step. Single source of
 * truth — previously this logic was duplicated inline in Onboarding.
 */
export function ApiKeyForm({ onSuccess, submitLabel, compact = false }: Props) {
  const { t } = useTranslation();
  const [groqKey, setGroqKey] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError('');

    if (!groqKey.trim()) {
      setError(t('form.apiKey.errorRequired'));
      return;
    }
    if (!groqKey.startsWith('gsk_')) {
      setError(t('form.apiKey.errorFormat'));
      return;
    }

    setLoading(true);
    try {
      await invoke('validate_groq_api_key', { key: groqKey });
      await invoke('set_groq_api_key', { key: groqKey });
      onSuccess();
    } catch (err) {
      // Rust may surface a translation key like "error.api_invalid_key";
      // translate those defensively, otherwise show the raw string.
      setError(
        typeof err === 'string' && (err.startsWith('error.') || err.startsWith('permission.'))
          ? t(err)
          : String(err),
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className={compact ? 'space-y-3' : 'space-y-4'}>
      <div>
        <label
          htmlFor="groq-key"
          className="block text-xs font-medium text-app-muted mb-1.5"
        >
          {t('form.apiKey.label')}
        </label>
        <Input
          id="groq-key"
          type="password"
          value={groqKey}
          onChange={(e) => setGroqKey(e.target.value)}
          placeholder={t('form.apiKey.placeholder')}
          autoComplete="off"
          autoFocus
          invalid={Boolean(error)}
          leftIcon={<Key className="size-4" aria-hidden />}
        />
      </div>

      <ol className="text-xs text-app-muted list-decimal pl-5 space-y-1.5">
        <li>
          {t('form.apiKey.stepSignup')}{' '}
          <a
            href="https://console.groq.com"
            target="_blank"
            rel="noopener noreferrer"
            className="text-app-accent hover:underline inline-flex items-center gap-1"
          >
            console.groq.com
            <ExternalLink className="size-3" aria-hidden />
          </a>{' '}
          {t('form.apiKey.stepSignupSuffix')}
        </li>
        <li>{t('form.apiKey.stepCreate')}</li>
        <li>
          {t('form.apiKey.stepCopy')}{' '}
          <code className="px-1 py-0.5 rounded bg-app-surface-hover font-mono text-[11px] text-app-text">
            gsk_
          </code>{' '}
          {t('form.apiKey.stepCopySuffix')}
        </li>
      </ol>

      {!compact && (
        <p className="text-xs text-app-faint">{t('form.apiKey.helpFreeTier')}</p>
      )}

      {error && (
        <p className="text-sm text-app-danger" role="alert">
          {error}
        </p>
      )}

      <Button type="submit" disabled={!groqKey} loading={loading} fullWidth size="lg">
        {submitLabel ?? (loading ? t('common.validating') : t('common.getStarted'))}
      </Button>
    </form>
  );
}
