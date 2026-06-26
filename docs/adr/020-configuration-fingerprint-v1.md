# ADR 020: Configuration Fingerprint v1

## Status

Accepted for Phase 4 implementation.

## Context

Phase 4 needs repeated `up` operations to distinguish unchanged containers from containers whose runtime-relevant service configuration changed. The fingerprint must be deterministic, versioned, daemon-independent, and safe to store in runtime labels.

## Decision

Susun v1 configuration fingerprints use this label format:

```text
susun-fp-v1:sha256:<lowercase-hex-sha256>
```

The digest input is a length-prefixed UTF-8 canonical byte stream. Each atom is encoded as:

```text
<key-byte-len>:<key>=<value-byte-len>:<value>\n
```

The v1 input includes:

- fingerprint schema version;
- selected image reference plus resolved digest or image ID when available;
- command and entrypoint;
- environment keys and redacted value digests;
- container labels;
- ports;
- volume target semantics;
- network attachments, aliases, and resolved runtime names;
- config and secret identities plus mount targets;
- healthcheck;
- effective restart policy;
- runtime defaults that affect container configuration.

The v1 input excludes source spans, diagnostics, timestamps, random engine IDs, raw secret values, CLI display options, profiles, and planner presentation metadata.

Environment values are represented by SHA-256 digests, not plaintext. Secret mounts include only resource identity, resolved runtime name, and target metadata; secret contents are never serialized, logged, or included in debug output.

Unknown observed fingerprint versions are treated conservatively by convergence. They produce a fingerprint invariant error until a later migration window adds compatibility handling.

## Consequences

Changing any documented runtime-relevant field changes the fingerprint. Formatting-only changes and metadata-only changes do not. Future fingerprint schemas can coexist by adding new `susun-fp-vN` parsers while retaining v1 parsing for at least one migration window.
