# Local patches

This is a vendored copy of nih-plug at commit
`de421011f41a6d10fc8c7a6084e4f4dee0143683`.

Changes from upstream:

- `src/wrapper/clap/wrapper.rs`: request a parameter value rescan after loading
  host state, so the host reflects the restored parameter values.
- `src/wrapper/clap/wrapper.rs`: reject state streams larger than 16 MiB before
  allocating, so malformed state cannot trigger an allocation failure.
- `src/lib.rs`: allow the vendored crate's own warnings so they do not fail the
  plugin build.
