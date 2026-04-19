# Composit RFCs

Formal specifications for externally-visible contracts: file formats,
protocols, and discovery mechanisms. These are the surfaces that
third-party tools and other implementations target.

## Status meanings

- **Draft** — under active discussion; shape may change
- **Proposed** — stable enough for implementers to target; breaking
  changes require a new minor version
- **Accepted** — shipped in a tagged release; breaking changes require a
  major version bump of the affected artifact
- **Superseded** — replaced by a later RFC (linked in the header)

## Index

| #   | Title                                      | Status | Artifact                                          |
|-----|--------------------------------------------|--------|---------------------------------------------------|
| 001 | composit-report v0.1 format                | Draft  | `schemas/composit-report-v0.1.json`               |
| 002 | Provider manifest: public + contract tiers | Draft  | `schemas/composit-provider-manifest-v0.1.json`    |

## Process

1. Open a GitHub Discussion under the **RFC** category with the draft.
2. After review, land the RFC as a PR under `docs/rfcs/NNN-<slug>.md`
   together with any schema/fixture changes.
3. Update this index.
4. For breaking changes, bump the affected schema's version and preserve
   the previous file under its original name.
