# Artifact delivery and previews

English | [简体中文](MEDIA_DELIVERY_DESIGN.zh-CN.md)

Images, videos, audio, documents and project files are delivered through one Artifacts service. Input attachments, in-progress files and final deliverables are managed separately. The model cannot turn an input URL or arbitrary error address into a claim of successful generation.

## Persistent files

Resources in trusted execution receipts are validated and saved with owner, Session, Run, filename, media type, size and SHA-256. Final responses are based on registered artifact facts, not links or dimensions invented by a model.

MCP `resource_link` and explicit result containers can declare artifacts. Media type determines presentation; the framework does not infer video from a service name. Duplicate resources are deduplicated. Whether intermediate files are published is determined by the workflow and its explicit publication manifest, not a hard-coded rule such as “hide the first image.”

Download and preview URLs are created from `EXOHARNESS_MEDIA_PUBLIC_BASE_URL`. The local default is `http://127.0.0.1:9790`; public deployments need the actual HTTPS domain. A local address must not be returned to a remote browser or MCP service.

## Download and playback

Downloads use signed URLs that do not expire by default. Access still depends on file retention and the signing key remaining valid. A signed link grants file access and should be handled as a credential, with redaction in public logs.

Images are served with the correct MIME type. MP3/MP4 and other supported media implement Range and HTTP 206 for browser seeking. Metadata digests and sizes must match file contents. The file endpoint does not depend on a user cookie or act as an arbitrary external URL proxy.

## Website projects

Single-file downloads and previews are separate. Multi-file project previews use a publication manifest to define the entry point and resources. Relative CSS, JavaScript, images and subpages remain accessible; the preview does not execute a backend service.

Previews use a restricted CSP/iframe sandbox without same-origin access to the main site. Browser acceptance checks scripts, resources, navigation, mobile layout and console errors—not only whether `index.html` downloads.

## Input attachments

Uploads are stored privately. Ordinary model tasks receive metadata or a reference. MCP services receive an address only when authorization and source restrictions allow it. Image inspection and reference-image handling follow user intent, the Skill and the service schema.

Implementation: `modules/artifacts`, `modules/api/src/media/` and `modules/api/src/chat/attachments.rs`. File routes are listed in the [API reference](INTERFACES.md); network and credential boundaries are in [Security](SECURITY.md).
