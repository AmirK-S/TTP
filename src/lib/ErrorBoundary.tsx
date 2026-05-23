// TTP - Talk To Paste
// Lightweight in-house ErrorBoundary so the pill window doesn't have to ship
// the full @sentry/react SDK (~80KB) just to wrap a render tree. Sentry-aware
// windows (Settings, Onboarding) can lazy-import the SDK inside componentDidCatch
// when they need to escalate; the pill simply renders the fallback and moves on.

import React from 'react';

type Props = {
  children: React.ReactNode;
  fallback: React.ReactNode;
  /** Optional escalation hook — used by Settings/Onboarding to lazy-import Sentry. */
  onError?: (error: Error, info: React.ErrorInfo) => void;
};

type State = { hasError: boolean };

export class ErrorBoundary extends React.Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // eslint-disable-next-line no-console
    console.error('[ErrorBoundary]', error, info);
    this.props.onError?.(error, info);
  }

  render() {
    return this.state.hasError ? this.props.fallback : this.props.children;
  }
}

export default ErrorBoundary;
