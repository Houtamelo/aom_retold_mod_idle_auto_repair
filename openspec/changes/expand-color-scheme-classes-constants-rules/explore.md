# Exploration: Class Member Extraction (Bucket C Remainder)

## Executive Summary

The prior slice (`finish-semantic-token-distinctions`) added `SymbolKind::Class` and made class *names* emit semantic tokens, but explicitly skipped extracting class body members. This exploration covers the remaining work: walking each `class_specifier` body to extract fields and methods as separate symbols, surfacing them in the document outline, and extending semantic tokens so member references (`obj.field`, `obj.method()`) can be colored distinctly. Based on the grammar and current LSP architecture, the change is straightforward but crosses the LSP, cache schema, and plugin color scheme. It will likely exceed the default 400-line review budget unless scoped tightly, so the proposal phase should decide between one larger slice or a LSP-first/plugin-coloring split.

## XS Class Member Syntax

All line references are to `tools/xs-language-server/tree-sitter-xs/grammar.js`.

| Construct | Grammar Rule | Example |
|-----------|--------------|---------|
| Class declaration | `class_specifier` (lines 213-217) | `class BaseDockInfo { ... }` |
| Class body | `field_declaration_list` (lines 326-330) | `{ ... }` |
| Field | `field_declaration` + `_field_declarator` = `identifier` (lines 332-347) | `int mBaseID = -1;` |
| Method | `field_declaration` + `_field_declarator` = `function_declarator` with optional `compound_statement` (lines 332-347) | `void doAnalysis(int baseID = -1) { ... }` |
| Member access | `field_expression` (lines 632-638) | `obj.field`, `obj.method()` |
| Constructor/new | `new_expression` (lines 624-628) | `new int(gMilitaryBuildings.size(), -1)` |
| Array builtins | parsed as `field_expression` too | `arr.size()`, `arr.add(x)`, `arr.clear()` |

Notes from the grammar:

- `field_expression` uses `.` or `->` and has an `argument` (`expression`) plus `field` (`field_identifier`).
- `field_identifier` is an alias of `identifier` (line 716) but has node kind `field_identifier`, so the current token walker does **not** match it.
- There is no explicit `this` keyword in the grammar; class members are referenced directly (`mBaseID = baseID`) or via `obj.field`.
- No access-modifier syntax exists for class members; everything is effectively public.

## Vanilla Game Examples

From `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/buildings/buildings.xs` (read-only):

```xs
class BaseDockInfo
{
   int mBaseID = -1;
   vector mBaseLocation = cInvalidVector;
   bool mValidForDock = false;

   void displayInfo()
   {
      debugMilitaryBuildings("   mBaseID: " + mBaseID);
   }

   void doAnalysis(int baseID = -1)
   {
      mBaseID = baseID;
      mBaseLocation = kbBaseGetLocation(cMyID, baseID);
      if (gMapInfo.mHasFish == true) { ... }
   }
}

class DockManager
{
   BaseDockInfo[] mBaseDockInfos = default;
   int mCurrentBaseID = -1;

   void resetCurrentInfo()
   {
      mCurrentBaseID = -1;
      mShorelineIDs.clear();
      mDockBuildPlans.clear();
      if (aiPlanGetIsIDValid(mScoutPlanID) == true) { ... }
   }

   int updateBaseArrays()
   {
      BaseDockInfo newInfo;
      newInfo.doAnalysis(baseID);
      mBaseDockInfos.add(newInfo);
      return cCheckForBetterBaseID;
   }
}
```

Observations relevant to implementation:

- Methods inside `BaseDockInfo` reference local fields bare (`mBaseID`) while external callers use `newInfo.doAnalysis(...)`.
- `mBaseDockInfos.add(newInfo)` and similar `.size()`, `.removeIndex(i)` are array built-in calls indistinguishable syntactically from class method calls.
- Arrays can be initialized with `default` as well as `new elementType(size, fill)`.

## Current LSP State

- `tools/xs-language-server/src/symbols.rs:24-31` — `SymbolKind` has `Rule`, `Function`, `Variable`, `Constant`, `Class`. Class members are not represented.
- `tools/xs-language-server/src/symbols.rs:82-114` — `Symbol` has no `class_owner` field.
- `tools/xs-language-server/src/symbols.rs:307-330` — `extract_class` only records the class name; it does **not** walk the `field_declaration_list` body.
- `tools/xs-language-server/src/semantic_tokens.rs:19-34` — Legend has token types `function`, `variable`, `type`, `constant`, `rule` and modifiers `engine`, `modded`, `unmodded`, `local`, `static`, `extern`. No `member` modifier.
- `tools/xs-language-server/src/semantic_tokens.rs:110-148` — Walker handles `identifier`, `_type_identifier`/`type_identifier`, and `primitive_type` only. `field_expression`/`field_identifier` is ignored.
- `tools/xs-language-server/src/merged_view.rs:622-627` — `include_visible` treats `Class` as non-local. Class members should be `Visibility::Local` so they are not incorrectly merged across files, since XS members are scoped to the class and referenced via `.` or bare within the class body.
- `tools/xs-language-server/src/cache.rs:8-14` — Parse cache currently lives under `game_parse/v2/`. Adding a non-optional `class_owner` field would break deserialization unless defaulted; more importantly old cached tables will silently lack member symbols.
- `tools/xs-language-server/src/typecheck.rs:317-330` — Already recognizes `field_expression` for call callee extraction, but does no type-based member resolution. This confirms the implementation can reuse the AST shape.

## Required LSP Changes

1. **Extend `SymbolKind`** (`symbols.rs:24-31`)
   - Add `ClassField` and `ClassMethod` variants.
   - Update `label()`.
   - ~5 LOC.

2. **Extend `Symbol` struct** (`symbols.rs:82-114`)
   - Add `#[serde(default)] pub class_owner: Option<String>`.
   - ~2 LOC.

3. **Walk class bodies in symbol extraction** (`symbols.rs:307-330`)
   - Modify `extract_class` to iterate `field_declaration_list` children.
   - Add helper `extract_class_member`:
     - Detect method vs field from `_field_declarator` (`function_declarator` vs `identifier`).
     - Extract type from `_declaration_specifiers`.
     - Extract parameter list for methods.
     - Handle `default`, `=` initializer, array types.
     - Push `ClassField`/`ClassMethod` with `class_owner = Some(class_name)` and `Visibility::Local`.
   - ~90-130 LOC.

4. **Update symbol-kind consumers**
   - `semantic_tokens.rs:225-231` — map `ClassField` → `VARIABLE`, `ClassMethod` → `FUNCTION`.
   - `merged_view.rs:622-627` — keep members hidden from cross-file includes (Local).
   - `completion.rs:115` and `server.rs:1321,1343` — add exhaustive mappings for the new kinds.
   - ~30 LOC combined.

5. **Add `member` semantic-token modifier and field-expression handling** (`semantic_tokens.rs`)
   - Add `"member"` to `TOKEN_MODIFIERS`.
   - In the walker, handle `field_expression`: classify its `field_identifier` child.
   - Add a member classifier that:
     - Looks for any class in the workspace/mod/game with a matching member name.
     - Emits `FUNCTION` + `member` for methods or `VARIABLE` + `member` for fields, plus the owner's origin modifier.
     - Falls back to no token for array builtins (`.size()` etc.) unless they are also declared as class members (they won't be).
   - Detect member declarations by checking if the identifier's symbol is `ClassField`/`ClassMethod` and append `member` modifier.
   - ~80-120 LOC.

6. **Bump per-file parse cache schema** (`cache.rs`)
   - Move `game_parse/v2/` → `game_parse/v3/` so old cached symbol tables are rebuilt with members.
   - Update doc comments and `parse_cache_dir`.
   - ~10 LOC.

7. **LSP tests**
   - `tests/class_extraction_repro.rs` — add field/method extraction tests.
   - `tests/semantic_tokens_repro.rs` — add member modifier in legend and member reference token tests.
   - `tests/semantic_token_distinctions_repro.rs` — add modded/unmodded member tests.
   - ~200-260 LOC combined.

## Required Plugin Changes

1. **New text-attribute keys** (`XsTextAttributes.kt`)
   - Add member-specific keys, e.g.:
     - `FUNCTION_MEMBER_ENGINE`, `FUNCTION_MEMBER_MODDED`, `FUNCTION_MEMBER_UNMODDED`
     - `VARIABLE_MEMBER_ENGINE`, `VARIABLE_MEMBER_MODDED`, `VARIABLE_MEMBER_UNMODDED`
   - Inherit from `FUNCTION_DECLARATION` / `INSTANCE_FIELD` or `LOCAL_VARIABLE` / `GLOBAL_VARIABLE` as appropriate.
   - ~10-12 keys plus comments; ~25 LOC.

2. **Register descriptors** (`XsColorSettingsPage.kt:45-80`)
   - Add new entries under `Identifier//Member//...` paths.
   - ~12-15 LOC.

3. **Converter updates** (`XsSemanticTokensConverter.kt`)
   - Add `MODIFIER_MEMBER` constant.
   - In `convert`, when `member` is present:
     - `function` + engine/modded/unmodded → new member-function keys.
     - `variable` + engine/modded/unmodded → new member-field keys.
   - Also add `member` modifier to `XsSemanticTokensSupport.kt` modifier list.
   - ~40-50 LOC.

4. **Plugin tests**
   - `XsSemanticTokensConverterTest.kt` — add members-for-each-origin cases.
   - ~40-60 LOC.

## Cache Schema Impact

Adding `class_owner: Option<String>` could technically be made backward-compatible with `#[serde(default)]`, but the bigger issue is semantic: existing `game_parse/v2/` cached `SymbolTable`s simply will not contain class member symbols. A user who already has a warm cache would silently lose member features until every touched file is re-parsed. The clean fix is a schema bump to `game_parse/v3/` so all parse caches are invalidated once and rebuilt with members. This has a one-time startup cost (re-parsing the 300+ vanilla `.xs` files), after which warm starts behave as before.

## Out of Scope

The following items are intentionally excluded from this slice:

- Constructors / dedicated `new` expression analysis (record members only, not invocation targets).
- Class inheritance, polymorphism, or `virtual` methods.
- Access modifiers (XS does not have them).
- Static class members (no `static` syntax inside classes in vanilla code).
- Type inference for local variables, which would be required to reliably color `obj.method()` by the dynamic type of `obj`.
- Cross-file member lookup via `include` paste (members will be `Visibility::Local`).
- Coloring array built-in method calls (`.size()`, `.add()`, `.clear()`); they will continue to receive no semantic token unless explicitly classified elsewhere.

## Slice Recommendation

**Recommended: 1 slice, but with explicit review-budget planning.**

All changes are coupled around a single semantic concept: class members must be extracted before they can be colored, and the LSP legend must gain the `member` modifier before the plugin can map it. Splitting cleanly is possible but artificial:

- Option A — One slice (preferred): LSPD extraction + semantic tokens + plugin colors + cache bump. Estimated 600-750 changed lines. Exceeds default 400-line review budget; delivery strategy would need `exception-ok` or a chained-PR split.
- Option B — Two slices:
  1. LSP member extraction, cache v3 bump, and LSP tests (plugin colors members as normal function/variable for now). ~350-400 LOC.
  2. Plugin member-specific colors and converter/legend updates. ~200-250 LOC.

If reviewer load is the overriding concern, Option B is safer; otherwise Option A keeps the feature end-to-end in a single merge. The proposal phase should make the final call.

## pluginVersion Impact

**Minor bump: 0.7.0 → 0.8.0.**

Per the project policy, a new LSP feature plus new color-scheme categories falls under a minor version bump. The bundled LSP binary changes, and pluginSettings UI gains new descriptor groups, so PATCH is insufficient.

## Risks

1. **Member access without type inference.** The LSP cannot reliably know the class of `obj` in `obj.method()`, so member-reference coloring will be heuristic (matching the member name against any class in the project). This can mis-color calls when two classes define the same member name or when `.size()`/`.add()` array builtins are used.

2. **Ambiguity between class methods and global functions.** A bare call `resetCurrentInfo()` is syntactically a global function call even if a class method with the same name exists. Without type inference we cannot tell, so member coloring may leak into global calls or be omitted entirely.

3. **Cache schema bump cost.** Moving from `game_parse/v2/` to `v3/` invalidates every cached parse result. The next game-folder integration test and the first user launch after upgrade will re-parse ~300 files. This is one-off and bounded, but it temporarily raises startup latency and must be verified.

4. **Grammar edge cases.** `field_declaration` declares both fields and methods with overlapping node shapes. Methods with no body or malformed headers may surface as `ERROR` nodes inside the class body and must not crash extraction.

5. **Review budget overrun.** The combined LSP + plugin + tests change is expected to land around 600-750 LOC, which exceeds the default 400-line guarded budget. The proposal phase must explicitly choose `exception-ok`, plan a chained PR, or reduce scope.

## Open Questions for the Proposal Phase

1. Should member references use a new `member` **modifier** (paired with `function`/`variable` token types) or a dedicated `member` **token type**? A modifier preserves token-type orthogonality and matches what the plugin already does with `modded`/`unmodded`.

2. How should ambiguous member names be colored? For example, if both a modded class and an unmodded class define a member named `reset()`, should the LSP default to unmodded, prefer the class of the same-file local variable, or skip member coloring for ambiguous names?

3. Should class member symbols be included in document-symbol/workspace-symbol results, or only used for internal coloring and outline? Adding them to the symbol outline is natural, but exposing every private field in a workspace-wide symbol search may be noisy.
