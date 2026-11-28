# Essays

Essays is a native Markdown notebook for Windows x64 and macOS 15 or later on Apple Silicon and Intel. Its desktop UI uses GPUI Kit, its notes live in SQLite, and Cloudflare D1 sync is optional.

## Run from source

Install stable Rust and the platform tools in the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation/). On Windows use the MSVC toolchain. On macOS install Xcode Command Line Tools.

```sh
cargo run --locked -p essays
cargo test --locked --workspace
```

Debug builds accept `ESSAYS_DEV_DATA_DIR` for an isolated development profile. Set it to a test directory before running the app if you use Essays on the same computer.

## Move from the previous release

Fully quit the old app, including its tray process, and download the matching installer from [GitHub Releases](https://github.com/ekko-cheung/essays/releases). On macOS, replace `Essays.app` in Applications. On Windows the new setup has its own install entry. You can remove the old application through Windows Settings after verifying that the new app opens your notes. Keep user data if the old uninstaller asks.

Essays automatically opens the existing `essays.db` and `settings.json` in:

- Windows: `%APPDATA%\com.essays.app`
- macOS: `~/Library/Application Support/com.essays.app`

Your notebooks, notes, and sync settings carry over. Appearance and layout preferences stored in the old web frontend start at their native defaults. The editor opens in Split mode by default. Use `Cmd/Ctrl+1` for Live Preview, which renders Markdown in the editor and reveals the source on lines you edit. Use `Cmd/Ctrl+2`, `3`, and `4` for Source, Split, and Preview.

In **Settings → Updates**, Essays can check for a newer stable release and open its download page. Install the downloaded update after exiting Essays.

## Package and release

The desktop crate's version in `desktop/Cargo.toml` is the release version. After bumping it and refreshing `Cargo.lock`, run the **Release** workflow manually to build review artifacts. Pushing a matching `vX.Y.Z` tag publishes the three installers and SHA-256 checksums after all builds and tests pass.

For local packaging, build the matching target with `cargo build --release --locked -p essays --target <target>`. On Windows install Inno Setup 6 and the Visual Studio x64 redistributable files, then run `./packaging/windows.ps1 -Version X.Y.Z`. The script includes the required Visual C++ runtime library in the per-user installer. On macOS run `bash packaging/macos.sh X.Y.Z <target>`. Artifacts appear in `dist/`.

The default CI artifacts use ad-hoc signing on macOS and have no Windows Authenticode signature. For Developer ID distribution, install the identity on the packaging host, set `MACOS_SIGNING_IDENTITY`, and set `MACOS_NOTARY_PROFILE` to an existing `notarytool` keychain profile. Sign the Windows binary before packaging and the installer after packaging if an Authenticode certificate is available.
