// TTP - Talk To Paste
// Settings state store using Zustand

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { safeInvoke } from '../lib/safeInvoke';
import { emit } from '@tauri-apps/api/event';
import { setLanguage, type LanguageChoice } from '../i18n/config';

/** Dictionary entry structure matching Rust backend */
export interface DictionaryEntry {
  original: string;
  correction: string;
  created_at: number;
}

/** History entry structure matching Rust backend */
export interface HistoryEntry {
  text: string;
  timestamp: number;
  raw_text?: string;
}

/** Settings structure matching Rust backend */
export interface Settings {
  ai_polish_enabled: boolean;
  shortcut: string;
  fn_key_enabled: boolean;
  telemetry_enabled: boolean;
  hands_free_mode: boolean;
  hide_pill_when_inactive: boolean;
  history_enabled: boolean;
  use_beta_channel: boolean;
  /** 'en' | 'fr' | 'system' | null. null is treated as 'system' (autodetect). */
  language: string | null;
}

/** License info returned by Rust backend */
interface LicenseInfo {
  is_pro: boolean;
  license_key: string | null;
  status: string | null;
  expires_at: number | null;
  last_validated_at: number | null;
  activation_count: number | null;
  activation_limit: number | null;
}

/** Usage stats returned by Rust backend */
interface UsageStats {
  is_pro: boolean;
  is_in_trial: boolean;
  trial_days_left: number | null;
  trial_started_at: number | null;
  polish_count_this_month: number;
  polish_limit_free: number;
  dictionary_count: number;
  dictionary_limit_free: number;
  history_count: number;
  history_limit_free: number;
}

interface SettingsStore {
  // State
  aiPolishEnabled: boolean;
  shortcut: string;
  fnKeyEnabled: boolean;
  telemetryEnabled: boolean;
  handsFreeMode: boolean;
  hidePillWhenInactive: boolean;
  historyEnabled: boolean;
  useBetaChannel: boolean;
  language: LanguageChoice;
  dictionary: DictionaryEntry[];
  history: HistoryEntry[];
  loading: boolean;

  // License state
  isPro: boolean;
  licenseKey: string | null;
  licenseStatus: string | null;
  licenseExpiresAt: number | null;
  licenseLastValidatedAt: number | null;
  licenseActivationCount: number | null;
  licenseActivationLimit: number | null;
  licenseLoading: boolean;
  licenseError: string | null;

  // Usage state
  usage: UsageStats | null;

  // Actions
  loadSettings: () => Promise<void>;
  saveSettings: (settings: Partial<Settings>) => Promise<void>;
  resetSettings: () => Promise<void>;
  loadDictionary: () => Promise<void>;
  deleteEntry: (original: string) => Promise<void>;
  clearDictionary: () => Promise<void>;
  loadHistory: () => Promise<void>;
  clearHistory: () => Promise<void>;
  loadLicense: () => Promise<void>;
  activateLicense: (key: string) => Promise<void>;
  deactivateLicense: () => Promise<void>;
  validateLicense: () => Promise<void>;
  loadUsage: () => Promise<void>;
}

function applyLicenseInfo(info: LicenseInfo) {
  return {
    isPro: info.is_pro,
    licenseKey: info.license_key,
    licenseStatus: info.status,
    licenseExpiresAt: info.expires_at,
    licenseLastValidatedAt: info.last_validated_at,
    licenseActivationCount: info.activation_count,
    licenseActivationLimit: info.activation_limit,
  };
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  // Initial state
  aiPolishEnabled: true,
  shortcut: 'Alt+Space',
  fnKeyEnabled: false,
  telemetryEnabled: false,
  handsFreeMode: false,
  hidePillWhenInactive: false,
  historyEnabled: true,
  useBetaChannel: false,
  language: 'system',
  dictionary: [],
  history: [],
  loading: false,

  // License initial state
  isPro: false,
  licenseKey: null,
  licenseStatus: null,
  licenseExpiresAt: null,
  licenseLastValidatedAt: null,
  licenseActivationCount: null,
  licenseActivationLimit: null,
  licenseLoading: false,
  licenseError: null,

  usage: null,

  // Load settings from backend
  loadSettings: async () => {
    set({ loading: true });
    try {
      const settings = await safeInvoke<Settings>('get_settings');
      const lang = (settings.language ?? 'system') as LanguageChoice;
      set({
        aiPolishEnabled: settings.ai_polish_enabled,
        shortcut: settings.shortcut || 'Alt+Space',
        fnKeyEnabled: settings.fn_key_enabled ?? false,
        telemetryEnabled: settings.telemetry_enabled ?? false,
        handsFreeMode: settings.hands_free_mode ?? false,
        hidePillWhenInactive: settings.hide_pill_when_inactive ?? false,
        historyEnabled: settings.history_enabled ?? true,
        useBetaChannel: settings.use_beta_channel ?? false,
        language: lang,
      });
      setLanguage(lang);
    } catch (error) {
      console.error('Failed to load settings:', error);
    } finally {
      set({ loading: false });
    }
  },

  // Save settings to backend
  saveSettings: async (updates: Partial<Settings>) => {
    try {
      const currentSettings: Settings = {
        ai_polish_enabled: get().aiPolishEnabled,
        shortcut: get().shortcut,
        fn_key_enabled: get().fnKeyEnabled,
        telemetry_enabled: get().telemetryEnabled,
        hands_free_mode: get().handsFreeMode,
        hide_pill_when_inactive: get().hidePillWhenInactive,
        history_enabled: get().historyEnabled,
        use_beta_channel: get().useBetaChannel,
        language: get().language,
      };

      const newSettings: Settings = {
        ...currentSettings,
        ...updates,
      };

      await invoke('set_settings', { settings: newSettings });
      // Emit event so other components (like pill, other windows) can react to settings changes
      emit('settings-changed', newSettings);
      const newLang = (newSettings.language ?? 'system') as LanguageChoice;
      set({
        aiPolishEnabled: newSettings.ai_polish_enabled,
        shortcut: newSettings.shortcut,
        fnKeyEnabled: newSettings.fn_key_enabled,
        telemetryEnabled: newSettings.telemetry_enabled,
        handsFreeMode: newSettings.hands_free_mode,
        hidePillWhenInactive: newSettings.hide_pill_when_inactive,
        historyEnabled: newSettings.history_enabled,
        useBetaChannel: newSettings.use_beta_channel,
        language: newLang,
      });
      setLanguage(newLang);
    } catch (error) {
      console.error('Failed to save settings:', error);
      throw error;
    }
  },

  // Reset settings to defaults
  resetSettings: async () => {
    try {
      await invoke('reset_settings');
      set({
        aiPolishEnabled: true,
        shortcut: 'Alt+Space',
        fnKeyEnabled: false,
        telemetryEnabled: false,
        handsFreeMode: false,
        hidePillWhenInactive: false,
        historyEnabled: true,
        useBetaChannel: false,
        language: 'system',
      }); // Default values
      setLanguage('system');
    } catch (error) {
      console.error('Failed to reset settings:', error);
      throw error;
    }
  },

  // Load dictionary entries
  loadDictionary: async () => {
    try {
      const entries = await safeInvoke<DictionaryEntry[]>('get_dictionary');
      set({ dictionary: entries });
    } catch (error) {
      console.error('Failed to load dictionary:', error);
      // Dictionary might not be implemented yet
      set({ dictionary: [] });
    }
  },

  // Delete a single dictionary entry
  deleteEntry: async (original: string) => {
    try {
      await invoke('delete_dictionary_entry', { original });
      set((state) => ({
        dictionary: state.dictionary.filter((e) => e.original !== original),
      }));
    } catch (error) {
      console.error('Failed to delete entry:', error);
      throw error;
    }
  },

  // Clear all dictionary entries
  clearDictionary: async () => {
    try {
      await invoke('clear_dictionary');
      set({ dictionary: [] });
    } catch (error) {
      console.error('Failed to clear dictionary:', error);
      throw error;
    }
  },

  // Load history entries
  loadHistory: async () => {
    try {
      const entries = await safeInvoke<HistoryEntry[]>('get_history');
      set({ history: entries });
    } catch (error) {
      console.error('Failed to load history:', error);
      set({ history: [] });
    }
  },

  // Clear all history entries
  clearHistory: async () => {
    try {
      await invoke('clear_history');
      set({ history: [] });
    } catch (error) {
      console.error('Failed to clear history:', error);
      throw error;
    }
  },

  // Load current license info from backend
  loadLicense: async () => {
    try {
      const info = await safeInvoke<LicenseInfo>('get_license_info');
      set({ ...applyLicenseInfo(info), licenseError: null });
    } catch (error) {
      console.error('Failed to load license:', error);
    }
  },

  // Activate a new license key
  activateLicense: async (key: string) => {
    set({ licenseLoading: true, licenseError: null });
    try {
      const info = await invoke<LicenseInfo>('activate_license', { licenseKey: key });
      set({ ...applyLicenseInfo(info), licenseError: null });
    } catch (error) {
      const message = typeof error === 'string' ? error : 'error.license_activation_failed';
      set({ licenseError: message });
      throw error;
    } finally {
      set({ licenseLoading: false });
    }
  },

  // Deactivate the current license (frees an activation slot on LS)
  deactivateLicense: async () => {
    set({ licenseLoading: true, licenseError: null });
    try {
      await invoke('deactivate_license');
      set({
        isPro: false,
        licenseKey: null,
        licenseStatus: null,
        licenseExpiresAt: null,
        licenseLastValidatedAt: null,
        licenseActivationCount: null,
        licenseActivationLimit: null,
      });
    } catch (error) {
      const message = typeof error === 'string' ? error : 'error.license_deactivation_failed';
      set({ licenseError: message });
      throw error;
    } finally {
      set({ licenseLoading: false });
    }
  },

  // Force a re-validation against the LS server
  validateLicense: async () => {
    set({ licenseLoading: true, licenseError: null });
    try {
      const info = await invoke<LicenseInfo>('validate_license');
      set({ ...applyLicenseInfo(info), licenseError: null });
    } catch (error) {
      const message = typeof error === 'string' ? error : 'error.license_validation_failed';
      set({ licenseError: message });
    } finally {
      set({ licenseLoading: false });
    }
  },

  // Load usage counters and trial state
  loadUsage: async () => {
    try {
      const stats = await safeInvoke<UsageStats>('get_usage_stats');
      set({ usage: stats });
    } catch (error) {
      console.error('Failed to load usage:', error);
    }
  },
}));
