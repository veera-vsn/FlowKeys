# FlowKeys

One lightweight, offline-first Windows utility for global hotkeys, clipboard history, and text
snippets — replacing the usual stack of three separate tools. No account, no cloud, nothing leaves
your machine.

Built with [Tauri 2](https://tauri.app) (Rust) and React + TypeScript.

> **Status: pre-release.** Sprints 1–5 of the [roadmap](docs/PRD-addendum-positioning-monetization.md)
> are complete — the entire free tier is implemented and working. There is no installer or
> auto-update yet, so running it today means building from source.

## Features

### Global hotkeys
Shortcuts registered with the OS, so they fire from any application even when FlowKeys is minimized
to the tray. Add your own, rebind them, or switch them off individually. Conflicting combinations
are rejected with the name of whatever already claims them, and every shortcut needs at least one
modifier so it can't hijack ordinary typing.

A built-in **Toggle Clipboard Popup** binding ships enabled on `Alt+Shift+V`. It can be rebound or
disabled but not deleted, since removing it would leave no way to summon the popup.

### Clipboard history
Everything you copy is captured automatically and kept locally — up to 500 entries, searchable by
substring. Re-copying an entry moves it back to the top.

`Alt+Shift+V` opens a floating popup from anywhere: type to filter, arrow keys to move, `Enter` to
copy. `Esc`, clicking away, or the `×` button dismisses it.

### Snippets
Type a trigger anywhere on your system and it expands into the full text. Managed through a visual
editor rather than a config file. Triggers should be strings you'd never type by accident — `;addr`
rather than `addr`.

Expansions are inserted by **pasting**, not by simulating keystrokes. Typing character-by-character
proved unreliable in practice: characters arrived out of order, got dropped, and mangled newlines,
because each one is a separate event racing through the input queue. A paste is atomic, so
multi-line snippets like addresses and signatures survive intact.

### Copy-on-selection *(opt-in)*
Off by default. When enabled, selecting text with the mouse copies it automatically, anywhere on
your system, with a small confirmation showing the character count.

**Hold `Ctrl` while selecting to leave the clipboard untouched.** This matters more than it sounds:
selecting text in order to paste over it is the same gesture as selecting text to copy it, so
without an opt-out the selection would destroy the very thing you meant to paste. `Ctrl` is already
held for the `Ctrl+V` that follows, so the whole replace flow stays one motion.

## Requirements

- Windows 10/11
- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18+
- **Visual Studio Build Tools** with the *Desktop development with C++* workload — Tauri cannot link
  on Windows without the MSVC toolchain. Installing Rust alone is not enough.

## Running from source

```bash
npm install
npm run tauri dev
```

If `cargo` isn't found, or linking fails with a missing `link.exe`, the MSVC toolchain isn't on your
`PATH`. Launch the *Developer PowerShell for VS* and run from there, or import the dev shell in your
existing session:

```powershell
$vs = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"
Import-Module "$vs\Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments "-arch=x64 -host_arch=x64"
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

To produce a standalone executable and installer:

```bash
npm run tauri build
```

### Checks

```bash
cd src-tauri && cargo test --lib      # unit tests
cd src-tauri && cargo clippy --all-targets
npm run build                          # tsc + vite
```

## Where your data lives

Everything is plain JSON under `%APPDATA%\com.flowkeys.app\`, and nothing is transmitted anywhere:

| File | Contents |
|---|---|
| `hotkeys.json` | Shortcut bindings |
| `clipboard_history.json` | Captured clipboard entries |
| `snippets.json` | Snippet triggers and expansions |
| `settings.json` | Application preferences |

Deleting a file resets that feature.

## Known limitations

- **FlowKeys has to be running.** The keyboard hook, clipboard watcher, and registered shortcuts all
  live in the process. Closing the window hides it to the tray and keeps everything working — only
  tray → *Quit FlowKeys* actually stops it. There is no "launch at login" yet, so it must be started
  manually after a reboot.
- **Snippets and copy-on-selection use `Ctrl+V` / `Ctrl+C`.** Applications that bind paste elsewhere
  won't work — most terminals use `Ctrl+Shift+V`. In a terminal, `Ctrl+C` also interrupts whatever
  is running, which is why copy-on-selection ships disabled.
- **Snippet triggers must be typed in one go.** Matching is against a buffer of recently typed
  characters, which is deliberately discarded whenever the caret moves — an arrow key, a click,
  `Enter`, or a `Ctrl` shortcut. Otherwise a later match would backspace over text you never typed
  as part of a trigger.
- **`Ctrl+drag` behaves differently in some editors** (multi-cursor in VS Code, sentence selection
  in Word), so the selection itself may not come out as expected there. That's the host
  application's behavior and outside FlowKeys' control.
- **The keyboard hook may trip antivirus heuristics.** Watching global keystrokes is what text
  expansion requires, and it looks structurally like a keylogger even though nothing is transmitted
  and everything stays on disk locally.

## Docs

- [PRD Addendum: Competitive Positioning, Monetization & Revised Roadmap](docs/PRD-addendum-positioning-monetization.md)

## License

See [LICENSE](LICENSE).
