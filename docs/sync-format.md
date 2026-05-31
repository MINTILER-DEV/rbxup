# Sync Format

`rbxup` sync is an early Studio-facing project format that keeps readable structure in normal files and pushes noisy scene spam into bundles.

## Goals

- Keep services, models, folders, scripts, remotes, and config-like objects readable on disk.
- Avoid exploding large scenes into thousands of tiny files for `Part`, `MeshPart`, `Attachment`, `Decal`, `Texture`, and constraints.
- Preserve stable IDs so rename, move, and patch behavior stay sane.

## File Types

- `.xup`: readable metadata for one Roblox instance.
- `.xzup`: readable bundle pointer file.
- `.xbundle.zip`: compressed bundle under `bundles/`.
- `.lua`: exploded script source.
- `init.xup`: metadata for a folder-backed instance.
- `init.server.lua`, `init.local.lua`, `init.lua`: script folder entrypoints.

## Script Rules

- `Script`, `LocalScript`, and `ModuleScript` are always exploded.
- A script with no children may be a single `.lua` file.
- A script with children becomes a folder with `init.*.lua` plus `init.xup`.
- Scripts are never stored inside `.xzup` bundles.

## Bundle Rules

- Bundle decisions are based on direct children only.
- Readable children stay exploded.
- Bundleable noisy children are grouped by class name.
- Virtual class-group pointers use lowercase names like `_part.xzup` or `_meshpart.xzup`.
- `.xzup` is metadata plus a pointer. The actual payload lives in `bundles/<prefix>/<hash>.xbundle.zip`.

## `.xzup` Kinds

- `kind: "instance"` means a real Roblox instance is represented by a bundle pointer file.
- `kind: "child-group"` means the file is a virtual grouping for loose children restored directly under a parent.

## Bundle Contents

Each `.xbundle.zip` contains:

- `manifest.json`
- `instances.jsonl`

`manifest.json` describes the bundle shape. `instances.jsonl` contains one instance record per line.

## Stable IDs

Every synced instance should carry an `rbxup_id` attribute in Studio.

- Same ID + new name means rename.
- Same ID + new parent means move.
- Missing ID means unmanaged/new.
- Duplicate IDs are warnings or errors depending on the stage.

## Property Serialization

Only useful properties should be serialized.

- identity and rebuild properties
- non-default values
- attributes
- tags
- references needed for restoration
- script source

Property reads and writes in the plugin should always be protected with `pcall`.

## CLI / Bridge Design

Current early commands:

- `rbxup sync pull <dir>`
- `rbxup sync push <dir>`
- `rbxup sync diff <dir>`
- `rbxup sync serve <dir>`
- `rbxup sync doctor <dir>`

Today these focus on:

- project manifest validation
- `.xup` / `.xzup` JSON parsing
- `.xbundle.zip` read/write helpers
- localhost bridge scaffolding
- machine-readable stdout

## Safe Push Behavior

- Push is patch-first by default.
- Missing file properties should not reset Studio by default.
- Unmanaged Studio instances are not deleted by default.
- Deletion should only happen when explicitly enabled.
- Partial failures should report which instances or files failed.

## Current Status

This is an architecture-first scaffold.

- The disk format and bundle helpers exist.
- The CLI can seed and validate a sync project.
- The plugin source is scaffolded in XLuau and compiled to Luau.
- Full Studio import/export and API-dump-driven property coverage are still TODO.
