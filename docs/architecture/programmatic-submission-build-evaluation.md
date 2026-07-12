# Programmatic Submission Build Evaluation (OSS Sanitized)

This evaluates the OSS-clean build against the sanitized plan. The build uses synthetic fixtures only and does not include official BIR package materials, extracted application assets, non-public compatibility artifacts, production endpoints, credentials, or private taxpayer submissions.

## Implemented

- Synthetic 1601C template rendering from JSON.
- Deterministic payload packaging and encryption/decryption round trips.
- Safe-by-default CLI submission gate.
- Durable submission records.
- SQLite job queue and local IPC server.
- Profile/settings/PIN app-state primitives.
- Receipt parsing, matching, and local directory polling against synthetic receipts.

## Known limits

- Only synthetic 1601C assets are included.
- This OSS repo does not include private XML captures or official BIR package-derived artifacts.
- This OSS repo does not include production endpoint details or credentials.
- Additional forms require independently authored, redistributable templates and mappings.
