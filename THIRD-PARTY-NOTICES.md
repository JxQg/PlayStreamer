# Third-party notices

PlayStreamer is distributed under the MIT license. Third-party packages retain their own licenses.

| Component | License | Distribution |
| --- | --- | --- |
| [MediaMTX v1.21.4](https://github.com/bluenviron/mediamtx/tree/v1.21.4) | MIT | Bundled as a separate executable; original license in `src-tauri/resources/mediamtx-LICENSE` |
| [Tauri](https://github.com/tauri-apps/tauri) | MIT / Apache-2.0 | Desktop framework |
| [glib 0.18.5](https://github.com/gtk-rs/gtk-rs-core/tree/0.18.5/glib) | MIT | Vendored GTK dependency with the upstream VariantStrIter security fix; license, copyright and patch provenance in `vendor/glib-0.18.5/` |
| [React](https://github.com/facebook/react) | MIT | User interface runtime |
| [Lucide](https://github.com/lucide-icons/lucide) | ISC | Interface icons |
| [Noto Sans SC](https://fonts.google.com/noto/specimen/Noto+Sans+SC) | SIL Open Font License 1.1 | Bundled variable font for all application text; original license in `public/fonts/NotoSansSC-OFL.txt` (distributed as `fonts/NotoSansSC-OFL.txt`) |

The complete dependency versions are recorded in `Cargo.lock` and `pnpm-lock.yaml`. Source distributions and dependency packages contain their respective license notices. MediaMTX includes Go dependencies covered by their upstream licenses.

OBS Studio is a separate, user-installed application and is not bundled or linked into PlayStreamer. Its license is GPL-2.0-or-later.

The PlayStreamer application icon and tray icon artwork are original project assets covered by this repository's license. PlayStation, Twitch, Bilibili, OOPZ and OBS names belong to their respective owners; their appearance in setup instructions does not imply affiliation or endorsement.
