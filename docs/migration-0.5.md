# Migrating to 0.5

Version 0.5 keeps the existing public API paths intact. Most applications can
upgrade without source changes.

The internals no longer rely on cloned materialized prop keys or a boxed future
in version middleware. These are implementation changes only; protocol v3 page
objects and snapshots are intentionally unchanged.

`InertiaProps::with_capacity` is available when an application knows its likely
number of lazy props. It is optional and does not change resolver behavior.
