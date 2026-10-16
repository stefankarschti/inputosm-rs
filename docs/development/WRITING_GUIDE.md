# Technical writing guide

Use ASD-STE100 Simplified Technical English, Issue 9, for project documents and code comments.
Follow [the project instructions](../../AGENTS.md).
Use [the official standard](https://www.asd-ste100.org/) for dictionary meanings and writing rules.
This guide does not replace that standard.

## Technical terms

| Term | Meaning |
|---|---|
| API | Application programming interface. |
| borrow | Access data without ownership. |
| callback | A function that the reader calls with decoded data. |
| crate | A Rust package compilation unit. |
| decode | Obtain values from their encoded representation. |
| delta | The difference between consecutive values. |
| entity | An OpenStreetMap node, way, or relation. |
| fixture | Test input with known contents. |
| PBF | Protocolbuffer Binary Format. |
| slice | A reference to consecutive elements. |
| string table | A list that maps string IDs to byte strings. |
| varint | An integer representation with a variable byte count. |
| view | A borrowed reference to reader data. |

Keep identifiers, paths, commands, and measured values exact.
Use one term for each concept.
Use active voice and simple verb forms.
Use `Examine` for review instructions.
Do not classify general words as technical terms to avoid dictionary rules.

## Review procedure

1. Read the complete changed document.
2. Compare technical claims with the current source.
3. Apply the writing rules and dictionary meanings.
4. Validate local links and executable examples.
5. Compare benchmark statements with raw measurements.
6. Record the scope and limitations in [the review record](REVIEW.md).

Automated checks detect some defects.
They do not prove full language compliance.
If the complete dictionary is unavailable, record that limitation.
