# Provenance and Non-Use Statement

This repository is an independent, unofficial tax-form software prototype. It is not affiliated with, endorsed by, sponsored by, or certified by the Philippine Bureau of Internal Revenue (BIR) or any other government agency.

## What this repository does not contain

This repository does **not** include, copy, decompile, unpack, modify, patch, automate, redistribute, or depend on the BIR Offline eBIRForms Package or any BIR-distributed executable package.

In particular, the public repository is intended to exclude:

- official eBIRForms installers, executables, DLLs, archives, or package resources;
- extracted source code, binary resources, images, fonts, scripts, database files, or other assets from the Offline eBIRForms Package;
- private taxpayer data, real return payloads, credentials, session tokens, or production endpoint research;
- non-public compatibility research or private implementation notes.

If official BIR software is needed, users must obtain it directly from BIR or another authorized source. Nothing in this repository grants rights to copy, modify, redistribute, or create derivative works from BIR software or BIR package contents.

## What this repository is

The code is a clean-room, data-driven prototype for rendering, packaging, queueing, and locally tracking taxpayer-authorized return data. Public fixtures are synthetic. Templates, mappings, and UI layouts are authored for interoperability testing and demonstration from independently redistributable inputs such as public form requirements, synthetic examples, and operator-provided data.

The project is designed to operate as third-party software that generates its own return artifacts. It does not tamper with, alter, hook into, or automate the BIR Offline eBIRForms Package.

## Compatibility identifiers

Some XML element names, field labels, or filing terms may resemble identifiers used in taxpayer filing artifacts because they are compatibility identifiers needed to produce or test structured return data. Their presence should not be read as evidence that this repository includes copied package code or bundled package assets. Compatibility identifiers should be validated independently against public requirements and authorized filing workflows before any production use.

## Live filing and credentials

Live submission is intentionally gated. The app defaults to dry-run behavior unless an authorized operator or distributor supplies explicit live-filing configuration and confirms live mode. Production credentials and endpoint values must be provided outside the public repository, for example by a private CI/distribution environment or local operator configuration.

## Compliance posture

This document is a provenance and hygiene statement, not legal advice. Before public distribution or production filing use, review the repository history and release artifacts for secrets, private data, official BIR package materials, and wording that could imply affiliation, certification, redistribution, or derivative use of the BIR Offline eBIRForms Package.
