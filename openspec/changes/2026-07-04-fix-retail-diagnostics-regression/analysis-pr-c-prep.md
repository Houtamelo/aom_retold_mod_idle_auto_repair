# Pre-PR-C analysis: remaining retail ERROR diagnostics

This run parses `game/**/*.xs` **excluding `random_maps/`** as requested.

## Summary

- Total files parsed: 234
- Files with diagnostics: 157
- Total diagnostics: 3668
- Root top-level resyncs: 471
- Cascade `invalid syntax` errors: 3162
- Direct (non-cascade) errors: 506

Note: parsing the full `game/` tree **including** `random_maps/` reproduces the reported 4,782 / 587 baseline; omitting `random_maps/` drops the totals to the 3668 / 471 shown above.

## Top constructs causing top-level parser resyncs

| Construct fingerprint | Count | Example source line | Likely root cause |
|-----------------------|------:|---------------------|-------------------|
| `` Class-typed local var declaration `ClassType name;` `` | 219 | `}` | class-typed local declarations without init; also triggered when workspace class names are not registered in TypeTable |
| `` Class-typed local var init `ClassType name = ...` `` | 160 | `}` | class-typed local variable initialization at file/function scope |
| `` } `` | 71 | `}` | parser resync point after an earlier root failure (no actionable construct within 5-line window) |
| `` Lambda expression `[...](...) {}` in assignment/default `` | 9 | `void boVillager(int villagerUnitType = -1, int villagerResourceType = -1, void(int) afterQueue = [](int id = -1) {})` | lambda literals are not modeled in the XS grammar |
| `` }; `` | 5 | `};` | parser resync point after an earlier root failure (no actionable construct within 5-line window) |
| `` FunctionPointer-typed variable or parameter default `type(...) name = ...` `` | 4 | `kbAttackRouteAddPath(routeID, pathID2); **/` | function-pointer types/defaults are not supported in local declarations or parameters |
| `` C-style block comment `/* ... */` `` | 3 | `/***** Increase army sizes on harder difficulties deep into the mission. *****/` | block comments are not skipped by the current lexer (only `//` line comments) |

## Cascade analysis

- 471 root resyncs × ~6.71 cascade ratio = 3162 cascade errors
- Direct (non-cascade) errors: 506

## Recommended PR-C scope

Target the largest resync clusters in this order:

1. `` Class-typed local var declaration `ClassType name;` `` (219 root resyncs)
2. `` Class-typed local var init `ClassType name = ...` `` (160 root resyncs)
3. `` Lambda expression `[...](...) {}` in assignment/default `` (9 root resyncs)
4. `` FunctionPointer-typed variable or parameter default `type(...) name = ...` `` (4 root resyncs)
5. `` C-style block comment `/* ... */` `` (3 root resyncs)

## Estimated diagnostic reduction

Fixing the above 395 root resyncs should remove approximately **3047 diagnostics** (roots + associated cascades), leaving roughly **621** errors (excluding random_maps).
