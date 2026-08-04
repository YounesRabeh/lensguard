# GNOME UX review

Environment:

- Fedora Linux 44 Workstation, GNOME Shell 50.3, GJS 1.88.1.
- Packaged extension loaded by `gnome-shell-test-tool` on an isolated 1280×720 Wayland monitor.
- Fake D-Bus service used only to create deterministic presentation states.
- Review recorded on 2026-08-04.

Scope:

- Step 10 preferences, menu presentation, text safety, and accessibility.
- Notifications were explicitly excluded and are not implemented.

Reviewed states:

1. One session displayed `Discord — Integrated Camera`, a visible symbolic camera icon, the title
   `Camera in use`, and `1 active application`.
2. Multiple sessions remained in deterministic order and the title reported the correct count.
3. A deliberately long application name was bounded to 160 Unicode characters and visually
   ellipsized by GNOME's native menu layout instead of expanding the menu.
4. Missing application and device metadata rendered as
   `Unknown application — Unknown camera`.
5. Backend unavailable rendered explicit warning language and `dialog-warning-symbolic` by
   default. Disabling warning presentation live changed it to neutral unavailable-state language
   and `dialog-information-symbolic`. Disabling the failure indicator hid only the panel icon.

Preference and accessibility checks:

- The Quick Settings submenu includes a `Preferences` action with accessible name
  `Open LensGuard preferences`.
- The Adwaita preferences window contains two documented switch rows backed by the extension's
  own GSettings schema.
- Both settings applied without restarting GNOME Shell and persisted through extension disable
  and re-enable.
- Active, inactive, service unavailable, backend unavailable, session rows, and Preferences all
  expose descriptive accessible text.
- Markup-like external names remain literal text. C0/C1 controls, bidi formatting controls, and
  excessive length are normalized before rendering.
- No state uses green/safe styling or claims that an unknown camera state is safe.

Screenshots:

![Active sessions, including long and missing metadata](../../docs/images/step10-active-sessions.png)

![Backend unavailable warning](../../docs/images/step10-backend-unavailable.png)

Commands:

```sh
make smoke-extension
make capture-extension-screenshots
```

Actual result:

- All actor assertions, live preference changes, persistence checks, text-safety unit tests, and
  screenshot checks passed.
- The screenshots were generated directly by GNOME Shell's compositor and visually inspected.

Pass/fail:

PASS
