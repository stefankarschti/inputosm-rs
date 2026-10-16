# Project instructions

These instructions apply to this repository and its subdirectories.

## Technical language

Use ASD-STE100 Simplified Technical English, Issue 9, for all technical documents and code comments.
This requirement is mandatory for new and changed text.
Apply the writing rules and approved dictionary.
Use approved words with their specified meanings and parts of speech.
Use technical nouns and technical verbs as permitted by ASD-STE100.
Use each term consistently.

- Use active voice and simple verb forms.
- Give one instruction in each procedural sentence.
- Use the imperative form for instructions.
- Use a maximum of 20 words in procedural sentences and 25 words in descriptive sentences.
- Put a necessary condition before its instruction.
- Keep each paragraph on one subject.
- Use no more than six sentences in a paragraph.
- Preserve technical meaning, code identifiers, commands, file paths, and literal values.
- Preserve recorded measurements and their units.
- Use American English spelling.

Follow the [writing guide](docs/development/WRITING_GUIDE.md).
The [official standard](https://www.asd-ste100.org/) remains the language reference.
The guide does not replace the standard or its dictionary.

## Mandatory implementation review

Before declaring an implementation task complete, obtain an independent adversarial review.
This requirement includes bug fixes, refactors, tests, build changes, and dependency changes.
Follow the [review procedure](docs/development/ADVERSARIAL_REVIEW.md).
Delegate the review to a separate agent with fresh context.
The reviewer must not change project files.

Provide the task requirements, complete task diff, relevant source paths, and validation results.
Ask the reviewer to find concrete failures against the requirements.
Do not require a minimum number of findings.

Verify each finding before changing code.
Add regression tests for confirmed behavioral defects.
Run the affected checks after corrections.
Obtain another review of the corrections and their interactions with the original change.
After three review cycles, report unresolved substantive findings before another cycle.
Do not declare completion while blocking findings remain unresolved.

If independent review is unavailable, disclose that limitation.
Include review coverage, finding dispositions, and validation results in the final report.

## Mandatory document review

Review every new or changed technical document before completion.
Follow the [document review procedure](docs/development/WRITING_GUIDE.md#review-procedure).
Compare API claims with source code.
Compare benchmark claims with recorded data.
Validate local links and executable examples.
Record the review scope and limitations in `docs/development/REVIEW.md`.

Do not treat sentence-length checks as proof of full ASD-STE100 compliance.

## Architecture and validation

Read [the architecture](docs/architecture/OVERVIEW.md) before changing component responsibilities.
When responsibilities or data flows change, update that document.
Keep technical documents in `docs/`, except for this file and the root `README.md`.
Keep the source structure close to the original inputosm project.
Document Rust API differences and dependency decisions.

Select project-owned files before running a formatter.
Do not format downloaded dependencies or toolchain sources.
Run affected tests after code changes.
Do not run planet benchmarks for documentation-only changes.
Do not modify or delete source maps during validation.
