# Rustdoc Style Guide

This document defines the project-wide Rustdoc conventions for both public and
private APIs. It is the single source of truth for documentation style.

## Goals

1. Keep documentation consistent across modules and visibility levels.
2. Make behavior, inputs, outputs, and error cases obvious at the call site.
3. Match existing in-repo Rustdoc style (see `Cli::parse_args` and `Cmd` methods).

## Attribute placement

Place attributes after the Rustdoc block and before the documented item.

Example:
```rust
/**
 * Creates a new command builder.
 *
 * # Arguments
 * - `program`: Program to execute.
 *
 * # Returns
 * Constructed `Cmd` with program name set.
 */
#[must_use]
pub fn new(program: impl Into<OsString>) -> Self {
    // ...
}
```

## Function doc format

Use doxygen-style multiline comments for all functions (public and private).
Single-line `///` comments are reserved for short non-function items such as
struct fields or enum variants.

Required structure for function docs:
1. One-line summary sentence.
2. Optional short paragraph for behavior details or invariants.
3. Section blocks as needed:
   - `# Arguments`
   - `# Returns`
   - `# Errors`
   - `# Panics`
   - `# Examples`

## Public and private API requirements

Public and private functions must be documented to the same depth. At minimum:
1. `# Arguments` when the function accepts parameters.
2. `# Returns` when the function returns a non-`()` value.
3. `# Errors` when returning `Result` or when error conditions are possible.
4. `# Panics` when panics are possible or intentionally used.

If a section is not applicable, omit it rather than adding a placeholder.

## Example (full format)

```rust
/**
 * Runs the configure phase (`cmake`) for the resolved configuration.
 *
 * # Arguments
 * - `runner`: Command runner configuration.
 * - `rc`: Resolved configuration for the active profile.
 * - `extra`: Extra arguments to append to the `cmake` invocation.
 *
 * # Returns
 * `Ok(())` on success.
 *
 * # Errors
 * Returns `KazeError` if the build directory cannot be created or if the
 * configure command fails.
 */
pub fn cmd_conf(runner: Runner, rc: &ResolvedConfig, extra: &[String]) -> Result<(), KazeError> {
    // ...
}
```

