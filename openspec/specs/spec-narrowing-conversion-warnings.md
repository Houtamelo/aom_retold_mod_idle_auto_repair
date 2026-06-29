# Narrowing-Conversion Warnings

**Requirement**: The LSP server MUST emit a `DiagnosticSeverity::WARNING`
diagnostic for narrowing numeric conversions in function-call arguments.

**Scenarios**:

**Given** a user-defined function `void takeInt(int x) {}`
**When** called with a float argument whose value has a fractional part
**Then** the server publishes a `DiagnosticSeverity::WARNING` whose message names the conversion (`float` → `int`), names the truncated value, and suggests `int(...)` as an explicit unblock.

**Given** a user-defined function `void takeInt(int x) {}`
**When** called with a float literal whose value has no fractional part
(`takeInt(1.0)`, `takeInt(2.0)`, `takeInt(0.0)`)
**Then** the server publishes NO diagnostic. The user has clearly
written an integer in disguise; truncation is a no-op and the warning
would just be noise.

**Given** a user-defined function `void takeInt(int x) {}`
**When** called with an identifier of declared type `float`
(`takeInt(someFloat)`)
**Then** the server publishes a `DiagnosticSeverity::WARNING`. The value
is unknown at compile time, so it MAY have lost precision; warn.

**Given** widening (int → float), string-vs-int mismatch, etc.
**Then** behavior unchanged from prior shipped behavior. Widening is
silent (no precision loss), non-numeric mismatches remain hard errors.

## Tests

- `warns_on_narrowing_float_to_int_for_unrounded_literal`: `takeInt(3.14)` → WARNING
- `warns_on_narrowing_float_to_int_for_non_literal`: `takeInt(gSomeFloat)` (global) → WARNING
- `silent_for_rounded_float_literal`: `takeInt(1.0)` → no diagnostic
- `allows_int_to_float_widening_for_user_function`: `takeFloat(5)` → no diagnostic (regression guard)
- Integration roundtrip: `float_to_int_loss` fixture asserts the warning fires end-to-end
