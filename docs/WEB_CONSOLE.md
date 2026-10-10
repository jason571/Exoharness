# Web Console and Languages

English | [简体中文](WEB_CONSOLE.zh-CN.md)

## Pages and assets

The management console and user portal use TypeScript and static HTML. Source files are in `web-console/src/`; build output is in `web/`; the API module embeds these assets in the executable. Page updates require rebuilding the release package and restarting the main service.

The management console listens on port 9797 by default and provides model, assistant, user, capability, knowledge, run, and audit management. The user portal defaults to 9798 and provides registration, login, streaming chat, and personal settings. The file service defaults to 9790 and handles media playback, document downloads, and website previews. Deployment configuration can change these ports.

## Chinese and English

Management navigation is on the left, with collapse and mobile drawer behavior. Desktop pages share a consistent top-right control area for service status, account actions, theme, and language. The management console supports light and dark themes; the preference is stored under `exoharness.management.theme`.

The language controller is in `web-console/src/i18n.ts`, and interface strings are in `i18n-catalog.ts`. All production management, user, login, and registration pages load the shared `i18n.js` bundle.

The page header offers Chinese and English. The first visit defaults to Chinese; the selection is saved in browser `localStorage` as `exoharness.ui-language`, and same-origin tabs synchronize changes. Switching updates `html.lang`, interface strings, and input hints while preserving the current page and form state.

The language controller translates interface text only. User messages, model responses, conversation titles, Skill instructions, original tool progress, code, and raw Trace data retain their original language. Input values, file paths, API fields, and tool arguments are not changed. Mark newly added data regions with `data-i18n="off"` when they must remain untouched.

`MutationObserver` handles dynamically inserted interface strings by visiting only changed subtrees. Translated values retain their source text so users can switch back. Language handling is separate from model requests and execution.

## Maintenance and tests

When adding interface copy, add its English translation and identify whether the node is interface text or user data. Run:

```sh
npm --prefix web-console run typecheck
npm --prefix web-console run build
node scripts/tests/browser_i18n.mjs
```

Browser tests cover static copy across 15 production pages, language persistence and restoration, dynamic insertion, raw-content protection, form values, event handlers, and mobile layout. Authentication, file playback, and execution are covered by their respective browser and protocol tests. See [Testing](TESTING.md) for test categories.
