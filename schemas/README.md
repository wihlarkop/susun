# Susun JSON Schemas

This package contains versioned JSON Schema artifacts for public machine-readable
Susun outputs. Schema IDs include the artifact name and major/minor schema
version. Additive fields should use a minor version bump; incompatible shape
changes require a major version bump.

Every schema must declare `x-susun-secret-policy`. Schemas may describe Compose
secret resources, but they must not define cleartext credential fields such as
`password`, `token`, `credential`, or `secret_value`.

`project-summary.schema.json` describes the SDK and CLI summary artifact used
by downstream applications such as desktop integrations.
