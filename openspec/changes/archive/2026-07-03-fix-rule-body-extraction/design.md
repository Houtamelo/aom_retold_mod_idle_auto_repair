# Design: Fix rule-body extraction in the lelwel XS parser

## 1. Architecture overview

The typed AST already knows how to extract a `RuleDefinition` from a clean CST node. The problem is that the grammar's `block_item^` currently forces declaration-style alternatives first using the always-true `?t` predicate and **non-backtracking** `|` alternation, so a local declaration such as `int x = 1;` is first tried as a function definition, fails, and produces ERROR nodes. A rule body full of ERROR nodes causes `RuleDefinition::from_cst` to return `None`.

The fix introduces a small `TypeTable` parser context and uses it in a semantic predicate that answers "does the current token start a declaration?". The predicate is applied to the declaration/forward-declaration/function-definition branches of `block_item^`, and the alternation is switched to ordered choice (`/`) so the parser can backtrack from `declaration`/`forward_declaration_rule` into `function_definition_rule` when the body item really is a nested function definition.

```text
                    .xs source
                         |
                         v
              lelwel::Parser::new_with_context
                    Context = TypeTable
                         |
                         v
            block_item^ dispatch
      /?is_type declaration            \
      /?is_type forward_declaration_rule \  ordered choice +
      /?is_type function_definition_rule /  TypeTable predicate
      / statement                        /
                         |
                         v
            clean RuleDefinition CST node
                         |
                         v
            RuleDefinition::from_cst -> Some(_)
```

`TypeTable` lives in `ast/` (re-exported through `xs_parser::ast`) so Phase 4 can build one from the engine API + workspace class list and pass it to `Parser::new_with_context`.

## 2. Data model

```rust
// tools/xs-language-server/lelwel-xs/src/ast/type_table.rs
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct TypeTable {
    /// Built-in type keywords: void, int, bool, float, string, vector.
    pub primitives: HashSet<&'static str>,
    /// Caller-supplied class / user-defined type names.
    pub classes: HashSet<String>,
}

impl TypeTable {
    pub fn with_primitives() -> Self {
        Self {
            primitives: [
                "void", "int", "bool", "float", "string", "vector",
            ]
            .into_iter()
            .collect(),
            classes: HashSet::new(),
        }
    }

    pub fn insert_class(&mut self, name: &str) {
        self.classes.insert(name.to_owned());
    }

    pub fn is_type(&self, name: &str) -> bool {
        self.primitives.contains(name) || self.classes.contains(name)
    }
}
```

`Default` delegates to `with_primitives`, so `Parser::new` continues to work for callers that do not know about class names yet.

Parser callback wiring:

```rust
// tools/xs-language-server/lelwel-xs/src/parser.rs
impl<'a> ParserCallbacks<'a> for Parser<'a> {
    type Diagnostic = Diagnostic;
    type Context = TypeTable;          // was ()

    // ... create_tokens / create_diagnostic unchanged ...

    // lelwel 0.10.4 only accepts numeric/?t predicates in .llw files,
    // so the grammar uses `?1`; the generated method is therefore:
    fn predicate_block_item_1(&self) -> bool {
        let text = match self.current {
            Token::Identifier => &self.cst.source()[self.span()],
            Token::Void => "void",
            Token::Int => "int",
            Token::Bool => "bool",
            Token::Float => "float",
            Token::StringKw => "string",
            Token::Vector => "vector",
            // Qualifiers never start an expression statement in XS.
            Token::Extern | Token::Static | Token::Const
            | Token::Mutable | Token::Ref => return true,
            _ => return false,
        };
        self.context.is_type(text)
    }
}
```

`re-export` in `ast/mod.rs`:

```rust
pub use crate::parser::{Cst, CstChildren, Node, NodeRef, Rule, Span, TypeTable};
```

## 3. Grammar change

File: `tools/xs-language-server/lelwel-xs/src/xs.llw`, lines 573-577.

```diff
 block_item^:
-  ?t function_definition_rule
-| ?t forward_declaration_rule
-| ?t declaration
-| statement
+  ?1 declaration
+ / ?1 forward_declaration_rule
+ / ?1 function_definition_rule
+ / statement
 ;
```

`?1` is the lelwel 0.10.4 encoding for the `is_type` semantic predicate. The predicate looks at the **current token** of the branch:

* Identifier tokens are looked up as-is in `TypeTable::classes` (and, because `HashSet<&str>` borrows `str`, in `TypeTable::primitives` as well).
* The primitive keyword tokens `void`, `int`, `bool`, `float`, `string`, `vector` are mapped to their string names and then checked through `TypeTable::is_type`.
* The declaration-qualifier keywords `extern`, `static`, `const`, `mutable`, `ref` are treated as declaration-start tokens without a table lookup.

Switching from `|` to `/` enables lelwel's ordered-choice backtracking, so a token sequence that begins with a known type can still be a function definition (e.g. `int foo() {}` inside a block) without mis-parsing as a declaration.

## 4. Module / file changes

| File | Action | Description |
|------|--------|-------------|
| `tools/xs-language-server/lelwel-xs/src/xs.llw` | Modify | Replace `?t` with `?1` on `block_item^` declaration branches; switch to ordered choice and put declaration first. |
| `tools/xs-language-server/lelwel-xs/src/ast/type_table.rs` | Create | New `TypeTable` struct with primitive seeding, class insertion, and `is_type` lookup. |
| `tools/xs-language-server/lelwel-xs/src/ast/mod.rs` | Modify | Re-export `TypeTable` so Phase 4 can build parser contexts from `xs_parser::ast`. |
| `tools/xs-language-server/lelwel-xs/src/parser.rs` | Modify | Change `ParserCallbacks::Context` from `()` to `TypeTable`; implement `predicate_block_item_1`. |
| `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` | Modify | Add `rule_definition_*` tests for non-empty bodies and a context-aware parse helper. |

## 5. Compatibility / regression strategy

* Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`. The existing 159 `lelwel-xs` unit tests must stay green.
* Run `cargo run --manifest-path tools/xs-language-server/lelwel-xs/Cargo.toml --example test_parse`; the example currently emits 15 diagnostics for local rule/function-body declarations and should drop to 0.
* Sample a retail rule-body fragment from `game/ai/core/economic_units.xs::deleteExcessGatherers` (e.g. the `int queryID = ...; int numResults = ...;` block) and parse it with `TypeTable::default()`. Assert zero rule-level ERROR nodes.
* The change is intentionally scoped to `block_item^`; `top_level_item^` keeps its existing `/` ordered choice and `?t` semantics, so file-scope parsing is untouched.

## 6. Test strategy

Tests are added next to the existing `RuleDefinition` tests in `src/ast/top_level.rs`.

| Scenario | TypeTable | Assertion |
|----------|-----------|-----------|
| S-RBE-01 empty body | `default()` | `RuleDefinition` extracts with 0 body items |
| S-RBE-02 `rule r { x; }` | `default()` | 1 `BlockItem::Statement` |
| S-RBE-03 `rule r { int x = 1; }` | `default()` | 1 `BlockItem::Declaration` |
| S-RBE-04 `rule r { MyClass x; }` | `with_primitives()` + `insert_class("MyClass")` | 1 `BlockItem::Declaration` |
| S-RBE-05 mixed body | `default()` | declaration followed by statement |
| S-RBE-06 retail snippet | `default()` | rule node has 0 ERROR children |
| S-RBE-07 empty table `int x;` | empty `TypeTable { primitives: empty, classes: empty }` | extracts as `BlockItem::Statement`, not `Declaration` |
| S-RBE-08 unknown class `MyClass x;` | `default()` | extracts as `BlockItem::Statement`, not `Declaration` |

A small helper is added in the test module:

```rust
fn parse_with_types(source: &str, mut ctx: TypeTable) -> Cst<'_> {
    let mut diags = vec![];
    Parser::new_with_context(source, &mut diags, ctx).parse(&mut diags)
}
```

## 7. Migration / rollout

`TypeTable` is a new public type. Phase 4 integration uses only these APIs:

```rust
use xs_parser::ast::TypeTable;

let mut types = TypeTable::with_primitives();
for class_name in workspace_class_names {
    types.insert_class(class_name);
}

let mut diags = Vec::new();
let cst = Parser::new_with_context(source, &mut diags, types).parse(&mut diags);
```

`Parser::new` remains available and uses `TypeTable::default()` (primitive keywords only). No existing call sites need to change until Phase 4 explicitly wants class-aware parsing.

## 8. Open questions / risks

1. **lelwel 0.10.4 predicate syntax is numeric only.** The design uses `?1` and the generated callback `predicate_block_item_1`. If a future lelwel version supports named predicates, the grammar can be updated to `?is_type` and the method renamed, but today `?is_type` will not compile.
2. **Qualifier handling.** `extern`, `static`, `const`, `mutable`, and `ref` are hard-coded as declaration-start tokens. This is correct for XS, but if any of those keywords ever appears as a value/expression, the predicate would mis-classify the branch.
3. **`for (int i = 0; ...)` is still unsupported.** The sampled retail test for S-RBE-06 must stop before the `for`-loop line in `deleteExcessGatherers`; full rule-body parsing requires Limitation 2.
4. **Class names outside the table.** Until Phase 4 populates `TypeTable.classes`, class-typed local declarations (`BOSystem myBOSystem;`) will be parsed as statements. This is the intended degrading behaviour, but it means retail verification must either seed known classes or restrict samples to primitive types.
5. **Ordered-choice backtracking performance.** Local backtracking over a few declaration branches is bounded to the length of one declarator, so the 50 ms / 5,000-line budget is safe.
