# Portal light theme and incomplete settings localization

## Symptoms

The user portal on gateway port 9798 retained a dark language control and dark category selectors after switching to the light theme. In English, account settings still displayed Chinese document and memory descriptions, empty states, the settings loading message, and built-in companion names.

## Causes

- The language control used management theme variables instead of portal theme variables.
- Light settings styles covered textareas but omitted select elements.
- Several asynchronously rendered settings messages and built-in companion labels were missing from the translation catalog.
- The settings translator was deferred behind the settings bundle, allowing the loading message to appear in Chinese before translation started.

## Changes

- Use portal colors for the language control and its options, with fallbacks for other application pages.
- Apply light settings styles to both textareas and category selectors.
- Add English translations for settings descriptions, empty states, and related actions and status messages.
- Translate built-in companion groups, names, and descriptions only within the companion gallery. User-owned names and conversation content retain their existing protection.
- Load the settings translator before parsing the body and observe parser-created text immediately. Keep language reversal and same-origin storage synchronization.

## Regression coverage

`scripts/tests/browser_i18n.mjs` includes cases for the reported dynamic messages, companion labels and Chinese reversal, unrelated names, light category selectors, and language control colors in both themes. These browser cases were added but were not executed during this change.

## Completed checks and deployment

- TypeScript type checking passed.
- The translation bundle and offline native release build completed successfully.
- The source diff passed whitespace checks.
- No active or queued Runs were found before replacement. Runtime files were backed up with an automatic rollback path before restarting the existing service.
- The running binary SHA-256 is `3127da71bd3525bfb2fb8917df802e8b0c60281532eca93c1bedd6eebb165256`.
- Authenticated HTTP responses for the embedded translation and CSS assets matched the packaged file hashes:
  - `i18n.js`: `16b4745e29188abb8259536a2ecfed27da332ef8a0f1196b65e589b3d50dc249`
  - `styles.css`: `c75dfb61da4cd68d705b82de51b6c26c0ff28e5752e283457bfdc0304bb8af9a`
- The three internal listeners and the public portal, management, and media HTTPS health endpoints returned successful responses.

These checks confirm the deployed binary and assets. They do not establish a completed browser regression run. Reload the portal and reopen account settings to load the updated resources.
