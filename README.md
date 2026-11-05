# RPT Advanced Hamlib adapter

This shared object provides typed radio control through Hamlib 4.6.2 or later.
Its public C contract is in
`include/rptadv_hamlib_adapter/rptadv_hamlib_adapter.h`. The ABI and SONAME major
are 1. Consumers use the descriptor returned by `rptadv_hamlib_api_v1()`.

One non-audio owner opens, operates, closes, and destroys each rig. Calls on
different opened rigs run independently. Process-wide Hamlib lifecycle work is
serialized. Every transport uses explicit finite timeout and retry settings;
the reported exchange budget does not promise whole-operation cancellation.

The catalog reports per-connection modes, coverage, read/write access, units,
and discrete constraints. Numeric model configuration, raw CAT commands, and
native Hamlib handles are not exposed. Readback returns actual values, and
failed operations preserve their numeric cause. The shared remote-base engine
and physical owner enforce node RF limits and choose PTT/signaling providers.

The adapter dynamically links the released LGPL-2.1-or-later Hamlib library.
No Hamlib implementation is bundled. Private bindings are generated from its
installed headers. The adapter is GPL-2.0-only.

Focused tests use Dummy rigs and loopback transports. They do not qualify a
physical radio. `QUALITY.md` lists build, developer-documentation, package,
and installed-ABI checks.
