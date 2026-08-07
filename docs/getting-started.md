# Getting started

This guide is for people who want to use Lens Guard, not develop it.

## Install from a release

Download the extension ZIP from the latest Lens Guard release, install it through GNOME
Extensions, and enable **Lens Guard**. The release also includes packages for supported Linux
distributions; install the package that matches your system when you want the bundled camera
monitor service installed for you.

After installation, open Quick Settings and look for **Lens Guard**. Start a video call or another
camera application to see the active application and camera appear there.

## Install from the source repository

If you downloaded the project source, follow the short command sequence in the
[per-user installation guide](installation.md). It installs only inside your home directory and
does not require administrator access.

## First use

1. Enable Lens Guard in the GNOME Extensions application.
2. Open a camera application such as a video meeting or camera app.
3. Open Quick Settings and expand **Lens Guard**.
4. Select **Preferences** if you want to hide the top-bar icon or change unavailable-state warnings.

If GNOME does not list a freshly installed extension, log out and back in once so GNOME Shell can
refresh its extension list.

## Remove Lens Guard

Use the **Remove** action in GNOME Extensions for a release installation. For a source checkout,
use the uninstall command documented in [Installation details](installation.md#uninstall).

## Need help?

Start with [Troubleshooting](troubleshooting.md). If the issue is about an application that opens
the camera directly through V4L2 rather than PipeWire, read the detection limitations there before
reporting a bug.
