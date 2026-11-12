# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Auto-updates

The app checks GitHub Releases for updates with the
[`tauri-plugin-updater`](https://v2.tauri.app/plugin/updater/).
Three seconds after launch (unless disabled in **Settings → Updates**) it fetches
`https://github.com/ekko-cheung/essays/releases/latest/download/latest.json`.
When a newer version exists the user can download it, the minisign signature is
verified, and the app restarts into the new version.

### Signing key

The public key lives in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
The matching private key was generated at `.tauri/essays.key` and is gitignored.
**Back it up somewhere safe** — if it is lost, existing installs can never be
updated again. The key was generated without a password.

Add one repository secret under **Settings → Secrets and variables → Actions**:

| Secret | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | The full contents of `.tauri/essays.key` |

The generated key has no password, so leave `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
unset. (If you later generate a protected key, add that secret too.)

With the GitHub CLI:

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY < .tauri/essays.key
```

### Publishing a release

1. Bump the version in `package.json`, `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml` and `core/Cargo.toml` (keep them in sync).
2. Commit, then create and push a matching tag:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. The release workflow checks the tag against `tauri.conf.json`, builds the
   macOS (ARM/Intel) and Windows installers, signs the updater bundles, and
   uploads `latest.json` alongside them.

The first release that contains the updater must be installed manually by users
on older builds. After that, releases update themselves.

To build a signed release locally:

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat .tauri/essays.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
pnpm tauri build
```
