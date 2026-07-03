# lelwel-xs — Proof-of-Concept Lelwel Grammar for XS

This directory contains a proof-of-concept [lelwel](https://github.com/0x2a-42/lelwel) grammar
for the XS scripting language used by Age of Mythology: Retold's modding engine.

The grammar lives at [`src/xs.llw`](src/xs.llw). It is a translation of the existing
[tree-sitter XS grammar](../tree-sitter-xs/src/grammar.json) into lelwel's
LL(1) + extensions format, with the goal of answering the question:

> *What would it look like to migrate the LSP from tree-sitter to lelwel?*

The answer is: **feasible for the language surface, painful for the disambiguations.**
See the "Known limitations" section below for the details.

## How to try it

### Validate the grammar compiles

The sample Cargo project in this directory wraps `xs.llw` in a build script that
runs `lelwel::build()`. From this directory:

```sh
cargo build
```

Prerequisites:
- Rust **nightly** (lelwel 0.10.4 uses `let`-chains stabilized in 1.88+; this
  project's `rust-toolchain.toml` pins nightly).
- The project's `Cargo.toml` declares `lelwel = "0.10"` as a build-dependency.

On a clean run this produces a working library (5 dead-code warnings in
the generated `lexer.rs`, no errors).

### Author with feedback

```sh
cargo install --features=cli,lsp lelwel
lelwel-ls
```

Then open `src/xs.llw` in Neovim with the [nvim-lelwel](https://github.com/0x2a-42/nvim-lelwel)
plugin installed. The language server will surface any LL(1) conflicts the grammar
contains, and you can hover over `///` doc-comments to see each rule's intent.

## What is covered

| Feature                              | tree-sitter XS | lelwel-xs | Notes |
|--------------------------------------|----------------|-----------|-------|
| Primitive types (`void`/`int`/...)   | yes            | yes       | Identical keyword set |
| Storage / qualifier modifiers        | yes            | yes       | `extern`/`static`/`mutable`/`const`/`ref` |
| Top-level declarations               | yes            | yes       | `int gFoo = -1;` |
| Function definitions                 | yes            | yes       | `void foo() { ... }` |
| Function forward declarations        | **ERROR node** | **first-class** | The main fix vs tree-sitter — see below |
| Function pointer types               | yes            | yes       | `void(int) callback`; also `void() gFoo = lambda;` (function-pointer-typed variable) |
| Lambda expressions                   | yes            | partial   | `[]() -> bool { ... }` — no capture list |
| Class definitions                    | yes            | yes       | `class Foo { ... };` |
| Method definitions                   | yes            | yes       | Handled as `class_member` |
| Rule blocks                          | yes            | yes       | `rule name\nminInterval N\nactive\n{}` |
| `switch` / `case` / `default:`       | yes            | yes       | Found in spire_ai/core, added after first draft |
| `if` / `while` / `for`               | yes            | yes       | But only `for (i=0; ...)` — see limitations |
| Update expressions (`++` / `--`)     | yes            | yes       | Added after spotting them in human_assist.xs |
| Unary `-` / `!` / `~`                | yes            | yes       | |
| Binary precedence (Pratt-style)      | yes            | yes       | Direct-left-recursion with 6 levels |
| Comma operator                       | yes            | **no**    | Chained `a, b, c` not modeled — see limitations |
| `include "path.xs";`                 | yes            | yes       | |
| C preprocessor (`#include` etc.)     | yes            | no        | Requires a separate preprocessor pass |
| `do { } while()`                     | yes            | no        | Not used in AoM:R mod scripts |
| `goto` / labels                      | yes            | no        | Not used in AoM:R mod scripts |
| `typedef`                            | yes            | no        | Not used in AoM:R mod scripts |
| Pointer declarators (`*p`)           | yes            | no        | XS has no pointers |
| Vector literals (`vector(1, 2, 3)`)  | yes            | no        | Only `new vector(N, default)` is used in practice |
| Multi-declarator lists (`int a, b;`) | yes            | partial   | TODO marker; only single declarators parse |
| Function-pointer-typed *variables* (`void(int) callback = lambda;`) | yes | **no** | See "Known limitations" — would require ParserCallbacks or non-recursive type rules |
| Statements inside function bodies    | yes            | partial   | See "Known limitations" — relies on `?t` predicates to break an LL(1) tie between `declaration` and `statement`; precise precedence may not match every input |

## Known limitations

### 0. Compile-time fixes that landed here (lelwel PoC history)

The grammar file as it stands today is the result of an iterative fix loop against
`lelwel::build`. The fixes below were necessary to get `cargo build` to succeed:

- **`expression` named `expression`, not `expr`**: agent's first draft had
  `expr^: comma_expr` but 14 call sites referenced `expression`. Renamed to
  `expression`.
- **`Question='?'` token**: the ternary `cond ? then : else` references
  `'?`, which needs to be a declared token.
- **Optional trailing `;`**: agent used `';'?;` (PEG-style). lelwel uses
  `[X]` for optional, so changed to `[';'];`.
- **Nested ordered choice is forbidden** (E028): several rules had `/` in
  positions where lelwel disallows it. Fixed by switching to `|` where
  alternatives are LL(1)-distinct, by restructuring to split rules, or by
  using `?t` (constant-true semantic predicate) to suppress the LL(1) check.
- **Recursive first sets**: `function_pointer_type: type_specifier '(' ... ')'`
  recursed through `type_specifier`, making the rule's first set unsolvable.
  Made `function_pointer_type` non-recursive (uses `(primitive_type | Identifier)`
  directly). This drops the rare `BOSystem(int)` user-defined function-pointer
  return type from the grammar.
- **Direct-left-recursion in `comma_expr`**: the original
  `comma_expr: comma_expr ',' assignment_expr | assignment_expr` hit E012.
  Restructured but a chained-comma Kleene-star form hit E013. Final form is
  `comma_expr: assignment_expr` (a single-element chain); users must use
  semicolons to separate multiple expressions, losing
  `for (i = 0, j = 0; ...)` syntax.

### 1. The C typedef problem (LL(1) conflict)

XS inherits C's ambiguity: a token like `BOSystem` could be a *type* (in
`BOSystem boSystem;`) or a *value* (in `BOSystem.create();`). Resolving this
requires symbol-table knowledge that an LL(1) parser cannot have natively.

**lelwel solution:** ordered choice. The `top_level_item` and `block_item`
rules try `function_or_forward_declaration` and `declaration` before falling
back to `statement`. If the first branch fails to consume all the input, the
parser backtracks and tries the next. The function-pointer-typed declarator
is tried first inside `declarator` so that `void() gFoo = ...;` parses
correctly.

**tree-sitter solution:** explicit `conflicts` list in `grammar.json` (see
lines 4504-4579) plus the ERROR-node recovery in `src/symbols.rs:155-165`.

**C.llw solution:** semantic predicates (`?1`, `?2`) that consult a parser
context to ask "is the current token a known type name?".

The lelwel approach is the most fragile: a code snippet like `BOSystem
myBOSystem;` (a variable declaration with no initializer) is currently
parsed as two back-to-back expression statements (`BOSystem;` and
`myBOSystem;`), because the function-pointer declarator fails to match
(no `(` after `BOSystem`), the plain-identifier declarator matches `BOSystem`
alone, and the trailing `myBOSystem;` is left for the next iteration. A real
implementation would need either:

- A `ParserCallbacks` impl that tracks known types in the current scope
  (cleanest), or
- A pre-pass that injects a synthetic tag token before each identifier (ugly)

### 2. Function forward declarations (the *reason* for this grammar)

In the tree-sitter version, `void bar(int x = -1);` surfaces as an `ERROR`
node because the grammar has no rule for declarations without bodies. The
existing LSP works around this with `extract_error_forward_declaration` in
`src/symbols.rs:641-688`, which manually unpacks the type/identifier/parameters
from the error node.

In lelwel, forward declarations are first-class. The
`function_or_forward_declaration` rule factors out the common
`declaration_specifiers declarator` prefix and uses the next token (`{` or
`;`) to decide whether the result is a function definition or a forward
declaration. This is fully LL(1). Function-pointer-typed *variable*
declarations (`void(int) gFoo = ...;`) go through the regular `declaration`
rule (via the function-pointer declarator added to `declarator`).

### 3. Lambdas

`[](int x) -> bool { return x > 0; }` is supported, but the *capture list*
`[x, &y]` is not modeled. Real-world XS code in this repo always writes
lambdas with an empty capture list `[]` (e.g.
`bo_system_internal.xs:108-109`), so this is acceptable for the actual
scripts we care about.

### 4. Vector literals

XS supports both `new vector(N, default)` and the C-style literal
`vector(1, 2, 3)`. The first form is the only one used in this repo's
mods, so the literal form is not modeled. Adding it would be a
straightforward `vector` keyword + parens + comma list.

### 5. `new` expressions

`new` only appears as `new int[N]` or `new vector(N, default)` in real
code, so the `new` rule accepts any `type_specifier` + argument list and
does not enforce the "type must be a primitive + dimension" form. This
matches tree-sitter's behavior.

### 6. `static` class members

`static` is in the storage-class list. In real XS code, `static` is only
ever used inside a `class` body (e.g.
`static int[] areasToScout = default;`), but the grammar does not enforce
this restriction.

### 7. Semantic actions / predicates are not wired up

A real lelwel build would generate a `ParserCallbacks` trait that the
parser invokes for each `?1`, `#1`, `!1` site. This grammar uses none of
those — every disambiguation is done with ordered choice and the commit
operator `~` (implicit via the C-style rule structure). The generated
Rust code would need to be hand-edited to add the symbol-table tracking
described in limitation #1.

## What would it take to migrate the LSP for real?

Roughly:

1. **Write the lexer.rs skeleton.** The Logos regexes for `Identifier`,
   `IntConst`, `FloatConst`, `StringLiteral`, and the keyword set must be
   hand-written. This is the most error-prone part — regex mistakes will
   produce silent mislexes.

2. **Add semantic actions to disambiguate types vs expressions.** As
   noted in limitation #1, ordered choice alone is not enough. A
   `ParserCallbacks` impl that tracks the symbol table (typedefs,
   classes, current class owner) is the only correct solution.

3. **Port the symbol-table extraction.** The existing
   `src/symbols.rs:140-167` and the ERROR-node recovery in
   `src/symbols.rs:641-688` would become mostly obsolete once
   forward declarations parse cleanly. The tree visitor that walks the
   CST would need to be re-targeted at lelwel's CST shape (which is
   different from tree-sitter's — lelwel produces a *lossless* CST
   including whitespace and comments).

4. **Re-implement semantic tokens, hover, completion.** These features
   all consume the CST; porting them is mechanical but tedious.

5. **Re-write the integration tests** under
   `tools/xs-language-server/tests/` to drive the lelwel parser.

The net effect of the migration, if successful, would be:

- **Pro:** error resilience. Lelwel's recovery is genuinely better than
  tree-sitter's for partial / malformed input. The grammar's recovery
  sets are computed automatically from the grammar structure.
- **Pro:** clearer CST. Lossless trees are easier to reason about than
  tree-sitter's anonymous nodes.
- **Con:** the symbol-table work moves from `symbols.rs` into the
  grammar (via `ParserCallbacks`). You trade a Rust visitor for a
  grammar that has to know about types.

## File layout

```
lelwel-xs/
├── README.md         # this file
└── src/
    └── xs.llw        # the grammar
```

The directory is gitignored at the project root — this is exploratory
and we do not want it landing in the main repo.
