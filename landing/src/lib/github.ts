interface GitHubAsset {
  name: string;
  browser_download_url: string;
  download_count: number;
}

interface GitHubRelease {
  tag_name: string;
  name: string;
  published_at: string;
  body: string;
  assets: GitHubAsset[];
}

export interface DownloadLinks {
  mac: string;
  windows: string;
}

export interface ReleaseData {
  tag_name: string;
  name: string;
  published_at: string;
  body: string;
}

export interface GitHubData {
  downloadLinks: DownloadLinks;
  totalDownloads: number;
  changelog: ReleaseData[];
}

const REPO = "AmirK-S/TTP";
const FALLBACK_URL = "https://github.com/AmirK-S/TTP/releases";

/** Module-level cache so the second page build (fr/index.astro) reuses the first call's result. */
let cached: GitHubData | null = null;

export async function fetchGitHubData(): Promise<GitHubData> {
  if (cached) {
    return cached;
  }

  try {
    const response = await fetch(
      `https://api.github.com/repos/${REPO}/releases`
    );
    if (!response.ok) {
      console.warn(`GitHub API returned ${response.status}`);
      return getFallbackData();
    }

    const releases: GitHubRelease[] = await response.json();
    if (!Array.isArray(releases) || releases.length === 0) {
      console.warn("GitHub API returned empty or non-array response");
      return getFallbackData();
    }

    // Download links from latest release
    const latest = releases[0];
    const dmg = latest.assets.find(
      (a) => a.name.endsWith(".dmg") || a.name.endsWith(".zip")
    );
    const exe = latest.assets.find(
      (a) => a.name.endsWith(".exe") || a.name.endsWith(".msi")
    );
    const downloadLinks: DownloadLinks = {
      mac: dmg?.browser_download_url || FALLBACK_URL,
      windows: exe?.browser_download_url || FALLBACK_URL,
    };

    // Total download count across all releases
    const totalDownloads = releases.reduce(
      (sum, release) =>
        sum +
        release.assets.reduce(
          (assetSum, asset) => assetSum + (asset.download_count || 0),
          0
        ),
      0
    );

    // Changelog entries (top 10)
    const changelog: ReleaseData[] = releases.slice(0, 10).map((r) => ({
      tag_name: r.tag_name,
      name: r.name,
      published_at: r.published_at,
      body: r.body,
    }));

    const data: GitHubData = { downloadLinks, totalDownloads, changelog };
    cached = data;
    return data;
  } catch (error) {
    console.warn("Failed to fetch GitHub releases:", error);
    return getFallbackData();
  }
}

function getFallbackData(): GitHubData {
  return {
    downloadLinks: { mac: FALLBACK_URL, windows: FALLBACK_URL },
    totalDownloads: 0,
    changelog: [],
  };
}
