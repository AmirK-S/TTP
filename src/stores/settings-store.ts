// TTP - Talk To Paste
// Settings state store using Zustand

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

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

/** Transcription provider options */
export type TranscriptionProvider = 'gladia' | 'groq' | 'openai';

/** Settings structure matching Rust backend */
export interface Settings {
  ai_polish_enabled: boolean;
  shortcut: string;
  transcription_provider: TranscriptionProvider;
}

interface SettingsStore {
  // State
  aiPolishEnabled: boolean;
  shortcut: string;
  transcriptionProvider: TranscriptionProvider;
  dictionary: DictionaryEntry[];
  history: HistoryEntry[];
  loading: boolean;

  // Actions
  loadSettings: () => Promise<void>;
  saveSettings: (settings: Partial<Settings>) => Promise<void>;
  resetSettings: () => Promise<void>;
  loadDictionary: () => Promise<void>;
  deleteEntry: (original: string) => Promise<void>;
  clearDictionary: () => Promise<void>;
  loadHistory: () => Promise<void>;
  clearHistory: () => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  // Initial state
  aiPolishEnabled: true,
  shortcut: 'Alt+Space',
  transcriptionProvider: 'gladia',
  dictionary: [],
  history: [],
  loading: false,

  // Load settings from backend
  loadSettings: async () => {
    set({ loading: true });
    try {
      const settings = await invoke<Settings>('get_settings');
      set({
        aiPolishEnabled: settings.ai_polish_enabled,
        shortcut: settings.shortcut || 'Alt+Space',
        transcriptionProvider: settings.transcription_provider || 'groq',
      });
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
        transcription_provider: get().transcriptionProvider,
      };

      const newSettings: Settings = {
        ...currentSettings,
        ...updates,
      };

      await invoke('set_settings', { settings: newSettings });
      set({
        aiPolishEnabled: newSettings.ai_polish_enabled,
        shortcut: newSettings.shortcut,
        transcriptionProvider: newSettings.transcription_provider,
      });
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
        transcriptionProvider: 'gladia',
      }); // Default values
    } catch (error) {
      console.error('Failed to reset settings:', error);
      throw error;
    }
  },

  // Load dictionary entries
  loadDictionary: async () => {
    try {
      const entries = await invoke<DictionaryEntry[]>('get_dictionary');
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
      const entries = await invoke<HistoryEntry[]>('get_history');
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
}));
