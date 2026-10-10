# Duplicate video cards from assistant prose

## Symptom

A completed teaching-video conversation displayed two video players. One played and downloaded successfully; the other failed for both actions.

## Evidence

The execution receipt declared one video, approximately 2.3 MB and 58.1 seconds long. Its complete signed URL returned HTTP 206 with `video/mp4` and a valid byte range. The final model answer repeated the file URL without its signed query. That unsigned URL returned HTTP 404.

The production Chat bundle matched the inspected source bundle and contained the prose-to-video fallback. No second video-generation job was responsible for the extra player.

Production session identifiers, account data and signed access URLs are omitted from this report.

## Cause

`videoArtifactsFromRichText` scanned rendered assistant links for paths ending in `.mp4` and created additional artifact descriptors. Both streaming completion and persisted history combined those descriptors with the official execution artifacts.

The video renderer deduplicated by the complete URL. A signed URL and its unsigned counterpart therefore became separate cards. The prose URL had no registered artifact receipt and could not pass the file service's signature validation.

## Fix

- Remove the prose-to-video fallback from Chat.
- Render video cards exclusively from declared execution artifacts, for streaming completion and persisted history.
- Keep the exact playback and download URLs returned by execution.
- Hide redundant Markdown links whose origin and path match a declared media file, including links where the model omits or changes the signed query.
- Keep unrelated links as ordinary links, without converting them into players.

The fix does not rewrite stored model answers or relax file-service signature checks.

## Regression case

`scripts/tests/chat-artifact-authority.mjs` covers persisted history and reload with one declared signed video and an unsigned duplicate in the answer. It also covers an unrelated prose-only video link. Expected results: one player, the exact signed playback/download URL, no unsigned media fetch, and no player for the unrelated link.

The case uses sample metadata and does not invoke models or MCP services. The regression case was added but not executed during this change.

## Deployment

Target: the ExoHarnessPro service on the existing Linux deployment. Page assets are embedded, so the service binary must be rebuilt and restarted. Check active Runs before updating, back up runtime files, and retain a rollback path. After updating, verify the running binary digest, embedded Chat bundle, three health endpoints and access to the existing video.

### Completed checks

- TypeScript type checking and the native release build passed.
- No active or queued Runs were found before replacement.
- Runtime files were backed up, with automatic restoration available if startup checks failed.
- The running binary SHA-256 is `eb0039b1cedb80a92ba715ec7f309eb2363fb37430366583c89eac2ba732c2d8`.
- The authenticated served Chat bundle SHA-256 is `2cfd04eb5248646efd823934bc04a96c4219dab2b7b9116aa5da8906eec849f1`, matching the packaged bundle. It no longer contains the prose-to-video fallback.
- All three internal listeners and public HTTPS health endpoints returned successful health responses.
- The existing completed Run and its declared video remained available after restart; the signed video URL returned HTTP 206 with `video/mp4`.

These checks establish the deployed asset version and existing file access. They do not claim a completed browser regression run or a new model/video-generation task. Refreshing the browser loads the corrected history renderer.
