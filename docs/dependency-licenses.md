# Dependency license audit

Audit policy: run for every release candidate using the locked workspace dependency graph.

LensGuard itself is licensed under GPL-3.0-or-later. The packaged GNOME extension has no production JavaScript
dependencies. Its ESLint dependency is development-only and is not included in release artifacts.

`scripts/audit-licenses.sh` inspected all 125 external Rust packages in the locked Cargo graph.
Every package declares one of the reviewed SPDX expressions below, and no package has a missing
license declaration:

- Apache-2.0
- Apache-2.0/MIT
- Apache-2.0 OR MIT
- Apache-2.0 WITH LLVM-exception
- Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT
- BSD-3-Clause
- ISC
- MIT
- MIT/Apache-2.0
- MIT OR Apache-2.0
- (MIT OR Apache-2.0) AND Unicode-3.0
- MIT OR Apache-2.0 OR LGPL-2.1-or-later
- Unlicense OR MIT

The bindgen override is pinned to commit `8126d0791465053aa88cb4903330f8c4260a0184` and declares
BSD-3-Clause. The audit is a release gate: a missing or previously unreviewed license expression
causes the script to fail. Each release directory includes the complete package/version/license/
source report as `lensguard-VERSION-dependency-licenses.tsv`.

This is an attribution and compatibility review, not legal advice. Dependency source distributions
remain subject to their own license text and notice requirements.
