# Delta for LSP Server Lifecycle

## ADDED Requirements

### Requirement: Single-Mutex Hold Across Await Points

Every async LSP handler that accesses server state MUST hold at most one mutex guard at a time. The handler SHALL release each guard before acquiring another, and no mutex guard SHALL be held across any `.await` point. This discipline MUST be enforced for every handler, not only the one that motivated the refactor.

#### Scenario: happy path — handler releases each guard before acquiring the next

- GIVEN an LSP handler needs to update `documents`, then `symbol_tables`, then `merged_views`
- WHEN the handler executes its async body
- THEN it SHALL hold no more than one mutex guard at any point between `.await` points
- AND each guard SHALL be released before the next mutex is acquired.

#### Scenario: adversarial case — regression test rejects multi-mutex holds

- GIVEN the suite includes a regression test that audits lock discipline
- WHEN a change causes any handler to hold two or more mutex guards across an `.await`
- THEN `cargo test` SHALL fail and name the handler and await point that violated the single-mutex rule.
