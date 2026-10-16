# Adversarial implementation review

## Purpose and scope

An adversarial review searches for concrete ways that a change can violate its requirements.
The reviewer examines the implementation independently from its author.
Passing tests supply evidence but do not replace this review.
The root [project instructions](../../AGENTS.md) require this procedure for every implementation task.

The requirement covers code, regression fixes, refactors, tests, examples, build configuration, and dependency changes.
A documentation-only task uses the [documentation validation procedure](WRITING_GUIDE.md#review-procedure).
Documentation that changes executable examples or behavior requirements also needs an implementation review.
Small tasks need a focused review; they do not need an arbitrary number of findings.

## Prepare the review

1. Finish the implementation and its required tests.
2. Identify the task requirements and compatibility constraints.
3. Identify the reviewed Git revision and any uncommitted task changes.
4. Include new files and changed tests in the complete task diff.
5. Separate unrelated workspace changes from the review scope.
6. Provide relevant source paths, interface documents, test results, and known limitations.
7. Start a separate reviewer agent without the implementation conversation history.

The reviewer needs access to surrounding code, callers, and existing tests.
The task requirements include accepted user decisions and relevant repository instructions.
The author's implementation reasoning can bias the review.
Do not include that reasoning in the initial review request.

The implementing agent coordinates the review and applies corrections.
The reviewer reports findings without changing project files or starting further reviewer agents.
The reviewer can run focused reproductions with isolated temporary fixtures.
Review work does not authorize production mutations or large dataset operations.

## Reviewer instructions

Use this request with the task requirements and review inputs:

```text
Independently review this implementation for concrete defects.
Search for cases where the implementation violates the supplied requirements.
Inspect the complete task diff and the relevant surrounding code.
Examine error paths, resource limits, compatibility, and test coverage.
Examine new helpers and changes introduced by earlier corrections.
Do not change project files.
Use isolated temporary fixtures for any reproductions.
Report actionable findings with locations, triggers, consequences, and evidence.
Distinguish reproduced failures from source-supported findings and unverified suspicions.
Do not invent findings to reach a quota.
If you find no actionable issues, state that result and the review limits.
```

The following areas guide the review when they apply to the task.

| Area | Checks |
|---|---|
| Requirements | Requested behavior, unchanged contracts, unsupported assumptions, and unrelated changes |
| Storage correctness | Publication order, durability, locks, checkpoints, rollback, cancellation, and cleanup ownership |
| Failure handling | Missing or malformed inputs, partial writes, dependency failures, retained reports, and callback behavior |
| Resource use | Memory ownership, temporary-file lifetime, repeated scans, system calls, queue limits, and thread utilization |
| Compatibility | Public interfaces, file formats, optional components, platform assumptions, and dependency versions |
| Test quality | Failure-path coverage, boundary cases, weakened assertions, and mocks that conceal the behavior under test |

## Verify findings and correct defects

For each finding, record its severity, file location, trigger, consequence, and supporting evidence.
Use one of these evidence labels:

| Label | Meaning |
|---|---|
| Reproduced | A command or regression fixture demonstrates the failure |
| Source-supported | The source establishes a failing path, without a reproduction |
| Suspected | The proposed failure still depends on an unverified assumption |

Verify the finding against the requirements and current source before changing code.
Where practical, reproduce the failure with a focused test.
Do not change behavior solely because a reviewer expressed confidence.

Record one disposition for each finding:

| Disposition | Required record |
|---|---|
| Confirmed defect | Correction and validation evidence |
| Accepted tradeoff | Requirement or user decision that permits the limitation |
| Rejected finding | Evidence that the claimed failure does not occur under the contract |
| Unresolved | Missing evidence, remaining impact, and next action |

Correct confirmed defects within the task scope.
Add a regression test for each confirmed behavioral defect.
Where practical, verify that the test fails before the correction.
Run the affected tests after each correction group.
Record any reproduction or test limitation.

## Review corrections and finish

A review cycle contains an independent review, finding verification, and any resulting corrections and tests.
After corrections, obtain another independent review of the updated task diff.
Provide the previous findings and their evidence with that request.
Examine both the original failure and possible failures introduced by its correction.
A new review must examine the current implementation, not only the previous finding list.

Unresolved correctness, data-loss, security, and material performance defects block completion.
A confirmed violation of the task requirements also blocks completion.
An accepted tradeoff must remain consistent with the user's requirements.
A lower severity label alone does not resolve a finding.

After three cycles, report unresolved substantive findings before another cycle.
The report must identify any required user decision or missing evidence.
The cycle count does not permit acceptance of unresolved blocking defects.
If further work needs no user decision, continue within the authorized task scope.

If a separate reviewer is unavailable, report that the independent review did not run.
Do not describe an author review as independent verification.
Do not claim full completion of this procedure without an independent review.

Before declaring completion, confirm that the reviewed changes match the final task changes.
For later implementation changes, repeat the affected review and validation.
Include these details in the final task report:

- Reviewed scope and independent reviewer.
- Review cycles and finding dispositions.
- Remaining limitations or accepted tradeoffs.
- Regression tests and other validation results.

This procedure guides agent behavior; it does not install a continuous integration check or a repository merge restriction.
