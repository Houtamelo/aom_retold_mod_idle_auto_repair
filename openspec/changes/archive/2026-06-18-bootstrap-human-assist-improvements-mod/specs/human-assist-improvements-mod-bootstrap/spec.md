# Mod Bootstrap: Human Assist Improvements

## Purpose

Specify the bootstrap of the fourth bundled mod, `Human Assist Improvements`, which bundles the existing Auto-Repair and Intelligent Auto-Scout features in a single `human_assist.xs` overlay. It becomes the canonical home for future human-assist features.

## Requirements

### Requirement: Mod structure

A fourth mod MUST exist at `mod/human_assist_improvements/game/ai/human_assist/`. Its `human_assist.xs` MUST be the vanilla player overlay plus the `auto_repair.xs` include, the `auto_scout.xs` include, and the `autoScout_register(planID, unitID)` call inside `enableAutoScouting`, matching the combined mod exactly. It MUST NOT introduce new gameplay logic beyond the combined mod.

#### Scenario: First load of the bundled mod (MANUAL)

- GIVEN the repository is checked out and the game is not running
- WHEN `scripts/deploy-mods.sh` is run and AoM:R is launched with only `Human Assist Improvements` enabled
- THEN the mod appears in the local mods list, loads without syntax errors, and both Auto-Repair and Auto-Scout work for a human player

### Requirement: Feature-file reuse

The fourth mod SHALL NOT contain private copies of `auto_repair.xs` or `auto_scout.xs`. The deploy script MUST copy those files from the sibling source directories so that the shared source of truth is preserved.

#### Scenario: Shared source remains authoritative

- GIVEN a change is made to `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs`
- WHEN the fourth mod is deployed
- THEN the deployed `Human Assist Improvements/game/ai/human_assist/auto_repair.xs` MUST be byte-identical to the updated standalone repair source

### Requirement: Deploy script extension

`scripts/deploy-mods.sh` MUST copy both feature files into the fourth mod's tree and MUST NOT alter the existing three mods' deploy blocks.

#### Scenario: Existing mods remain unmodified (MANUAL)

- GIVEN the current deploy script and source tree
- WHEN the fourth mod block is added
- THEN the first three mods deploy exactly the same files to the same destinations
- AND a dry-run lists the fourth mod's files without errors

### Requirement: Conflict and compatibility

The fourth mod MUST document in its own README and in the top-level `README.md` that it conflicts with the three existing mods because all four overlay `game/ai/human_assist/human_assist.xs`. Players MUST NOT load it alongside them.

#### Scenario: Conflict warning is visible

- GIVEN a player reads either `mod/human_assist_improvements/README.md` or `README.md`
- WHEN they look for compatibility guidance
- THEN a clear warning states `Human Assist Improvements` must not be enabled alongside `Idle Auto-Repair`, `Intelligent Auto-Scout`, or `Intelligent Auto-Repair and Scout`

### Requirement: Placeholder thumbnail

A placeholder thumbnail asset MUST exist for the fourth mod, copied from the combined-mod thumbnail.

#### Scenario: Thumbnail present

- GIVEN the mod is prepared for upload
- WHEN a packaging workflow inspects the root-level thumbnail
- THEN `thumbnail_human-assist-improvements.png` exists and is byte-identical to the combined-mod thumbnail

### Requirement: Human-only behavior parity

The fourth mod's behavior SHALL equal the combined mod's bootstrap behavior: both the repair and scout features MUST be active for human players only and MUST do nothing for AI players.

#### Scenario: Feature parity in-game (MANUAL)

- GIVEN a human player owns an idle villager and an `AbstractScout`
- WHEN the villager becomes eligible for repair and the Auto-Scout button is toggled
- THEN `aiEcho` log entries from `auto_repair.xs` and `auto_scout.xs` appear, confirming both state machines ran

### Requirement: Patch-maintenance and rollback

This specification MUST call out that `human_assist.xs` is a vanilla overlay and will need manual rebasing after any game patch touching the original file. Rollback SHALL be clean: delete `mod/human_assist_improvements/`, revert the new deploy block, and revert all README additions.

#### Scenario: Rollback (MANUAL)

- GIVEN the change has been applied and the game is not running
- WHEN `mod/human_assist_improvements/`, the fourth deploy block, and README additions are reverted
- THEN the existing three mods deploy and behave exactly as before the change

## Out of scope

- New gameplay feature logic (deferred to a future change).
- Private copies of `auto_repair.xs` or `auto_scout.xs` inside `mod/human_assist_improvements/`.
- AoM:R platform manifest or packaging metadata beyond the placeholder thumbnail.
