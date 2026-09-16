# Getting an installable build without releasing anything

Written 2026-08-31 for workstream G. It answers one question: *how do I put a
real, signed TTP on my Mac and look at it, without publishing anything to
anyone?*

## What the situation was

There were exactly two CI paths, and neither of them did this.

| | `build.yml` (before) | `release.yml` |
|---|---|---|
| Fires on | push/PR to `main`, `master` | tags matching `v*` |
| Produces | `.dmg`, `.app`, `.exe` as workflow artifacts | a public **GitHub Release** |
| macOS signature | **ad-hoc** | Developer ID + notarised |

So a branch could be pushed and get no build at all, and the only way to get a
signed build was to tag it and publish it to end users.

The ad-hoc part is the expensive half, and it was not obvious. `tauri.conf.json`
carries `bundle.macOS.signingIdentity: "-"`, which means *ad-hoc*. `tauri-cli`
resolves the identity as:

```rust
let signing_identity = match std::env::var_os("APPLE_SIGNING_IDENTITY") {
    Some(id) => Some(id...),
    None => config.macos.signing_identity,   // "-"
};
```

The environment variable wins when it is set. `release.yml` sets it.
`build.yml` never did. An ad-hoc signature has no stable identity, so macOS
treats each rebuild as a different application: microphone and accessibility
permissions are re-requested, and the keychain item is a stranger's, so it asks
for the login password. Every single test install. That is the tax workstream G
exists to remove.

## What exists now

`build.yml` grew a third path. Same file, same job, different classification.

| Trigger | Signed | Notarised | Cosmetics | Purpose |
|---|---|---|---|---|
| push/PR to `main`/`master` | only if secrets present | no | no | compile gate, as before |
| push to `polaris/**` | **yes, required** | yes | **yes** | maintainer build |
| `workflow_dispatch`, any branch | **yes, required** | yes | yes (toggleable) | maintainer build |

"Required" is literal: a maintainer build that finds no `APPLE_CERTIFICATE`
fails in the first thirty seconds with an explicit error rather than spending
eight minutes producing an artifact that would reproduce the exact problem it
was meant to solve.

It reuses `release.yml`'s secrets verbatim — `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_API_KEY_BASE64`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER`. No new secret
was added and none needs to be.

No tag is created. No release is published. `permissions: contents: read`.

## How to get a build

### The way that works today

Push to a `polaris/**` branch. That is all.

```sh
git push origin polaris/v3.0.0
```

To rebuild the same code later:

```sh
git commit --allow-empty -m "rebuild" && git push origin polaris/v3.0.0
```

### The button, once this reaches `master`

GitHub only offers the **Run workflow** button for workflows that carry a
`workflow_dispatch` trigger *on the default branch*. This file has the trigger,
but master's copy does not yet, so the button and `gh workflow run` will both
refuse until the change is merged. After that:

```sh
gh workflow run build.yml --ref polaris/v3.0.0 -f cosmetics=true -f platforms=macos
```

or Actions → *Build TTP* → **Run workflow** → pick the branch.

### Downloading it

```sh
gh run list --workflow=build.yml --branch polaris/v3.0.0 --limit 5
gh run watch <run-id>
gh run download <run-id> --name 'TTP-macOS-3.1.7-<sha>'
```

Or Actions → the run → **Artifacts** at the bottom of the summary page.

The artifact is named `TTP-macOS-<version>-<short sha>` so several downloads
can coexist in a Downloads folder without guesswork. Retention is 90 days.

macOS artifacts are now the `.dmg` **only**. The raw `.app` used to be uploaded
alongside it; that was quietly harmful, because `upload-artifact` zips its input
without preserving symlinks or the executable bit, so the `.app` arrived with a
broken signature — indistinguishable, on the receiving Mac, from the ad-hoc
problem. A `.dmg` is one opaque file and survives the round trip byte-identical.

### Checking what you got

The run summary states whether it is signed, notarised and cosmetics-unlocked.
The **Verify the built app** step prints the actual `codesign`, `spctl` and
`stapler` output, so the claim can be checked rather than believed. On your own
machine, after installing:

```sh
codesign -dvv "/Applications/TTP by AmirKS.app" 2>&1 | grep Authority
spctl --assess --type execute -vv "/Applications/TTP by AmirKS.app"
```

`Authority=Developer ID Application: ...` and `source=Notarized Developer ID`
is what a good build looks like.

## The Companion cosmetics in a maintainer build

`cosmetics::unlocked()` reads `TTP_COSMETICS` from the environment at runtime,
not at compile time, so there is nothing to bake in at build time. The seam that
does work is the one already in use for `LSUIElement`: `bundle.macOS.infoPlist`
points at `src-tauri/Info.plist`, and LaunchServices applies an `LSEnvironment`
dictionary from an app's `Info.plist` when the app is launched from Finder or
the Dock.

So the workflow patches the **runner's checked-out copy** of `Info.plist` before
the bundle is built:

```
LSEnvironment = { TTP_COSMETICS = "1" }
```

The file in git is untouched, no Rust is edited, the gate is unchanged in the
product, and release builds never see it because the step only runs for
maintainer builds. The key goes in before signing, so the signature covers it.

**Caveat, stated plainly.** That the key is *present in the bundle* is verified
in CI (the Verify step prints it). That LaunchServices *honours* it on the
installed app is expected but not proven — `LSEnvironment` is a long-standing
key and hardened runtime only strips `DYLD_*`, but nobody has yet launched a
build made this way. If the cosmetics do not appear, either of these forces it:

```sh
# per-launch, from Terminal
TTP_COSMETICS=1 "/Applications/TTP by AmirKS.app/Contents/MacOS/TTP"

# for the whole login session, then launch normally
launchctl setenv TTP_COSMETICS 1
```

## What this path deliberately does not do

- It does not create a tag.
- It does not publish a GitHub Release.
- It does not build updater artifacts, so no existing installation can be
  pointed at one of these builds by the auto-updater. `release.yml` remains the
  only thing that can put a binary in front of anyone.
