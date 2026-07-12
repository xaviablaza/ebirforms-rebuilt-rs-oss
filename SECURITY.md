# Security Policy

## Reporting vulnerabilities

Please report security issues privately to the project maintainers. Do not file public issues for vulnerabilities involving credentials, taxpayer data, live submission behavior, or receipt parsing bypasses.

## Sensitive data policy

Do not commit:

- taxpayer PII or real filing payloads;
- live credentials, tokens, keys, cookies, or app passwords;
- private fixtures, production artifacts, or official BIR package-derived materials;
- production endpoint research that is not already public documentation;

The repository keeps `.ebirforms/`, `.env*`, and `fixtures/private/` ignored.

## Supported versions

This project is pre-1.0. Treat all APIs as experimental.
