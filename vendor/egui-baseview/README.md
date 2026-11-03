# Vendored egui-baseview

Vendored from egui-baseview 0.5.0 at revision
`ec70c3fe6b2f070dcacbc22924431edbe24bd1c0`.

Patched so keyboard events are returned as `EventStatus::Ignored` when egui does
not want keyboard input, except for Tab, Escape, and the arrow keys. This lets DAW
transport shortcuts such as Space keep working while the plugin editor is focused.
Space is always passed through, including while a numeric parameter field is being
edited. Text fields still capture other keyboard input normally.

The patch is limited to `src/window.rs`. The crate is MIT licensed; see `LICENSE`.
