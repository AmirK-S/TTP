import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mockInvoke } from '../test/setup';

// Stub the i18n + theme side-effects: settings-store calls setLanguage() and
// applyTheme() inside loadSettings/saveSettings. Both touch the DOM and the
// i18next runtime which we don't want to wire up for a state-store test.
vi.mock('../i18n/config', () => ({
  setLanguage: vi.fn(),
}));
vi.mock('../lib/theme', () => ({
  applyTheme: vi.fn(),
}));

// Import AFTER the mocks so the store picks them up.
import { useSettingsStore } from './settings-store';

const defaultRustSettings = {
  ai_polish_enabled: true,
  shortcut: 'FnKey',
  fn_key_enabled: true,
  telemetry_enabled: false,
  hands_free_mode: false,
  hide_pill_when_inactive: false,
  autostart_enabled: false,
  history_enabled: true,
  use_beta_channel: false,
  vad_auto_stop_enabled: false,
  vad_silence_secs: 3,
  audio_device_name: null,
  language: 'en',
  theme: 'dark',
};

describe('settings-store', () => {
  beforeEach(() => {
    // Reset the Zustand store between tests so leftover state doesn't bleed.
    useSettingsStore.setState({
      aiPolishEnabled: true,
      shortcut: 'Alt+Space',
      fnKeyEnabled: false,
      telemetryEnabled: false,
      handsFreeMode: false,
      hidePillWhenInactive: false,
      autostartEnabled: false,
      historyEnabled: true,
      useBetaChannel: false,
      language: 'system',
      theme: 'system',
      dictionary: [],
      history: [],
      loading: false,
      usage: null,
    });
  });

  it('loadSettings populates the store from the Rust IPC payload', async () => {
    mockInvoke.mockResolvedValueOnce(defaultRustSettings);

    await useSettingsStore.getState().loadSettings();

    const s = useSettingsStore.getState();
    expect(s.aiPolishEnabled).toBe(true);
    expect(s.shortcut).toBe('FnKey');
    expect(s.fnKeyEnabled).toBe(true);
    expect(s.language).toBe('en');
    expect(s.theme).toBe('dark');
    expect(s.loading).toBe(false);
    expect(mockInvoke).toHaveBeenCalledWith('get_settings', undefined);
  });

  it('loadSettings falls back to defaults when Rust returns null fields', async () => {
    mockInvoke.mockResolvedValueOnce({
      ai_polish_enabled: false,
      shortcut: '',
      // Every other field omitted to exercise nullish-coalescing defaults.
    });

    await useSettingsStore.getState().loadSettings();

    const s = useSettingsStore.getState();
    expect(s.aiPolishEnabled).toBe(false);
    expect(s.shortcut).toBe('Alt+Space'); // defaults when empty
    expect(s.fnKeyEnabled).toBe(false);
    expect(s.telemetryEnabled).toBe(false);
    expect(s.historyEnabled).toBe(true);
    expect(s.language).toBe('system');
    expect(s.theme).toBe('system');
  });

  it('loadSettings clears the loading flag even on IPC error', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('boom'));

    await useSettingsStore.getState().loadSettings();

    expect(useSettingsStore.getState().loading).toBe(false);
  });

  it('loadSettings reads the VAD opt-in fields from the IPC payload', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...defaultRustSettings,
      vad_auto_stop_enabled: true,
      vad_silence_secs: 5,
    });

    await useSettingsStore.getState().loadSettings();

    const s = useSettingsStore.getState();
    expect(s.vadAutoStopEnabled).toBe(true);
    expect(s.vadSilenceSecs).toBe(5);
  });

  it('saveSettings includes VAD fields in the merged payload sent to Rust', async () => {
    useSettingsStore.setState({ vadAutoStopEnabled: true, vadSilenceSecs: 7 });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSettingsStore.getState().saveSettings({ vad_silence_secs: 5 });

    expect(mockInvoke).toHaveBeenCalledWith('set_settings', {
      settings: expect.objectContaining({
        vad_auto_stop_enabled: true,
        vad_silence_secs: 5,
      }),
    });
    expect(useSettingsStore.getState().vadSilenceSecs).toBe(5);
  });

  it('loadSettings carries an audio device preference from Rust', async () => {
    mockInvoke.mockResolvedValueOnce({
      ...defaultRustSettings,
      audio_device_name: 'AirPods Pro',
    });
    await useSettingsStore.getState().loadSettings();
    expect(useSettingsStore.getState().audioDeviceName).toBe('AirPods Pro');
  });

  it('saveSettings forwards audioDeviceName in the merged payload', async () => {
    useSettingsStore.setState({ audioDeviceName: 'MacBook Pro Microphone' });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSettingsStore.getState().saveSettings({ audio_device_name: 'Blue Yeti' });

    expect(mockInvoke).toHaveBeenCalledWith('set_settings', {
      settings: expect.objectContaining({ audio_device_name: 'Blue Yeti' }),
    });
    expect(useSettingsStore.getState().audioDeviceName).toBe('Blue Yeti');
  });

  it('saveSettings merges partial updates over current state and forwards full payload', async () => {
    // Prime current state with non-defaults.
    useSettingsStore.setState({
      aiPolishEnabled: false,
      shortcut: 'Ctrl+Space',
      telemetryEnabled: true,
      language: 'fr',
      theme: 'dark',
    });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSettingsStore.getState().saveSettings({ ai_polish_enabled: true });

    // The IPC payload must be the FULL Settings shape with the merge applied,
    // not just `{ ai_polish_enabled: true }` — set_settings on the Rust side
    // expects every field.
    expect(mockInvoke).toHaveBeenCalledWith('set_settings', {
      settings: expect.objectContaining({
        ai_polish_enabled: true, // overridden
        shortcut: 'Ctrl+Space',
        telemetry_enabled: true,
        language: 'fr',
        theme: 'dark',
      }),
    });

    // Store reflects the new value.
    expect(useSettingsStore.getState().aiPolishEnabled).toBe(true);
  });

  it('resetSettings reverts to documented defaults', async () => {
    useSettingsStore.setState({
      aiPolishEnabled: false,
      shortcut: 'Ctrl+Space',
      telemetryEnabled: true,
      language: 'fr',
      theme: 'dark',
    });
    mockInvoke.mockResolvedValueOnce(undefined);

    await useSettingsStore.getState().resetSettings();

    const s = useSettingsStore.getState();
    expect(s.aiPolishEnabled).toBe(true);
    expect(s.shortcut).toBe('Alt+Space');
    expect(s.telemetryEnabled).toBe(false);
    expect(s.language).toBe('system');
    expect(s.theme).toBe('system');
  });

  it('saveSettings propagates errors so callers can show "Couldn\'t save"', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('disk full'));

    await expect(
      useSettingsStore.getState().saveSettings({ ai_polish_enabled: false }),
    ).rejects.toThrow('disk full');
  });
});
