# Management light theme flashes dark during navigation

## Symptom

With the management console set to light mode, navigating between menu pages briefly displayed the dark palette before changing to light. The affected deployment uses gateway port 9797.

## Cause

The synchronous head script restored `data-management-theme="light"` before styles loaded. However, the light palette selectors also required the root `management-console` class. That class was added by `auth.js` only after the asynchronous session request completed. Until then, the default dark styles applied.

## Fix

Add `management-console` in the existing management theme head script, before restoring the saved preference. Palette selectors now match before body parsing and do not depend on authentication timing. Authentication and navigation authorization continue through the existing session flow.

## Regression coverage

`scripts/tests/browser_management_theme.mjs` captures the root class, saved theme, and computed background while parsing the head, before the deferred authentication script runs. It checks this early light palette across all ten management pages. The browser regression case was added but not executed during this change.

## Validation

- JavaScript syntax checks and source whitespace checks passed.
- The offline native release build completed successfully.
- No active or queued Runs were found before replacement. Runtime files were backed up with automatic rollback available before the existing service restarted.
- The running binary SHA-256 is `c02edfcb3f2164525ff4b0d3901792e32dfa0111a3357e7925d0d545098c6826`.
- The authenticated HTTP response for `management-theme.js` matched the packaged asset SHA-256, `29722e75947b937506212131e03e22a1de177f33842870f7f875029a0af4e3cf`. The served script sets the root class before reading the saved theme.
- The embedded management theme stylesheet matched its packaged hash. All three internal health endpoints returned successful responses.

These checks confirm the deployed binary and theme assets. They do not establish a completed browser regression run. Reload the management console to replace cached theme scripts.
