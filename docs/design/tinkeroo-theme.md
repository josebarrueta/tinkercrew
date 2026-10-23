# Tinkeroo: playful, focused making

Tinkeroo is the maker sibling to Rallyroo's family-calendar naming theme. **Crew** remains the family group, not an account system. The shared tagline is **Build. Code. Imagine. Together.** The public application name is Tinkeroo, formerly TinkerCrew; the GitHub repository is still `josebarrueta/tinkercrew`.

## Visual direction

A warm, friendly roo icon invites participation, but the Mission—not the mascot—is the main attraction. Use quiet lilac/white surfaces, generous spacing, readable rounded/system typography, and small areas of saturated color. No bouncing characters, flashing signals, celebratory confetti, background sound, autoplay decoration, or external font downloads are introduced.

Shared tokens live in `app/src/lib/theme.css`:

| Role | Color | Use |
| --- | --- | --- |
| Canvas | `#f6f7fc` | Calm workspace |
| Surface | `#ffffff` | Current task/cards/forms |
| Primary purple | `#5842bd` | Next action, current navigation, progress |
| Soft lilac | `#f0edfc` / `#ebe5ff` | Gentle grouping/selection |
| Teal | `#137c73` | Ready/supportive cues |
| Sunshine | `#ffd66b` | Small decorative accents, with dark text |
| Coral | `#f18b70` | Reserved small illustrations, not body text |
| Ink / muted ink | `#26304b` / `#59647c` | Readable text, not pale low-contrast gray |
| Focus blue | `#315acb` | Visible keyboard-focus outline |

Purple consistently answers “what can I do next?” Do not make every control a bright filled button. Secondary actions are outlined; advanced telemetry/configuration stays in expandable panels. Home offers one direct Mission action rather than competing large destinations. During a Mission, Crew editing opens for the roles stage and remains available in a compact disclosure otherwise. A labeled progress bar shows the adventure's position without pressuring a child with time limits.

## Safety and accessibility

- Hardware sleeping/curious/alert **green/yellow/red values are unchanged**. Purple branding never changes a guardian's signal rules.
- Hardware output indicators retain text labels (including off), and connection state uses text, not just a colored dot. Real safety warnings remain amber and failures red; neither is decorative celebration.
- The shared base gives buttons/selects at least 44 px height, a 3 px visible keyboard-focus outline, readable form text, and reduced-motion overrides. There are no new ongoing animations, including in the default theme.
- Browser tests measure actual rendered primary-button and selected-navigation text contrast against the WCAG 4.5:1 normal-text threshold; this is not a claim of a complete accessibility audit.
- Desktop minimum width remains 900 px, matching the native window. At narrower desktop widths the hero illustration disappears and supporting cards/lab controls stack rather than overlap the primary task.
- No visual judgment is made about LEGO design, creativity, age, or performance. The palette is a design proposal awaiting real family feedback, not a claimed validated attention intervention.

## Rename compatibility

Changed: display/window titles, npm package display name, app/DMG/ZIP/release asset filenames, CI build-artifact prefix, primary README/Companion display documentation, and app icons.

Intentionally unchanged to avoid losing trust/progress or silently renaming infrastructure:

- GitHub repository/clone path and release endpoint repository.
- `com.tinkercrew.desktop` bundle identifier and related Keychain services.
- `tinkercrew.progress.*` browser progress keys and native progress database location.
- `_tinkercrew._tcp` Bonjour service, TBP protocol, native Rust crate/executable names, Companion Python module and installed `tinkercrew-companion` directory, `TC_*` configuration variables.
- Historical research/protocol records and original copyright attribution.

Existing state is not renamed/deleted. macOS may still ask for Keychain access again when the application's code signature/build changes; never work around that by copying secrets into progress storage. A future technical-identifier/repository migration needs a separate explicit plan.

## Checks and previews

`app/e2e/theme.spec.ts` checks Tinkeroo branding, the direct Mission path, task disclosure/progress, keyboard operation/focus, primary/selection contrast, 44 px action size, and reduced-motion behavior. It captures all five screens under ignored `app/test-results/`; CI retains them with QA artifacts for seven days. The pre-existing full Mission, recovery, setup, and backend suites remain in place.

Build outputs are now `Tinkeroo.app` and `Tinkeroo_VERSION_ARCH.dmg`; CI/release scripts and the read-only DMG layout check use the same names. Production signing/notarization and family/hardware acceptance remain pending.
