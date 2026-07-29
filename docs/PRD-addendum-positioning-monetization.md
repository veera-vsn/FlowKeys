# FlowKeys

### PRD Addendum: Competitive Positioning, Monetization & Revised Roadmap

*Version 1.1 · Companion to MVP PRD v1.0*

---

## 1. Why This Addendum Exists

The v1.0 MVP PRD scoped a strong build plan but didn't answer three questions a reviewer or investor will ask immediately: what stops someone from just using free tools instead, how does this make money, and does voice/AI belong in the first release. This addendum answers all three, grounded in the current (2026) competitive landscape rather than assumption.

---

## 2. Competitive Landscape

Every core Phase 1–4 feature in the original PRD already has a free, actively maintained equivalent. This doesn't invalidate the idea — it changes what's being sold.

| Feature | Free incumbent today | Status | FlowKeys wedge |
|---|---|---|---|
| **Global hotkeys** | Microsoft PowerToys (25+ free utilities, monthly updates) | Commoditized | Must be invisible plumbing, not a selling point |
| **Clipboard history** | Windows Win+V (built-in) | Commoditized | Search speed + cross-feature integration |
| **Text snippets** | Espanso (free, OSS, macros, encryption) | Commoditized, more powerful than MVP scope | Visual editor vs. Espanso's YAML-only config |
| **Voice / dictation** | WhisperPress, OpenWhispr, whisper-local (all free, OSS, offline, push-to-talk) | Commoditized as of 2026 | Not a differentiator — defer past MVP |

> **The honest takeaway**
> FlowKeys cannot win by being first. It has to win by being the one coherent tool instead of four stitched-together ones — one settings UI, one background process, one mental model. That is a real, defensible pitch, but it's a UX and integration bet, not a feature-invention bet, and the roadmap and messaging need to reflect that.

---

## 3. Differentiation Statement

Use this as the one-sentence test for every future feature decision:

> **FlowKeys is...**
> ...the single lightweight app that replaces the five-tool stack (PowerToys + Espanso + a clipboard manager + a separate dictation app + AutoHotkey scripts) with one fast, native-feeling, offline-first utility — for people who want power-user productivity without becoming a systems administrator of their own toolchain.

**Supporting proof points:**

- Espanso's own users cite its YAML-only configuration and lack of a GUI as a real pain point — a visual editor over equivalent power is a legitimate wedge.
- Power users today are visibly stitching together separate tools (AutoHotkey for hotkeys, Espanso for snippets, a separate app for dictation) — that integration tax is the gap FlowKeys fills.
- "No AI, works offline, no account" is a positioning choice, not a limitation — but it must be stated deliberately, or reviewers will default to comparing FlowKeys against AI-native tools like Wispr Flow and find it lacking.

---

## 4. Monetization Model

**Recommendation: freemium**, with sync and AI-assisted features as the paid hook. The core utility stays free and fully functional offline forever — this protects the "privacy-first, no account required" principle from the original PRD. Paid tier is opt-in and clearly additive.

| Free | Pro — $4/mo or $30/yr | Rationale |
|---|---|---|
| Unlimited hotkeys | Everything in Free, plus: | Hotkeys are the plumbing — gating them kills the core loop |
| Clipboard history (local, 500 items) | Unlimited clipboard history | Power users hit the cap; casual users never notice |
| Snippets (local only, unlimited) | Cross-device snippet + settings sync | Sync requires a backend — real recurring cost, real recurring value |
| — | AI-assisted snippet/macro builder | Genuine differentiator vs. Espanso/PowerToys, neither of which is AI-native |
| — | Smart clipboard (format detection + conversion) | Small AI-lite feature, cheap to run, clearly "pro" territory |
| Voice typing (basic model, local) | Voice typing (larger/faster model, custom vocab) | Ship free initially since it's already commoditized — see note below |

**Pricing note:** $4/mo or $30/yr undercuts Espanso alternatives with paid tiers (Text Blaze, PhraseExpress) while staying above "impulse buy and forget" territory. Validate against actual willingness-to-pay data post-beta — treat this as a starting hypothesis, not a committed number.

---

## 5. Should Voice/AI Be in the MVP?

**No.** Recommendation: cut voice typing from MVP scope, move it to a post-beta sprint gated on real usage data.

This is a sequencing decision, not a philosophical one about AI:

- Voice typing is the hardest engineering problem in the whole roadmap (model bundling, CPU/GPU fallback, latency tuning) — it shouldn't gate the first release of everything else.
- It's also now the least differentiated feature. Multiple free, MIT-licensed, actively maintained tools already ship exactly this spec: offline, push-to-talk, whisper.cpp-based, no cloud, no subscription.
- Cutting it from MVP doesn't mean "no AI ever." It means don't let the hardest and least-differentiated feature block shipping the three features (hotkeys, clipboard, snippets) that are genuinely usable and testable today.
- If voice ships later, it should ship as a Free-tier feature (since it's already commoditized) — the paid hook should be the AI-assisted snippet/macro builder and smart clipboard instead, since neither Espanso nor PowerToys is AI-native and both are real gaps.

> **Where AI actually helps, if added later**
> Not "offline dictation" (already free and everywhere). Instead: AI-assisted snippet/macro creation (natural-language description → working snippet) and smart clipboard (auto-detect and reformat JSON/CSV/dates on paste). Both are small, cheap to run, and absent from every free competitor — a real wedge rather than a reimplementation.

---

## 6. Revised Roadmap

Same sprint structure as v1.0, with voice moved from Sprint 7 to Sprint 10 (gated on beta data), a beta checkpoint inserted at Sprint 6, and paid-tier features slotted in immediately after.

| Sprint | Focus | Outcome | Tier |
|---|---|---|---|
| 1 | Foundation + Tray + Settings | Stable shell application | Free |
| 2 | Hotkey engine | Reliable global shortcuts | Free |
| 3 | Clipboard history + search | First feature used daily | Free |
| 4 | Auto-copy + clipboard popup | Complete clipboard workflow | Free |
| 5 | Snippets (GUI, not YAML) | Text expansion, visual editor | Free |
| 6 | Beta launch + telemetry opt-in | Real usage data on retention | — |
| 7 | Cloud sync (settings + snippets) | First paid hook | **Pro** |
| 8 | AI-assisted snippet/macro creation | Natural-language → snippet | **Pro** |
| 9 | Smart clipboard (auto-format JSON/CSV/dates) | AI-lite utility, still mostly local | **Pro** |
| 10 | Voice typing (whisper.cpp, push-to-talk) | Offline dictation, revisited post-data | Pro or Free — decide after Sprint 6 data |
| 11 | Polish, installer, updater | Public 1.0 release | — |

---

## 7. Summary of Changes from PRD v1.0

- **Added:** competitive analysis showing every MVP feature has a free 2026-era equivalent.
- **Added:** one-sentence differentiation statement to test future feature decisions against.
- **Added:** freemium monetization model (sync + AI features paid, core utility free forever).
- **Changed:** voice typing moved from Phase 4 (pre-launch) to a post-beta sprint, gated on usage data.
- **Changed:** "No AI in MVP" reframed as a sequencing decision, not a permanent principle — AI-assisted snippet/macro creation and smart clipboard are the recommended AI wedge for the Pro tier, not offline dictation.
