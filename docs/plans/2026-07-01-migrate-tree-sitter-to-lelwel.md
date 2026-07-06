# Migrate XS LSP Parser from tree-sitter to lelwel

**Status:** Planning only — do not start implementation without explicit approval.
**Strategy:** Big-bang cutover (single coordinated change, no intermediate shipping).
**Target outcome:** `tree-sitter-xs/` removed; `lelwel-xs/` becomes the parser of record; all LSP consumers walk a typed AST instead of an untyped CST; the **first-class forward-declaration** win is realized.

---

## 1. Goal

Replace the tree-sitter-generated XS parser (`tree-sitter-xs/grammar.js` → 818 lines DSL → 23,660 lines generated C → wrapped by `tree-sitter 0.26` Rust bindings) with the lelwel-generated parser (`lelwel-xs/src/xs.llw` → Logos lexer + typed Rust AST).

The **primary motivation** is a single behavioural win: tree-sitter cannot distinguish `void bar(int x = -1);` (forward declaration with parameter default values) from `void bar(int x = -1) { ... }` (function definition) because the grammar's `function_declarator` rule conflates them. Tree-sitter produces an `ERROR` node for the forward declaration; the LSP then walks ERROR-node subtrees via two fragile helpers (`extract_error_function_definition` at `symbols.rs:557` and `extract_error_forward_declaration` at `symbols.rs:606`) to reconstruct what should have been a first-class node.

Lelwel's `function_or_forward_declaration` rule factors out the common prefix and disambiguates by the next token (`{` vs `;`). After migration, both helpers become dead code and the regression-test that locks in the ERROR-recovery path (`tests/symbols_cleanup_repro.rs` R1-F-02) is inverted.

---

## 2. Architecture (target state)

### Before (tree-sitter era)

```
.xs file
   ↓
tree-sitter Parser + tree-sitter-xs (C parser compiled via cc)
   ↓
untyped CST (Tree<Node>) — 441 node types, 310 named types, 17 explicit conflicts
   ↓
string-keyed navigation: node.kind() == "function_definition", node.child_by_field_name("name")
   ↓
ERROR sentinel nodes for forward declarations → manually walked in symbols.rs:557-649
   ↓
SymbolTable, SemanticTokens, Diagnostics
```

### After (lelwel era)

```
.xs file
   ↓
lelwel::Parser + Logos lexer (built from xs.llw via lelwel build script)
   ↓
typed AST — each rule generates a Rust enum (e.g. TranslationUnit, FunctionDefinition, ForwardDeclaration, ClassSpecifier)
   ↓
typed navigation: match translation_unit { FunctionDefinition(fd) => ..., ForwardDeclaration(fd) => ... }
   ↓
no ERROR nodes; forward declarations are first-class
   ↓
codespan-reporting diagnostics for parse errors (no sentinel-node filtering)
   ↓
SymbolTable, SemanticTokens, Diagnostics
```

### Key shifts

| Concern | tree-sitter | lelwel |
|---|---|---|
| Tree representation | `Tree<Node<'a>>` with `kind()` strings | Typed Rust enums (one per rule) |
| Field access | `node.child_by_field_name("name")` returning `Option<Node>` | `.name` field directly typed |
| Whitespace | Stripped automatically | Lossless (must skip explicitly) |
| Comments | Stripped automatically | Lossless (must skip explicitly) |
| Parse errors | ERROR/MISSING sentinel nodes + recovery | Codespan-reporting spans; no sentinel |
| Ambiguity (C typedef) | `conflicts` array in grammar.js | Ordered choice + `?t` predicates + optional ParserCallbacks |
| Forward declarations | ERROR node, walked by helpers | First-class `ForwardDeclaration` node |
| Build artifact | 23,660-line `parser.c` compiled via `cc` | 212-line Logos lexer + 21-line parser entry |
| Lexer regexes | Hidden inside generated parser.c | Logos attributes in generated `lexer.rs` skeleton — **must be hand-filled** |

---

## 3. PoC Limitations — Per-Feature Analysis

The `lelwel-xs/src/xs.llw` PoC drops 7 features vs tree-sitter. Each one needs a decision before the migration can claim feature parity. Findings below, ordered by real-world impact.

### Limitation 1 — Function-pointer-typed variable declarations

**Form:** `void(int) gFoo = []() -> void { ... };`
**tree-sitter:** accepts.
**PoC:** dropped.
**Impact:** **HIGH — critical for lambda usability.** Dropping this feature makes lambdas assignable only inline (e.g. as arguments to higher-order function calls); the pattern `void() gFoo = lambda;` for storing a callback in a named variable becomes impossible. Affects every lambda usage that wants a stable handle.

Two sub-cases, with very different difficulty:

**Sub-case 1A — Primitive return type** (`void(int)`, `int(int)`, `bool()`, etc.):
The return type is a reserved keyword, so the lexer guarantees it's a type. No C-typedef ambiguity here — only the LL(1) parsing shape is in question.

**Fix (1A):** Restructure so the function-pointer alternative lives at the *declaration* level, not inside `type_specifier`:

```lelwel
// BEFORE (PoC): nested inside type_specifier → E011 first-set conflict
type_specifier: function_pointer_type | primitive_type | Identifier

// AFTER: function-pointer form is a sibling of regular_decl, ordered first
declaration: ?t function_pointer_decl | regular_decl

function_pointer_decl:
  primitive_type '(' parameter_list? ')' Identifier '=' initializer

regular_decl:
  type_specifier declarator

type_specifier: primitive_type | Identifier
```

The `?t` predicate tells lelwel "try `function_pointer_decl` first; fall through if it fails". Failure is well-defined: `void gFoo;` (no `(` after `void`) is not a function-pointer form, so the parser falls through to `regular_decl`. No ambiguity, no ParserCallbacks needed.

**Sub-case 1B — User-defined return type** (`BOSystem(int) gFoo = lambda;`):
Here `BOSystem` could be a type or a value. Same C-typedef problem as Limitation 6.

**Fix (1B):** Extend the ParserCallbacks introduced for Limitation 6:

```lelwel
declaration:
  ?is_type_then_paren user_defined_function_pointer_decl
  | ?t function_pointer_decl
  | regular_decl

user_defined_function_pointer_decl:
  Identifier '(' parameter_list? ')' Identifier '=' initializer
```

The `?is_type_then_paren` callback asks the workspace symbol table "is `BOSystem` a known type AND is the next token `(`?". If yes, parse as function-pointer-type-of-BOSystem-return; otherwise fall through to `regular_decl` where `BOSystem` is a type specifier.

**Cost:** Sub-case 1A is a one-task grammar restructure — no architectural decisions, no symbol-table plumbing. Sub-case 1B piggybacks on the ParserCallbacks work already required for Limitation 6 — no extra callback infrastructure, just one more call site.

**Survey:** Confirm via `Phase 0` grep whether user-defined-return-type forms (`BOSystem(...)` style) appear in retail `.xs` files. If zero matches, sub-case 1A alone is sufficient and the ParserCallbacks dependency can be scoped down. If matches exist, both sub-cases are needed.

### Limitation 2 — `for (int i = 0; ...)` syntax (for_init_decl)

**Form:** `for (int i = 0; i < 10; i = i + 1) { ... }`
**tree-sitter:** accepts.
**PoC:** dropped; only `for (i = 0; ...)` works (requires `int i;` before the loop).
**Impact:** **HIGH.** This is a common pattern across `human_assist.xs`-style mods and retail AI scripts. Affects hundreds of for-loops.

Two sub-cases, with very different difficulty:

**Sub-case 2A — Primitive type** (`for (int i = 0; ...)`, `for (bool f = ...)`):
The first token is a reserved keyword. **No LL(1) conflict exists** here: `for_init_decl`'s primitive-keyword first set is disjoint from `expression`'s first set (expressions can't start with `int`, `bool`, etc.).

**Fix (2A):** Restrict `for_init_decl` to primitive types only:

```lelwel
for_init: for_init_primitive_decl | expression?

for_init_primitive_decl:
  primitive_type declarator ['=' expression]
```

Compiles cleanly, no `?t` needed, no ParserCallbacks needed.

**Sub-case 2B — User-defined type** (`for (BOSystem foo = ...)`):
The first token is an Identifier. **This is the actual LL(1) conflict**:

- `for_init_decl` first set: `{int, bool, ..., Identifier}` (declaration_specifiers includes user types)
- `expression` first set: `{Identifier, literals, (, ...}`
- Intersection: `{Identifier}` — E011.

(Note: the PoC's E028 diagnosis is incorrect; the real constraint is E011. The original rule attempt would have triggered an E011 first-set overlap, not an E028 nested-ordered-choice violation.)

**Fix (2B):** Extend the ParserCallbacks introduced for Limitation 6:

```lelwel
for_init:
  ?is_primitive_type for_init_primitive_decl
  | ?is_type for_init_user_decl
  | expression?

for_init_user_decl:
  Identifier declarator ['=' expression]
```

The `?is_type` callback asks the workspace symbol table "is `BOSystem` a known type?". If yes, parse as a user-typed declaration; otherwise fall through to `expression?`. Note that the `?is_primitive_type` predicate can be implemented as `?t` because primitive keywords are unambiguous at the lexer level.

**Critical regression risk — why `?t` declaration-first ordering is wrong here:**
The plan's original recommendation was "use `?t` predicate, same trick as `block_item`". This was incorrect. The `block_item` context allows `?t` because the failure mode (a declaration form that doesn't quite parse) is rare and can be cleaned up post-hoc. The `for_init` context is different — `?t` declaration-first ordering would silently change the meaning of every existing `for (i = 0; ...)` pattern:

```xs
// BEFORE (matches tree-sitter behaviour, current PoC behaviour):
int i = 5;
for (i = 0; i < 10; i = i + 1) { ... }   // reassigns outer i

// AFTER with `?t` declaration-first:
for (i = 0; i < 10; i = i + 1) { ... }   // creates fresh i, scoped to loop body
                                        // outer i is unchanged
```

This is a **real semantic regression**. The pattern `for (i = 0; i < n; i++)` is the canonical C-style counter loop; silently breaking it would invalidate a large fraction of existing XS code.

**Three valid paths:**
- **2A only (primitive types)** — minimum viable. Accepts `for (int i = 0; ...)`, falls back to `for (BOSystem foo = ...)` being a parse error. No regression risk.
- **2A + 2B (with ParserCallbacks)** — full parity. Same callback infrastructure as Limitation 6; zero extra wiring cost.
- **Accept the limitation** — keep `int i;` outside the for-loop. Clearest semantics, regression for existing code.

**Cost:** 2A is a one-rule grammar addition. 2B piggybacks on Limitation 6's ParserCallbacks — same callback answers both Limitation 6 and 2B and 1B. One callback, three uses.

**Survey:** Confirm via `Phase 0` grep whether user-defined-type `for_init_decl` (`for (BOSystem foo = ...)`) appears in retail `.xs` files. If zero matches, sub-case 2A alone is sufficient.

### Limitation 3 — ~~Chained comma expressions~~ RESOLVED (XS engine does not support them)

**Status:** Removed 2026-07-01 after survey confirmed zero usage.

**Form:** `for (i = 0, j = 0; i < 10; i = i + 1)` — the `(i = 0, j = 0)` part.
**tree-sitter:** accepts (because tree-sitter's XS grammar was derived from C, where the comma operator exists).
**PoC:** dropped; `comma_expr: assignment_expr` only (single-element).
**Survey:** Grep across 322 retail `.xs` files in `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/**/*.xs` plus 14 mod files in `mod/`:
- 2,221 total for-loops (1,115 retail + 1,106 mod)
- 0 with a comma in the INIT position (`for (a, b; ...)`)
- 0 with a comma in the UPDATE position (`for (...; a, b)`)
- 0 with 3+ commas total in the parens
- 0 genuine comma-as-expression patterns in `return`, `=`, `if`, or `while` contexts

The few false positives (`return vector(1.0, 0.0, 231.0);`, `if (kbUnitCount(a, b) ...)`) were commas inside function-argument lists, not the comma operator.

**Conclusion:** Chained commas in for-loops and comma-as-expression are syntactic patterns inherited from C grammar but **not supported by the XS engine**. Zero usage across 2,221 for-loops + zero usage in any other expression context is conclusive — if the engine supported them, at least one author would have written them. The PoC's `comma_expr: assignment_expr` is the correct production for what the XS engine actually accepts — no change needed, no ParserCallbacks needed, no `?t` trick needed.

**Implication for the migration's evaluation criteria:** A PoC limitation is only a real regression if the underlying XS engine accepts the form. Tree-sitter's grammar inheritance from C is over-permissive; the PoC's grammar should target engine support, not grammar parity. This framing should apply to all remaining "limitations" — survey evidence of zero usage is treated as strong evidence that the engine doesn't accept the form.

### Limitation 4 — Vector literals (`vector(1, 2, 3)`)

**Status:** Survey complete; impact assessment was wrong. Real fix needed.

**Form:** `vector v = vector(1, 2, 3);` (and `vector(...)` as expression, argument, return value, etc.)
**tree-sitter:** accepts.
**PoC:** dropped.
**Survey:** Across 322 retail + 264 mod `.xs` files:

| Form                               | Retail | Mod   | Total | %     |
| ---------------------------------- | ------ | ----- | ----- | ----- |
| `vector(x, y, z)` literal            | 2,671  | 2,668 | **5,339** | 99.3% |
| `new vector(N, default)` constructor | 22     | 14    | 36    | 0.7%  |

**Arity of vector literal calls:**
- 3 args (`x, y, z`): **5,363** — the canonical 3D coordinate form (AoM:R is a 3D game; vectors are 3-tuples of floats)
- 1 arg: 5
- 2 args: 1

The PoC README's claim was **exactly inverted**: vector literals are **149× more common** than `new vector(...)` constructors. The 6 non-3-arg calls are likely bugs (should be flagged at semantic analysis time) but the engine accepts them.

Real examples from retail:
```xs
vector startPoint = vector(95.0, 0.0, 63.0);      // aomspe02_p3.xs:73
gOverrideStartBaseVec = vector(103.0, 0.0, 225.0); // aomspe02_p5.xs:130
return vector(131.0, 0.0, 231.0);                  // fott27_p2.xs:66
aiPlanSetVariableVector(..., vector(229.0, 0.0, 245.0)); // s0l0p0c10m1_p3.xs:26
```

**Impact:** **HIGH — critical for engine support.** 5,339 vector literals would fail to parse in the PoC. The LSP needs vector literals for:
- Type checking (3-arg call produces a `vector`, not a generic call expression)
- Hover ("vector literal: 3D coordinate")
- Semantic tokens (highlight `vector` as a type/literal keyword)
- Diagnostics (flag arity mismatches at the call site)

**Disambiguation mechanics — why this is a clean fix:**

`vector` is a reserved keyword (`Vector='vector'` in PoC's token list). At the lexer level, `vector` is never an Identifier — it's always the keyword token. This is the same situation as `void`, `int`, `bool`, etc. — no C-typedef ambiguity for vector literals.

The PoC's `primary_expr` rule (lines 598-609) has alternatives for lambda, new, paren, default, literals, and Identifier — but **no `vector_literal` alternative**. For input `vector(1, 2, 3)`:
1. Parser sees the `Vector` keyword token
2. None of the existing alternatives match (`vector` isn't `[`, `new`, `(`, `default`, a literal, or `Identifier`)
3. **Parse fails** — `vector(1, 2, 3)` is a primary_expr no-match

The PoC completely omitted vector literals from `primary_expr`. The fix is one alternative:

```lelwel
primary_expr^:
  ...
| 'vector' '(' argument_list ')' @vector_literal    // <-- NEW
| ...
```

**LL(1) check** — first sets of all `primary_expr` alternatives are disjoint:
- `vector_literal` first set: `{vector}` — disjoint from `{[}`, `{new}`, `{(}`, `{default}`, `{IntConst}`, `{FloatConst}`, `{StringLiteral}`, `{true, TRUE}`, `{false, FALSE}`, `{null, NULL, nullptr}`, `{Identifier}`
- **No E011 conflict.** No `?t` trick. No ParserCallbacks needed.

**Why this is different from Limitations 1 and 2:**
Limitations 1 and 2 had LL(1) conflicts that required restructuring or ParserCallbacks. Limitation 4 has none — the keyword is unambiguous at the lexer level, and the alternative goes into a slot where `vector` doesn't currently appear. The fix is purely mechanical.

**Lesson learned — don't trust the PoC's impact claims:**

The PoC README was empirically wrong about Limitation 4's impact. This is the opposite of Limitation 3 (where the survey confirmed the PoC's drop was correct). Implication: every "rare" / "low" / "low-medium" impact assessment in the PoC's README needs empirical verification via the survey methodology. The remaining un-surveyed limitations (5, 7) should be surveyed before any fix design.

### Limitation 5 — C preprocessor (`#if`, `#elif`, `#else`, `#endif`, `#define`)

**Status:** Survey complete; HIGH-impact limitation requiring grammar changes.

**Form:** `#define NAME` (include guard), `#if (defined(X) == false)`, `#if (cDifficultyCurrent == cDifficultyEasy)`, `#elif`, `#else`, `#endif`.
**tree-sitter:** partial — `preproc_def` matches the name only (line 119-123 of `grammar.js`); `#define` bodies are discarded. The grammar has `preproc_if`, `preproc_ifdef`, `preproc_def` rules under `_top_level_item`.
**PoC:** no support. `#` is not even a token; directives cause parse errors.

**Survey:** Across 586 .xs files (322 retail + 264 mod):

| Directive               | Matches | Files | Purpose                                               |
| ----------------------- | ------- | ----- | ----------------------------------------------------- |
| `#if`                     | 448     | 116   | Conditional compilation (include guards + difficulty) |
| `#endif`                  | 448     | 116   | End of `#if` block (perfectly balanced)                 |
| `#elif`                   | 171     | 29    | Chained conditions (mostly difficulty levels)         |
| `#else`                   | 78      | 58    | Fallback branch                                       |
| `#define`                 | 27      | 27    | Include guards only (`#define NAME` with no value)      |
| `#undef`                  | 0       | 0     | Not used                                              |
| `#ifdef`/`#ifndef`          | 0       | 0     | Not used (the `defined()` form is used instead)         |
| `#error`/`#warning`/`#pragma` | 0       | 0     | Not used                                              |

**Total files affected: 116 (20% of all .xs files).** The 448 `#if` / 448 `#endif` count match perfectly — all blocks are properly closed.

**Two usage patterns:**

Pattern A — Include guards (27 files, all use this exact template):
```xs
#if (defined(BIOME_INCLUDE) == false)
#define BIOME_INCLUDE

// ... file contents ...

#endif
```

Pattern B — Difficulty-based conditional compilation (`global_spc_modifiers.xs` and similar):
```xs
#if (cDifficultyCurrent == cDifficultyEasy)
   // easy-difficulty modifiers
#elif (cDifficultyCurrent == cDifficultyModerate)
   // moderate-difficulty modifiers
#elif (cDifficultyCurrent == cDifficultyHard)
   // hard-difficulty modifiers
#else
   // titan-difficulty modifiers
#endif
```

**Impact:** **HIGH — 20% of files affected.** Without preprocessor support, 116 files fail to parse. This breaks:
- Symbol extraction for include-guarded library files (RM core, lib2/*, etc.)
- Conditional compilation: symbols defined only in `#if` branches are invisible
- Cross-file resolution for `mod/*/lib2/` files (all of which use guards)

**Disambiguation mechanics — why this is a clean fix:**

All preprocessor directives start with `#` at the start of a line (with optional leading whitespace). This is a clean, unambiguous signal — none of the existing `top_level_item` alternatives start with `#`. The fix is to add preproc directives as siblings in `top_level_item`:

```lelwel
token Hash='#';

preproc_def:   '#' 'define' Identifier
preproc_if:    '#' 'if' preproc_expr
preproc_elif:  '#' 'elif' preproc_expr
preproc_else:  '#' 'else'
preproc_endif: '#' 'endif'

preproc_expr: ...  // C-style expression with defined() + binary ops + Identifiers

top_level_item^:
  ...existing alternatives...
| preproc_def | preproc_if | preproc_elif | preproc_else | preproc_endif  // NEW
```

**LL(1) check:** first sets of all `top_level_item` alternatives are disjoint — the new alternatives start with `{#}`, none of the existing ones do. **No conflict.** No `?t` trick. No ParserCallbacks.

**LSP doesn't need to evaluate conditions.** Just preserve all branches in the AST so symbol extraction sees symbols from every possible code path (the user might be editing any of them). This matches what tree-sitter does.

**Why this is heavier than Limitation 4 but still bounded:**
- Limitation 4 was one alternative in `primary_expr` — done.
- Limitation 5 needs: 1 new lexer token (`#`), 5 new grammar rules for directive kinds, 1 new `preproc_expr` rule (C-style expression with `defined()`), integration into `top_level_item`.
- Tree-sitter has reference rules for all of these; the PoC implementation can crib from the existing tree-sitter grammar at `tree-sitter-xs/grammar.js` lines 740-790 (`preprocIf` helper and `preprocessor(cmd)` function).

**Three fix options, in order of recommendation:**

- **(A) Grammar-level preproc directives** (recommended): tree-sitter approach; preserves everything; `#` highlighted as a special token. Bounded grammar additions; no separate preprocessor.
- **(B) Text-level preprocessing pass**: a small text-substitution pipeline runs before lelwel sees the source. Replaces `#define NAME` with empty, evaluates `defined()` for include guards. Pros: lelwel stays simple. Cons: lose `#`-directive highlighting; complex include-guard tracking; transitive include tracking required.
- **(C) Accept the limitation**: 116 files fail to parse. NOT viable given the 20% impact.

### Limitation 6 — C typedef disambiguation + defensive ParserCallbacks

**Form:** `BOSystem myBOSystem;` (class type, no initializer) — should parse as a declaration. Also includes:
- C-style casts: `(int) round(float_val)` — valid XS, 0 usage in real code
- Function-style construction: `Foo()` — covered by Limitation 4 (`vector(...)`)
- Function declarations with class return type: `Foo bar() { ... }` — valid XS, 0 usage in real code
- L1B (user-defined return type for function-pointer-typed var): `BOSystem(int) gFoo = lambda;` — 0 usage
- L2B (user-defined type for for_init_decl): `for (BOSystem foo = ...)` — 0 usage

**tree-sitter:** resolves via 17-entry `conflicts` array + ERROR-node recovery. Many of these conflicts handle C patterns that don't appear in XS (casts with class names, function-style constructors, implicit-int fn decls).
**PoC:** uses `?t` predicates (declarations tried first, fallback to statements); `ParserCallbacks::Context = ()` is empty so no symbol-table lookup happens.

**Survey (real XS code):**

| Pattern                                                       | Matches | Notes                                                |
| ------------------------------------------------------------- | ------- | ---------------------------------------------------- |
| `ClassType variableName;` (no init)                            | 657     | The "typedef problem" case for class declarations    |
| `ClassType variableName = init;`                                | 439     | `=` disambiguates; no typedef issue                  |
| C-style casts `(primitive) expr`                                | 0       | Valid syntax; engine supports; never used            |
| C-style casts `(ClassName) expr`                                | 0       | Same; unused                                         |
| Fn decl with class return type                                 | 0       | Valid syntax; never used                              |
| L1B user-defined return type for fp-typed var                  | 0       | Same; unused                                         |
| L2B user-defined type for for_init_decl                        | 0       | Same; unused                                         |

The "two statements" interpretation of `BOSystem myBOSystem;` is **syntactically invalid** (would require two semicolons). The PoC's `?t` approach correctly handles all 1,096 real XS class declarations.

**Impact:** **HIGH architecturally, even though 0 actual ambiguity in current usage.** Per user's explicit direction: "the whole point of this migration is to make the LSP more robust and maintainable, we will cut no corners here." The migration should support all engine-valid syntax, not just what's used today.

**Decision: ParserCallbacks with TypeTable.** This unblocks:
- L1B (user-defined return type for fp-typed var): `?is_type_then_paren` callback
- L2B (user-defined type for for_init_decl): `?is_type` callback
- L6 itself: `?is_type` callback for general declaration disambiguation
- Defensive C-style cast support: `?is_type` callback for `(ClassName) expr` casts
- All other places where the parser needs to know if an Identifier is a type

**Fix (Path 2 — defensive):**

In `lelwel-xs/src/lib.rs`, replace `Context = ()` with:
```rust
pub struct TypeTable {
    types: HashSet<String>,  // known type names from engine API + workspace
}

impl ParserCallbacks for TypeTable {
    fn is_type(&self, ident: &str) -> bool {
        self.types.contains(ident)
    }
}
```

In `xs.llw`, replace `?t` predicates in declaration/statement decision points with `?is_type`:
```lelwel
block_item^: ?is_type declaration | ?is_type forward_declaration_rule 
           | ?is_type function_definition_rule | statement

top_level_item^: ?is_type declaration | ?is_type forward_declaration_rule 
               | ?is_type function_definition_rule | class_specifier
               | rule_definition | ...
```

For L1B's function-pointer-typed var with user-defined return type:
```lelwel
declaration:
  ?is_type_then_paren user_defined_function_pointer_decl
  | ?t function_pointer_decl
  | regular_decl
```

For L2B's user-defined-type for_init_decl:
```lelwel
for_init:
  ?is_primitive_type for_init_primitive_decl
  | ?is_type for_init_user_decl
  | expression?
```

For defensive C-style cast support:
```lelwel
primary_expr^:
  ...
| '(' type_specifier ')' primary_expr @cast_expr    // NEW — supports both (int) and (BOSystem) casts
| '(' expression ')' @paren_expr
| ...
```

(For `(ClassName) expr` casts where ClassName isn't a known type, the `?is_type` predicate handles the disambiguation; if not a type, the parser falls back to paren_expr which would error.)

**LSP-side wiring:**
```rust
// In lsp/src/parser.rs
pub struct ParserContext {
    pub known_types: Arc<TypeTable>,  // built from engine_api + workspace symbol table
}

pub fn parse(text: &str, context: &ParserContext) -> ParseResult {
    let mut parser = xs_parser::Parser::new(text);
    parser.set_context(context.known_types.clone());
    // ...
}
```

**Cost:** This is the architectural keystone of the migration:
- 1 new struct (`TypeTable`) in `lelwel-xs/src/lib.rs`
- 1 new trait impl (`ParserCallbacks`)
- Grammar: replace `?t` with `?is_type` in 2-3 places, add new alternatives for L1B/L2B/casts
- LSP-side: 1 new `ParserContext` field; populate `TypeTable` from engine API + workspace symbol table
- ~150-300 lines of Rust + ~30 lines of grammar changes

Heaviest single component of the migration, but well-bounded. The LSP already has the building blocks (engine_api.rs, symbol table extraction); we just need to plumb them through.

### Limitation 7 — Multi-declarator lists (`int a, b, c;`)

**Form:** `int a = 1, b = 2, c = 3;` (with initializers) or `int a, b;` (without).
**tree-sitter:** accepts.
**PoC:** partial — TODO marker; only single declarators parse.
**Survey:** Across 586 .xs files (322 retail + 264 mod):

| Pattern                                                       | Matches |
| ------------------------------------------------------------- | ------- |
| `int a, b;` (no initializer, two declarators)                   | **0**     |
| `int a = 1, b = 2;` (with initializers)                        | **0**     |
| `int a, b = 2;` (mixed)                                        | **0**     |
| `extern const type a, b = ...;` (with storage class)             | **0**     |
| Any 2+ commas followed by an identifier in a declaration       | 0 (only false positives inside function-arg lists) |

The simplest possible multi-declarator `int a, b;` NEVER appears in real XS code. The "suspicious" matches I initially caught (e.g., `int routeID = kbCreateAttackRouteWithPath("Route To P1", startPoint, endPoint);`) are all false positives — the commas are inside function-argument lists, not between declarators.

**Impact:** **NONE in current usage** but the syntax is likely supported by the XS engine (consistent with C-like languages). Per Path 2 — defensive design — we add support even with 0 current usage.

**Disambiguation mechanics — the SIMPLEST fix of all the limitations:**

The PoC's original recommendation (`?t` predicate) was overkill. The disambiguation is straightforward because `init_declarator_list` is used in a context where the FOLLOW set is `;` (not `,`). Same structural pattern as the PoC's already-working `parameter_list` and `argument_list`:

```lelwel
parameter_list: parameter (',' parameter)*      // PoC line 360, works
argument_list:  argument  (',' argument)*       // PoC line 614, works
init_declarator_list: init_declarator (',' init_declarator)*   // NEW, follows same pattern
```

**Why this doesn't hit E013** (unlike Limitation 3's `comma_expr`):

`init_declarator_list` is used in exactly one context:
```lelwel
declaration: declaration_specifiers init_declarator_list ';'
```

FOLLOW is `;`. The body's FIRST is `,`. `,` is never in FOLLOW. No E013 conflict.

Compare to `comma_expr` (Limitation 3) which was at the top of the expression hierarchy and used everywhere — including as function arguments where `,` IS in FOLLOW. THAT's why it hit E013.

**The fix is one line of grammar:**
```lelwel
init_declarator_list: init_declarator (',' init_declarator)*
```

No `?t` predicate. No `~` commit operator. No ParserCallbacks. No LL(1) conflict. Pure mechanical grammar addition.

**Cost:** the cheapest fix in the entire migration:
- 1 new grammar rule (`init_declarator_list`)
- 1 grammar change in `declaration` to use the new rule
- 1 grammar change in `for_init_primitive_decl` (Limitation 2A) and `for_init_user_decl` (Limitation 2B) to also use the new rule
- ~5 lines of grammar total

This is even cheaper than Limitation 4 (vector literals), which had to add one alternative to `primary_expr`. Limitation 7 just defines a new rule used in one place.

**Why defensive design matters here:** even though no current XS code uses multi-declarator, users migrating from C-style code might naturally try `int a, b;` and expect it to work. The cost of supporting this is so small (one line) that there's no reason not to.

### Limitation 8 — `do { } while ()`, `goto` / labels, `typedef`, pointer declarators

**Form:** `do { ... } while (cond);`, `goto label;`, etc.
**tree-sitter:** accepts.
**PoC:** dropped.
**Impact:** **NONE.** Per the PoC README, none of these are used in AoM:R mod scripts.

**Recommendation:** Drop permanently. Document in `lelwel-xs/README.md` as "intentionally unsupported — not used in AoM:R".

### Summary

| # | Feature | Impact | Fix recommendation |
|---|---|---|---|
| 1 | Function-pointer-typed vars | HIGH (lambda usability) | Restructure (1A) + ParserCallbacks (1B) |
| 2 | `for (int i = 0; ...)` | HIGH | Restructure (2A) + ParserCallbacks (2B) |
| 3 | ~~Chained commas~~ | NONE — **resolved 2026-07-01** | Engine does not support; tree-sitter inherits from C, no real XS code uses it |
| 4 | Vector literals | HIGH — 5,339 uses | One-line grammar addition; no conflicts |
| 5 | C preprocessor | HIGH — 116 files (20%) | Grammar-level: `#` token + 5 preproc rules + `preproc_expr`; no conflicts |
| 6 | C typedef + defensive ParserCallbacks | HIGH (architectural keystone) | TypeTable + ParserCallbacks; unblocks L1B/L2B + cast support |
| 7 | Multi-declarator | NONE in current usage; Path 2 defensive | One-line grammar addition; same pattern as `parameter_list`; no callbacks |
| 8 | `do`/`goto`/`typedef`/pointers | NONE | drop |

All PoC fixes are mechanical and pattern-based on the existing `?t` trick used in `block_item`. No new architectural problem is introduced; the work is concentrated in `xs.llw` plus wiring up `ParserCallbacks::Context` in `lelwel-xs/src/lib.rs`.

---

## 4. Phase-by-Phase Plan

### Phase 0 — Survey & decisions

**Goal:** Resolve all "unknown" impact assessments before any grammar work.

**Tasks:**
- [x] ~~**Survey C preprocessor use**~~ — **COMPLETED 2026-07-01** (Limitation 5): 116 files affected (20%). Two usage patterns: include guards (`#define NAME` with `defined()` check) and difficulty-based conditional compilation. See Limitation 5 in Section 3 for full survey data and fix recommendation.
- [ ] **Survey function-pointer-typed variables** in retail game folder:
  ```bash
  grep -lE '\b(void|int|bool|float|string)\s*\([^)]*\)\s*[a-zA-Z_][a-zA-Z0-9_]*\s*=' \
    ~/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/game/**/*.xs
  ```
- [ ] **Survey multi-declarator usage** in retail game folder:
  ```bash
  grep -lE '\b(int|bool|float|string|vector)\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=.*,' \
    ~/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/game/**/*.xs
  ```
- [ ] **Survey chained comma usage** in retail game folder:
  ```bash
  grep -lE 'for\s*\([^;]+,[^;]+;' \
    ~/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/game/**/*.xs
  ```
- [ ] **Survey vector literal usage**:
  ```bash
  grep -lE '\bvector\s*\(' \
    ~/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/game/**/*.xs \
    | xargs grep -L 'new\s\+vector'
  ```
- [ ] **Survey for_init_decl usage**:
  ```bash
  grep -cE 'for\s*\(\s*(int|bool|float|string|vector)\s+' \
    ~/.steam/steamapps/common/Age\ of\ Mythology\ Retold/game/**/*.xs
  ```
- [ ] Record all results in `lelwel-xs/SURVEY.md` and revisit Section 3 to mark each limitation's chosen fix path.
- [x] ~~**Decision:** choose ParserCallbacks vs `?t`-fallback for Limitation 6~~ — **RESOLVED 2026-07-01**: ParserCallbacks (Path 2 — defensive). Per user direction "the whole point of this migration is to make the LSP more robust and maintainable, we will cut no corners here." ParserCallbacks unblocks L1B, L2B, and defensive cast support for the same wiring cost.

**Deliverable:** `lelwel-xs/SURVEY.md` with concrete impact numbers + final fix-path decisions.

---

### Phase 1 — Grammar completion

**Goal:** Bring `xs.llw` to feature parity with the survey results.

**Files:**
- Modify: `tools/xs-language-server/lelwel-xs/src/xs.llw`
- Modify: `tools/xs-language-server/lelwel-xs/README.md` (update coverage table)

**Tasks:**

- [ ] **1.1a** Restore `for_init_decl`, primitive-type form (Limitation 2, sub-case A) — **always required**:
  Restrict the new `for_init_primitive_decl` to primitive types only. No conflict exists here because keyword first-sets are disjoint from `expression`'s first set:
  ```lelwel
  for_init: for_init_primitive_decl | expression?

  for_init_primitive_decl:
    primitive_type declarator ['=' expression]
  ```
  Verifies: `for (int i = 0; ...)` parses as a declaration; `for (i = 0; ...)` (where `i` is an outer variable) falls through to `expression?` and parses correctly (no semantic regression).

- [ ] **1.1b** Extend with user-defined-type form (Limitation 2, sub-case B) — **REQUIRED** (Path 2 — defensive, even though 0 current usage):
  Add the `?is_type`-guarded alternative:
  ```lelwel
  for_init:
    ?is_primitive_type for_init_primitive_decl
    | ?is_type for_init_user_decl
    | expression?

  for_init_user_decl:
    Identifier declarator ['=' expression]
  ```
  Note: `?is_primitive_type` can be implemented as `?t` because primitive keywords are unambiguous at the lexer level. The `?is_type` callback is the same one introduced in Phase 1.6.

- [ ] **1.2** ~~Restore chained comma expressions~~ — **REMOVED 2026-07-01**:
  Survey confirmed 0 usage across 2,221 retail for-loops and zero genuine comma-as-expression patterns; the XS engine does not support the comma operator. See Limitation 3 in Section 3 for survey details.

- [ ] **1.3a** Restore function-pointer-typed variables, primitive return types (Limitation 1, sub-case A) — **required for lambda usability**:
  Restructure `xs.llw` so the function-pointer alternative lives at the *declaration* level, not inside `type_specifier`:
  ```lelwel
  declaration: ?t function_pointer_decl | regular_decl

  function_pointer_decl:
    primitive_type '(' parameter_list? ')' Identifier '=' initializer

  regular_decl:
    type_specifier declarator

  type_specifier: primitive_type | Identifier
  ```
  Verifies that `void gFoo;` still parses (falls through to `regular_decl`) and `void(int) gFoo = lambda;` parses as a `function_pointer_decl`.

- [ ] **1.3b** Extend with user-defined-return-type alternative (Limitation 1, sub-case B) — **REQUIRED** (Path 2 — defensive, even though 0 current usage):
  Add the `?is_type_then_paren`-guarded alternative to the declaration rule:
  ```lelwel
  declaration:
    ?is_type_then_paren user_defined_function_pointer_decl
    | ?t function_pointer_decl
    | regular_decl

  user_defined_function_pointer_decl:
    Identifier '(' parameter_list? ')' Identifier '=' initializer
  ```
  The `?is_type_then_paren` callback is the same `?is_type` from Phase 1.6 with an additional `(` lookahead check.

- [ ] **1.4** Add multi-declarator form (Limitation 7) — **REQUIRED** (Path 2 — defensive, even though 0 current usage):
  The simplest fix in the migration — one line of grammar. No `?t`, no `~`, no callbacks. Same structural pattern as the PoC's already-working `parameter_list` and `argument_list`:
  ```lelwel
  init_declarator_list: init_declarator (',' init_declarator)*
  ```
  Update `declaration` to use the new rule:
  ```lelwel
  declaration: declaration_specifiers init_declarator_list ';'
  ```
  Also apply to `for_init_primitive_decl` (Limitation 2A) and `for_init_user_decl` (Limitation 2B) — both should accept multi-declarator form for symmetry with C-like languages.

- [ ] **1.5** Add vector literal (Limitation 4) — **REQUIRED** (5,339 uses):
  Add `vector_literal` as a sibling alternative in `primary_expr`:
  ```lelwel
  primary_expr^:
    ...
  | 'vector' '(' argument_list ')' @vector_literal    // <-- NEW
  | ...
  ```
  No conflicts (first set `{vector}` is disjoint from all other `primary_expr` alternatives). No `?t`, no ParserCallbacks. Same pattern as the PoC's existing `'true' @true_literal` etc. — keyword + literal pair.
  After this change, `vector v = vector(1, 2, 3);`, `f(vector(1, 2, 3))`, `return vector(x, y, z);`, and standalone `vector(x, y, z);` all parse correctly.

- [ ] **1.6** Wire up ParserCallbacks with TypeTable (Limitation 6, Path 2 — defensive) — **REQUIRED** (architectural keystone for L1B, L2B, defensive cast support):
  - In `lelwel-xs/src/lib.rs`: replace `Context = ()` with:
    ```rust
    pub struct TypeTable {
        types: HashSet<String>,  // known type names from engine API + workspace
    }

    impl ParserCallbacks for TypeTable {
        fn is_type(&self, ident: &str) -> bool {
            self.types.contains(ident)
        }
    }
    ```
  - In `xs.llw`: replace `?t` predicates in `block_item^` / `top_level_item^` with `?is_type` predicates.
  - In `lsp/src/parser.rs`: add `ParserContext { known_types: Arc<TypeTable> }`; populate `TypeTable` from engine API + workspace symbol table on each parse.
  - The parser wrapper (`xs-parser` binary entry) accepts a `--types <file>` argument that pre-populates the context for the standalone binary.
  - This is the architectural keystone: the same `?is_type` callback is reused by L1B (`?is_type_then_paren`), L2B (`?is_type`), defensive cast support, and any future C-typedef-ambiguity cases.

- [ ] **1.7** Add C preprocessor support (Limitation 5) — **REQUIRED** (116 files affected, 20% of codebase):
  Use the **grammar-level approach** (Option A from Section 3) — add preprocessor directives as siblings in `top_level_item`, matching what tree-sitter already does at `tree-sitter-xs/grammar.js` lines 740-790. No preprocessing pass needed.
  ```lelwel
  token Hash='#';

  preproc_def:   '#' 'define' Identifier
  preproc_if:    '#' 'if' preproc_expr
  preproc_elif:  '#' 'elif' preproc_expr
  preproc_else:  '#' 'else'
  preproc_endif: '#' 'endif'

  preproc_expr: ...  // C-style: defined(X), binary ops, engine constants (Identifiers)

  top_level_item^:
    ...existing alternatives...
  | preproc_def | preproc_if | preproc_elif | preproc_else | preproc_endif  // NEW
  ```
  No conflicts (first set `{Hash}` is disjoint from all existing `top_level_item` alternatives). The LSP does NOT evaluate conditions — preserve all branches in the AST so symbol extraction sees every code path the user might edit.
  
  Sub-steps:
  - Add `Hash='#'` to the Logos lexer (`lelwel-xs/src/lexer.rs`).
  - Add `preproc_def` / `preproc_if` / `preproc_elif` / `preproc_else` / `preproc_endif` rules to `xs.llw`.
  - Add `preproc_expr` rule (C-style expression: `defined(X)` predicate + binary ops + `(` `)` + engine constants as Identifiers).
  - Wire preproc alternatives into `top_level_item^`.

- [ ] **1.8** Verify against the retail game folder. Build a `game_parse.rs` integration test inside `lelwel-xs/` that walks `AOMR_GAME_PATH/**/*.xs` and asserts zero parse failures (where "failure" means: parse error count > 0). This is the migration's first behavioural gate, before the LSP integration.

- [ ] **1.9** Update `lelwel-xs/README.md`'s coverage table with the new state.

**Deliverable:** `xs.llw` covers every feature the survey marked as "needed"; `cargo build` clean; `game_parse` test passes against the retail folder.

---

### Phase 2 — Lexer regex completion

**Goal:** The generated `lexer.rs` is a Logos skeleton with token variants but no regex patterns. Without filled-in regexes, the lexer cannot tokenize a single character of source.

**Files:**
- Create: `tools/xs-language-server/lelwel-xs/src/lexer.rs` (overwrite the generated skeleton)
- Cross-reference: `tools/xs-language-server/tree-sitter-xs/grammar.js` lines ~600-780 (lexer rules)

**Tasks:**

- [ ] **2.1** Extract all terminal tokens from `xs.llw`. There are ~70 (every keyword, operator, punctuation).
- [ ] **2.2** For each keyword (`int`, `void`, `bool`, `float`, `string`, `vector`, `extern`, `static`, `mutable`, `const`, `ref`, `rule`, `class`, `if`, `else`, `while`, `for`, `switch`, `case`, `default`, `break`, `continue`, `return`, `true`, `false`, `null`, `nullptr`, `new`, `this`, `include`), add a Logos regex attribute matching the keyword as a whole word (use `\b` boundaries).
- [ ] **2.3** For each operator (`+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `!`, `~`, `&`, `|`, `^`, `=`, `+=`, `-=`, `*=`, `/=`, `++`, `--`, `->`, `?.`, `?`), add the corresponding Logos attribute. Operators that share prefixes (e.g. `=` and `==`, `<` and `<=`, `&` and `&&`) must use Logos's priority feature: longer alternatives win.
- [ ] **2.4** For each punctuation (`(`, `)`, `{`, `}`, `[`, `]`, `;`, `,`, `.`, `:`, `?`), add Logos attributes.
- [ ] **2.5** For `Identifier`: regex `[a-zA-Z_][a-zA-Z0-9_]*` (Xs identifiers per the AoM:R convention).
- [ ] **2.6** For `IntConst`: regex `-?[0-9]+` (or signed via a unary `-` rule — match `tree-sitter-xs` choice).
- [ ] **2.7** For `FloatConst`: regex `-?[0-9]+\.[0-9]+` (and possibly exponent forms).
- [ ] **2.8** For `StringLiteral`: regex `"([^"\\]|\\.)*"` (handle escaped quotes).
- [ ] **2.9** For `LineComment`: `//[^\n]*`.
- [ ] **2.10** For `BlockComment`: `/\*([^*]|\*+[^*/])*\*+/`.
- [ ] **2.11** For `Whitespace`: `\s+` (the lossless CST includes it; consumers must skip).
- [ ] **2.12** Test: write a `lelwel-xs/tests/lexer.rs` that tokenizes a small sample file and asserts the token sequence matches expected kinds + spans.

**Deliverable:** A working Logos lexer that tokenizes all 302 retail `.xs` files without errors.

---

### Phase 3 — Parser wrapper API lockdown

**Goal:** Define the migration's public interface before any consumer rewrites begin.

**Files:**
- Modify: `tools/xs-language-server/lelwel-xs/src/lib.rs` (move from PoC skeleton to LSP-compatible shape)
- Create: `tools/xs-language-server/lsp/src/parser.rs` (replace tree-sitter wrapper with lelwel wrapper)

**Tasks:**

- [ ] **3.1** Define the wrapper's return type. The current tree-sitter wrapper exposes:
  ```rust
  pub fn parse(text: &str) -> Option<Tree>
  pub fn extract_include_directives(tree: &Tree, src: &str) -> Vec<IncludeDirective>
  pub fn detect_include_path_at_position(tree: &Tree, src: &str, line: u32, col: u32) -> Option<IncludePath>
  ```
  The lelwel wrapper must expose:
  ```rust
  pub fn parse(text: &str, context: &ParserContext) -> ParseResult
  pub fn extract_include_directives(result: &ParseResult, src: &str) -> Vec<IncludeDirective>
  pub fn detect_include_path_at_position(result: &ParseResult, src: &str, line: u32, col: u32) -> Option<IncludePath>
  ```
  Where `ParseResult` is a struct holding:
  ```rust
  pub struct ParseResult {
      pub cst: Cst,                          // lelwel's lossless tree
      pub diagnostics: Vec<Diagnostic>,       // codespan-reporting errors
      pub includes: Vec<IncludeDirective>,    // precomputed for hot-path consumers
  }
  pub struct ParserContext {
      pub known_types: Arc<HashSet<String>>,  // for ParserCallbacks (Limitation 6)
  }
  ```

- [ ] **3.2** Implement `parse()`. Internally: `lelwel::Parser::new(text).parse(&context)` → wrap into `ParseResult`. Map codespan-reporting diagnostics to a stable internal representation.

- [ ] **3.3** Implement `extract_include_directives()`. Walk the typed AST looking for `Include` nodes; pre-extract the path text. The AST navigation is now typed (no more `node.kind() == "include_directive"` string comparisons).

- [ ] **3.4** Implement `detect_include_path_at_position()`. Binary search the `includes` list by line/col.

- [ ] **3.5** Decide on consumer-side translation. **Two options:**
  - **(A) Replace all consumers to use the typed AST directly.** Cleanest; largest code churn.
  - **(B) Wrap the typed AST in a shim that mimics tree-sitter's `Tree<Node>` API.** Smaller consumer churn; leaves the migration incomplete (still paying the tree-sitter API tax).
  
  Recommendation: (A). The whole point of migration is typed access; shimming defeats the purpose.

- [ ] **3.6** Add a `lsp/src/parser.rs` integration test that parses a sample file and asserts `ParseResult { cst, diagnostics, includes }` has the expected shape.

**Deliverable:** `lsp/src/parser.rs` exposes a stable, typed `parse()` interface; downstream consumers can be ported mechanically.

---

### Phase 4 — Consumer rewrites

**Goal:** Replace every tree-sitter traversal with a typed-AST traversal. This is the bulk of the migration.

#### 4.1 — `lsp/src/symbols.rs` (1005 lines, the largest consumer)

**Tasks:**

- [ ] **4.1.1** Rewrite `extract_function_definition` — walk `TranslationUnit::FunctionDefinition(fd)` instead of `cursor.goto_first_child() / kind() == "function_definition"`.
- [ ] **4.1.2** **DELETE** `extract_error_function_definition` (symbols.rs:557) — obsolete; lelwel produces first-class `FunctionDefinition` nodes.
- [ ] **4.1.3** **DELETE** `extract_error_forward_declaration` (symbols.rs:606) — replaced by `extract_forward_declaration` walking `TranslationUnit::ForwardDeclaration(fd)`.
- [ ] **4.1.4** Rewrite `extract_class`, `extract_class_member`, `extract_rule`, `extract_declaration`, `extract_local_declarations`, `extract_params`, `extract_modifiers`, `has_ref_qualifier`, `format_function_pointer_type`, `extract_param_default` — mechanical typed-walk conversions.
- [ ] **4.1.5** Update `build_symbol_table` / `build_full_symbol_table` to dispatch on the typed AST root.
- [ ] **4.1.6** Address Limitation #485 (the silent `None => return` for no-init declarations): now that we have first-class forward declarations, decide whether `int gFoo;` (no init) at top level should become a `Declaration` symbol or remain dropped. Likely the latter (XS treats uninitialised globals as runtime errors), but the PoC grammar makes it explicit.

#### 4.2 — `lsp/src/semantic_tokens.rs` (621 lines)

**Tasks:**

- [ ] **4.2.1** Rewrite `walk_for_tokens`. The typed AST includes whitespace + comments as tokens; the walker must skip them.
- [ ] **4.2.2** Rewrite `classify_member_identifier` (line 404): the parent walk becomes a typed match (`Parent::FieldExpression` / `Parent::CallExpression`).
- [ ] **4.2.3** Rewrite `extract_symbols` (the public test helper) to dispatch on the typed AST root.
- [ ] **4.2.4** `MemberIndex::build` (cache.rs) re-parses via `cache::load_or_parse_symbols` — verify the cache schema accommodates the new return type. May need a `v3` cache version.

#### 4.3 — `lsp/src/diagnostics.rs` (444 lines)

**Tasks:**

- [ ] **4.3.1** Rewrite `collect_diagnostics` to consume `ParseResult.diagnostics` directly (no more walking ERROR nodes).
- [ ] **4.3.2** Rewrite `to_diagnostic` to map codespan-reporting severities to LSP severities.
- [ ] **4.3.3** Rewrite `friendly_kind`, `unexpected_token_message`, `missing_token_message` to consume codespan labels instead of tree-sitter node kinds.
- [ ] **4.3.4** The "must not leak grammar internals" contract stays; the assertion style changes (no more `"ERROR" / "MISSING"` rejection — instead, codespan labels are opaque strings and we only forward ones we recognise).

#### 4.4 — `lsp/src/merged_view.rs` (933 lines)

**Tasks:**

- [ ] **4.4.1** Update the 5 `parser::parse` call sites to the new signature: `parser::parse(text, &ctx)` where `ctx` is built from the workspace's known types.
- [ ] **4.4.2** Update `extract_include_directives` to consume the typed AST.
- [ ] **4.4.3** The 15 fixture files in `src/semantic_fixtures/` are the regression target — re-run all of them after migration.

#### 4.5 — `lsp/src/semantic.rs` (1460 lines, 36 tests)

**Tasks:**

- [ ] **4.5.1** Update the 4 `parser::parse` call sites (lines 59, 122, 493, 623).
- [ ] **4.5.2** Rewrite `collect_calls` / `walk_calls` / `call_callee_name` (lines 840-876) to dispatch on `Expression::Call(callee, args)`.
- [ ] **4.5.3** Update `node_text`, `node_range`, `find_named_child` (the duplicated helpers) — or consolidate them into a single utility module.

#### 4.6 — `lsp/src/typecheck.rs` (933 lines, 24 tests)

**Tasks:**

- [ ] **4.6.1** Update the parser call sites.
- [ ] **4.6.2** Rewrite `check_calls` / `check_calls_with_merged` to walk the typed AST.
- [ ] **4.6.3** Rewrite `extract_callee_name`, `expr_type`, `is_constant_expression`.

#### 4.7 — `lsp/src/definition_check.rs` (453 lines, 15 tests)

**Tasks:**

- [ ] **4.7.1** Update the parser call sites.
- [ ] **4.7.2** Rewrite the `translation_unit` walker for `function_definition` + `declaration`.
- [ ] **4.7.3** Rewrite `parameter_declaration` checks for `type_qualifier → ref` and `= default`.
- [ ] **4.7.4** Rewrite `validate_declaration`, `is_scalar_type`, `is_constant_expression` (the local copy — consolidate later).

#### 4.8 — `lsp/src/references.rs` (185 lines, 6 tests)

**Tasks:**

- [ ] **4.8.1** Update the parser call sites.
- [ ] **4.8.2** Rewrite `find_identifier_uses` to walk `Expression::Identifier(name)` typed nodes.
- [ ] **4.8.3** Rewrite `identifier_range_at` — `descendant_for_point_range` becomes a binary search on the typed AST's spans.

#### 4.9 — `lsp/src/server.rs` (1332 lines, 1 test)

**Tasks:**

- [ ] **4.9.1** Update the 8 `parser::parse` call sites (lines 743, 814, 851, 881, 934, 994, 1013, 1029). All sites use the same `let Some(...) = parser::parse(&text) else { ... }` shape — mechanical conversion.
- [ ] **4.9.2** No semantic changes — server.rs is a thin dispatcher.

#### 4.10 — `lsp/src/cache.rs` (560 lines, 12 tests)

**Tasks:**

- [ ] **4.10.1** The `ParseCacheEntry` deliberately doesn't cache the `Tree` (line 96 comment). With lelwel, the typed AST is `!Send` / `!Serialize` in many cases — same constraint applies. May need a `v3` cache version that stores only `(diagnostics, includes)` and re-parses on demand.
- [ ] **4.10.2** Update the `game_parse/v3/` cache key schema.

#### 4.11 — `lsp/src/workspace.rs` (900 lines, 25 tests)

**Tasks:**

- [ ] **4.11.1** Production code has no tree-sitter (per inventory). Only one test (`direct_include_resolves_via_tree_sitter` at line 686) needs updating.

#### 4.12 — Duplication consolidation (cross-cutting)

The `node_range` / `node_text` / `find_named_child` helpers are duplicated across 6 files (`symbols.rs`, `semantic_tokens.rs`, `semantic.rs`, `typecheck.rs`, `references.rs`, `definition_check.rs`). After migration, these helpers become typed-method calls on the AST node types. Consolidate them into a single utility module (`lsp/src/ast_util.rs`).

#### 4.13 — Bin files

- [ ] **4.13.1** `bin/day1_probe.rs`: rewrite to walk the typed AST. Drop `tree-sitter-c` dependency (line 22 of `lsp/Cargo.toml`).
- [ ] **4.13.2** `bin/dump_top_level.rs`: rewrite.
- [ ] **4.13.3** `bin/inspect_tree.rs`: rewrite.
- [ ] **4.13.4** `bin/lsp_roundtrip_test.rs`: zero changes (pure JSON-RPC harness).

**Deliverable:** Every LSP feature (hover, go-to-def, references, rename, prepareRename, completion, documentSymbol, workspaceSymbol, semanticTokens, diagnostics) works against the typed AST. No `tree_sitter::` imports remain in `lsp/src/`.

---

### Phase 5 — Test migration

**Goal:** Update every assertion that compares AST shapes.

**Files:**
- Modify: every `lsp/tests/*.rs` file
- Modify: every `#[cfg(test)] mod tests` block in `lsp/src/*.rs`

**Tasks:**

- [ ] **5.1** `tests/symbols_cleanup_repro.rs` (6 tests):
  - **5.1.1** R1-F-02 (the test that asserts ERROR-recovery IS needed, line 113-127) **inverts**: now assert that forward declarations produce **first-class** `ForwardDeclaration` AST nodes, not ERROR nodes.
  - **5.1.2** R1-F-01 / R1-F-03 audit assertions: keep (source-grepping is parser-independent).

- [ ] **5.2** `tests/game_folder_parse.rs` (8 tests) — the **migration's hardest regression gate**:
  - **5.2.1** The "3000 unexpected ERRORs" assertion has no direct equivalent (lelwel has no ERROR nodes). Replace with: "every game file produces a `ParseResult` whose `diagnostics` vec is either empty or contains only known recoverable errors; every `SymbolTable` is non-empty."
  - **5.2.2** Per-file 100-error threshold: replace with "every file's `ParseResult.diagnostics.len()` is below a configurable cap".
  - **5.2.3** Run against the retail folder; record baseline numbers; tighten caps after one iteration.

- [ ] **5.3** `tests/include_goto_definition_repro.rs` (5 tests): update the spawn-binary harness to consume the new `ParseResult` JSON shape.

- [ ] **5.4** All other integration tests (`class_extraction_repro.rs`, `class_member_extraction_repro.rs`, `class_member_semantic_tokens_repro.rs`, `forward_decl_repro.rs`, `r3_f01_deadlock_repro.rs`, `r5_test_honesty_repro.rs`, `semantic_token_distinctions_repro.rs`, `semantic_tokens_repro.rs`): mechanical updates to use the typed AST for assertions. No semantic test logic should change.

- [ ] **5.5** All in-module `#[cfg(test)] mod tests`: same mechanical update.

- [ ] **5.6** `lsp/src/semantic_fixtures/`: 15 `.xs` files. No source changes needed — these are XS source, parser-agnostic.

- [ ] **5.7** Add a new integration test `tests/typedef_disambiguation_repro.rs` that pins the ParserCallbacks behaviour for Limitation 6 (asserts `BOSystem myBOSystem;` produces a `Declaration` symbol, not two expression statements).

- [ ] **5.8** Add a new integration test `tests/forward_decl_first_class.rs` that pins the structural win (asserts forward decls produce typed AST nodes, not ERROR nodes).

**Deliverable:** All 252+ unit tests pass against the typed AST. The two new tests pin the migration's structural wins.

---

### Phase 6 — Plugin compatibility check

**Goal:** Verify the IntelliJ plugin is unaffected by the LSP-side change.

**Files:** none expected; plugin source is tree-sitter-free (per Phase 0 inventory).

**Tasks:**

- [ ] **6.1** Bump plugin version: 0.x.y → 0.x.(y+1) per the policy in `tools/intellij-xs-plugin/AGENTS.md`. The bundled LSP binary changes; the version bump is mandatory.
- [ ] **6.2** Audit `tools/intellij-xs-plugin/src/main/kotlin/lsp/XsSemanticTokensConverter.kt`: confirm the token-type / modifier-name string mapping matches the new legend. **The legend is unchanged** (the typed AST doesn't change token types — it changes the *walker* that emits them) — but verify.
- [ ] **6.3** Smoke-test the plugin against the new LSP binary (the same T16 Rider smoke test documented in `docs/post-lsp-migration-issues.md`). Same outcome expected: no regressions.
- [ ] **6.4** Verify `tools/intellij-xs-plugin/gradle.properties:pluginVersion` is bumped.
- [ ] **6.5** Rebuild the plugin zip: `./gradlew buildPlugin`. Confirm `build/distributions/intellij-xs-plugin-*.zip` includes the new LSP binary.

**Deliverable:** Plugin rebuilt with new LSP binary; smoke test passes.

---

### Phase 7 — tree-sitter removal

**Goal:** Remove tree-sitter from the dependency graph entirely.

**Files:**
- Delete: `tools/xs-language-server/tree-sitter-xs/` (entire directory)
- Delete: `tools/xs-language-server/lsp/src/bin/day1_probe.rs` (or keep rewritten)
- Delete: `tools/xs-language-server/lsp/src/bin/dump_top_level.rs`
- Delete: `tools/xs-language-server/lsp/src/bin/inspect_tree.rs`
- Modify: `tools/xs-language-server/lsp/Cargo.toml` (remove tree-sitter deps)
- Modify: `tools/xs-language-server/Cargo.toml` (workspace members)
- Modify: `tools/intellij-xs-plugin/...` (no changes; plugin was already tree-sitter-free)

**Tasks:**

- [ ] **7.1** Update `tools/xs-language-server/lsp/Cargo.toml`:
  - Remove: `tree-sitter = "0.26.9"`, `tree-sitter-c = "0.24.2"`, `tree-sitter-language = "0.1"`, `tree-sitter-xs = { path = "../tree-sitter-xs" }`
  - Add: `xs-parser = { path = "../lelwel-xs" }` (or whatever the lelwel crate publishes as)
  - Remove: `cc` from build-deps (no longer compiles parser.c)
  - Keep: `logos`, `codespan-reporting` (now used by lelwel xs directly)
- [ ] **7.2** Update `tools/xs-language-server/Cargo.toml` workspace members: keep `lsp` + `lelwel-xs`. Remove `tree-sitter-xs` if it was a workspace member (per inventory: it was NOT a member — it was a path-dep from `lsp/Cargo.toml`).
- [ ] **7.3** Delete `tools/xs-language-server/tree-sitter-xs/`. Verify nothing else references it (especially the IntelliJ plugin — should be safe).
- [ ] **7.4** Delete the three dev-only bins that depended on `tree_sitter_xs::LANGUAGE` (after Phase 4.13 rewrote them — they probably no longer exist or have been replaced).
- [ ] **7.5** `cargo build` from clean. Verify only `xs-parser` + `lelwel` + transitive deps.
- [ ] **7.6** `cargo build --release` to confirm release mode also works.
- [ ] **7.7** `cargo test` from clean. Verify all 252+ tests pass.

**Deliverable:** `tree-sitter-xs/` deleted; `tree-sitter` not in dependency graph; LSP builds + tests pass from clean.

---

### Phase 8 — Documentation & release

**Goal:** Update all human-facing docs to reflect the new architecture.

**Files:**
- Modify: `tools/xs-language-server/AGENTS.md` (testing-capabilities, stack description)
- Modify: `tools/xs-language-server/lsp/README.md` (architecture, dependencies)
- Modify: `tools/xs-language-server/lelwel-xs/README.md` (promote from PoC to production)
- Modify: `docs/post-lsp-migration-issues.md` — close it (or annotate: "superseded by tree-sitter → lelwel migration")
- Modify: `openspec/SPEC.md` if it describes the LSP architecture
- Modify: `README.md` (compatibility matrix entry)

**Tasks:**

- [ ] **8.1** Update `tools/xs-language-server/AGENTS.md` testing-capabilities: tree-sitter integration test is gone; the equivalent is now a lelwel integration test in `lelwel-xs/tests/`.
- [ ] **8.2** Update `tools/xs-language-server/lsp/README.md` dependencies section: tree-sitter line replaced with `xs-parser (lelwel-generated)`.
- [ ] **8.3** Update `tools/xs-language-server/lelwel-xs/README.md`: change "PoC" to "production"; update the coverage table to reflect the final state.
- [ ] **8.4** Document the `ParserContext` API (Limitation 6 resolution): how to populate `known_types` from the workspace, how to thread it through the LSP call chain.
- [ ] **8.5** Update `openspec/changes/xs-language-server/` archive entries: add a new entry summarizing the migration.
- [ ] **8.6** Tag a release: `git tag vX.Y.Z` per the project's release convention. Bump the LSP version in `lsp/Cargo.toml`.
- [ ] **8.7** Save a session summary to engram.

**Deliverable:** Docs match the code. Release tagged.

---

## 5. Risk Register

| # | Risk | Likelihood | Severity | Mitigation |
|---|---|---|---|---|
| 1 | Grammar rebuilds for 7 limitations hit unexpected E011/E012/E013 conflicts beyond what the `?t` pattern resolves. | MEDIUM | HIGH | Phase 0 survey narrows the impact surface; Phase 1.1-1.5 are sequential — each fix is verifiable in isolation before the next. |
| 2 | Limitation 6 (C typedef) cannot be solved cleanly with `?t` alone; ParserCallbacks requires a deeper lelwel investigation. | MEDIUM | HIGH | Phase 0 forces an explicit decision before Phase 1. Fallback is the post-processing pass (option C). |
| 3 | Logos lexer regex patterns miss an edge case (e.g. nested block comments, escape sequences in strings) that causes tokenizer divergence from tree-sitter. | MEDIUM | MEDIUM | Phase 2.12 verifies the lexer against all 302 retail files before any consumer port starts. |
| 4 | The `node_range` / `node_text` helper consolidation breaks subtle assumptions in 6 files. | HIGH | MEDIUM | Phase 4.12 is sequenced AFTER all consumers are individually working. Consolidation is a refactor with the new types as the target. |
| 5 | `tests/game_folder_parse.rs`'s "3000 ERRORs" cap has no equivalent in lelwel, and the replacement metric is too lax (passes while regressions hide) or too strict (impossible to meet). | HIGH | HIGH | Phase 5.2 forces a baseline measurement first; caps are tightened after one or two iterations. |
| 6 | Semantic-token legend rename (e.g. constant/rule/extern additions from Bucket C) desynchronises the LSP server from the IntelliJ plugin's `XsSemanticTokensConverter`. | LOW | MEDIUM | Phase 6.2 audits the converter explicitly. The convention is to keep the legend stable across the migration. |
| 7 | ParserCallbacks requires threading `Arc<TypeTable>` through every `parser::parse` call site — easy to forget. | HIGH | LOW | Compile error catches this; no runtime risk. |
| 8 | Big-bang cutover produces a 7-day branch with no shippable intermediate. Discoveries late in the cycle (e.g. Phase 4 consumer rewrites revealing grammar gaps) require re-doing Phase 1 work. | HIGH | HIGH | This is the inherent risk of the chosen cutover strategy. The mitigation is **strict ordering**: Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8, with each phase's gate held before the next begins. Do NOT start Phase 4 until Phase 1 + Phase 2 + Phase 3 are green. |
| 9 | The lelwel author `0x2a-42` is a single maintainer. If lelwel has a bug or API breaks across versions, the migration has no fallback. | MEDIUM | MEDIUM | Mitigation: pin `lelwel = "=0.10.x"` exactly; track upstream changes; the tree-sitter branch remains in git history (don't delete `tree-sitter-xs/` until Phase 7's verification is fully green — keep it around in a `backup/` folder for one release cycle, then delete). |
| 10 | Performance regression — lelwel is recursive-descent, tree-sitter is GLR/LR. Recursive-descent on the same inputs may be 2-5x slower for large files. | MEDIUM | MEDIUM | Mitigation: Phase 1.8 + Phase 5.2 measure against the retail folder; if regression exceeds 2x, the merge-paste cost in `MergedView::build` is the likely culprit (re-parse per file) and can be cached. |

---

## 6. Open Decisions (to resolve before Phase 1)

- [x] ~~**ParserCallbacks vs `?t`-fallback for Limitation 6**~~ — **RESOLVED 2026-07-01**: ParserCallbacks with TypeTable. Architectural keystone for L1B, L2B, defensive cast support, and any future C-typedef-ambiguity cases. See Section 3 Limitation 6 for full design.
- [x] ~~**C preprocessor approach**~~ — **RESOLVED 2026-07-01**: grammar-level (Option A from Section 3). 116 files affected; tree-sitter's grammar has reference rules; bounded additions to `top_level_item` + new `preproc_expr` rule.
- [ ] **Whether to keep `tree-sitter-xs/` as a backup directory for one release cycle**, or delete immediately at Phase 7.
- [ ] **Branch strategy for the big-bang**: single long-lived branch off `main`, or a temporary feature branch with a daily rebase. (The user picked big-bang — but didn't specify branch strategy.)
- [ ] **Whether to add an intermediate `[features.tree-sitter = false]` Cargo feature flag** that lets the migration be rolled back without a release. (This contradicts "big-bang" but is a safety valve worth considering.)
- [ ] **Plugin version bump** for `tools/intellij-xs-plugin/gradle.properties:pluginVersion`. Policy says minor bump for new LSP features / coverage; this migration touches LSP behaviour, so minor bump is the minimum.

---

## 7. What this plan does NOT cover

- **XS language extensions.** If the migration is a precursor to adding new XS syntax (e.g. pattern matching, async), the grammar file becomes more complex; some of the PoC's `?t` tricks may need to be revisited. Out of scope.
- **Doxygen extractor changes.** The `doxygen_retail.7z` extraction pipeline is parser-independent and unaffected.
- **Workspace resolution improvements.** `Workspace` model is independent of the parser; the migration does not touch it.
- **The three dev-only bins** (`day1_probe.rs`, `dump_top_level.rs`, `inspect_tree.rs`) are likely rewritten or deleted; the user may want to keep them as tree-sitter-comparison tools. Decision deferred.

---

## 8. References

- `tools/xs-language-server/lelwel-xs/README.md` — current PoC coverage table
- `tools/xs-language-server/lelwel-xs/SURVEY.md` — Phase 0 output (to be created)
- `tools/xs-language-server/tree-sitter-xs/grammar.js` — current grammar of record (818 lines)
- `tools/xs-language-server/lsp/src/symbols.rs:557-649` — the ERROR-recovery helpers that become obsolete
- `tools/xs-language-server/lsp/tests/symbols_cleanup_repro.rs:113-127` — the assertion that inverts after migration
- `tools/xs-language-server/lsp/tests/game_folder_parse.rs` — the migration's hardest regression gate
- `tools/xs-language-server/lsp/AGENTS.md` — testing capabilities to update in Phase 8
- `docs/post-lsp-migration-issues.md` — context for the previous migration cycle (IntelliJ-side, not parser-side)
- `openspec/changes/archive/xs-language-server/` — historical SDD artifacts for the LSP project

---

## 9. Approval gate

**This plan does NOT begin implementation until the user explicitly approves it.** The plan answers the question "what would the migration look like?" — not "start the migration now?" Approval can be partial (e.g. "approve Phases 0-3, defer the rest") or full.