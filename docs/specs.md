# Scratchpad — Quick Notes

## Overview

A lightweight, instant-access scratchpad for temporary notes while programming. Designed to appear immediately via global shortcut without launching a full editor. Focused on speed, local-only storage, and seamless integration into a developer workflow.

## Core Features

### Instant Access
- Opens via a configurable global keyboard shortcut (no need to focus or launch a heavy editor).
- Minimal, always-ready floating or overlay window optimized for quick capture and discard.

### Markdown Support
- Native Markdown editing and preview.
- Auto-saves content as Markdown.
- Restores the previous state after logout / session restart.

### Multiple Scratchpads
- Support for multiple independent scratchpads.
- Association by:
  - Project
  - Workspace
  - Git repository (detected automatically when possible)
- Easy switching between scratchpads.

### Content Utilities
- Quick actions for common developer items:
  - Code snippets
  - TODOs
  - URLs
  - Terminal commands
- One-click copy of selected content or whole note.

### Search & Persistence
- Full-text search across scratchpad content.
- Ability to promote / export a scratchpad (or selection) to a regular Markdown file on disk.
- All data stored locally as files — no database, no cloud sync.

## Technical Stack

- **Language**: Rust
- **UI**: GTK4 or Slint (prefer the one that provides the best Wayland + modern desktop integration experience)
- **Storage**: Local filesystem only (Markdown files + simple metadata). No database. No network/cloud dependency.

## Target Environment

- **OS**: Fedora Workstation 44
- **Display Server**: Wayland
- **Compositor / Window Manager**: Niri
- **Shell / Desktop Components**: DankMaterialShell
- **User Shell**: Fish

The application must work reliably under Wayland with Niri, respect system theming where possible, and integrate cleanly with the global shortcut system available on this setup.

## Non-Goals

- Cloud synchronization
- Multi-device access
- Complex note organization beyond project/workspace/Git association
- Full-featured Markdown editor (tables, advanced plugins, etc.)
- Any network functionality

## Success Criteria

- Global shortcut opens the scratchpad in under 100–150 ms on typical hardware.
- Content is never lost across logout / reboot (auto-save + restore).
- Multiple scratchpads can be created, switched, and associated with projects/repos with minimal friction.
- Works cleanly on the specified Fedora + Niri + Wayland stack.
