// TTP - Talk To Paste
// First-run API key setup window

import { ApiKeyForm } from '../components/ApiKeyForm';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * ApiKeySetup is the first-run experience window.
 * It prompts users to enter their OpenAI API key before they can use TTP.
 * After successful setup, the window closes automatically.
 */
export function ApiKeySetup() {
  const handleSuccess = async () => {
    // Close setup window after successful save
    const window = getCurrentWindow();
    await window.close();
  };

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
      <div className="max-w-md mx-auto">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            Welcome to TTP
          </h1>
          <p className="mt-2 text-gray-600 dark:text-gray-400">
            Talk To Paste uses OpenAI's Whisper API for transcription.
            Enter your API key to get started.
          </p>
        </div>

        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-sm p-6">
          <ApiKeyForm onSuccess={handleSuccess} />
        </div>

        <div className="mt-6 text-center">
          <p className="text-xs text-gray-500 dark:text-gray-400">
            You only pay for what you use. No subscription required.
          </p>
        </div>
      </div>
    </div>
  );
}

export default ApiKeySetup;
