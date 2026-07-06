# Typed AST for the XS LSP — Design & Implementation Plan

**Status:** Design phase, ready for implementation.
**Target file:** `tools/xs-language-server/lelwel-xs/src/ast/mod.rs` (existing file, currently ~73 lines of placeholder code).
**Goal:** Convert the lelwel-generated `Cst<'a>` into typed Rust structs per the user's existing `ast/mod.rs` pattern (rich-type / eager / wrapper-tokens). Support formatting from day one.

---

## 1. Why rich-types (the user confirmed)

User explicit motivation: **"I do actually want to support formatting."** That requires preserving token positions through the AST — wrapper-token types (`Paren`, `Brace`, `Comma`, etc.) are the natural way to do this.

Trade-off vs the lazy C.llw pattern:
- **Allocation per node.** Each `cast()` builds the entire struct, allocating ~one wrapper per CST node. Cost is small for typical XS files (< 30k nodes) but non-zero.
- **Token positions preserved.** `(`, `)`, `,`, `{`, `}` are stored alongside the inner data so a formatter can round-trip the source.
- **Direct field access.** `param.t_ref`, `param.ty`, `param.ident` — no `cst.children()` to walk on hot path.

---

## 2. Core types

### 2.1 `Spanned<T>` — value with source position

```rust
pub struct Spanned<T> {
    pub node: T,        // or `pub value` — decided during Pass 1
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self { Self { node, span } }
    pub fn span(&self) -> Span { self.span }
    pub fn into_inner(self) -> T { self.node }
}
```

Used for token+position pairs (`Spanned<Identifier>` is just `(&'src str, Span)`).

### 2.2 Wrapper-token types

Tokens (parentheses, braces, etc.) are represented as zero-sized structs that exist purely for typed access. The actual `Token` (from lexer) and `Span` are pulled at `cast()` time:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Paren;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct LBrace;
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct RBrace;
// ... LBrak, RBrak, Semi, Comma, Arrow, Question, Dot, Colon ...

impl Paren {
    pub fn from_cst(cst: &Cst, parent: NodeRef) -> Option<Span> {
        cst.children(parent)
            .find_map(|c| cst.match_token(c, Token::LPar))
    }
}
```

Single `from_cst` per wrapper, returns the span. Zero allocation.

### 2.3 Wrapper collection types — preserve token positions

```rust
pub struct Parenthesized<T> {
    pub open: Span,         // span of the `(`
    pub inner: T,
    pub close: Span,        // span of the `)`
}

pub struct Braced<T> {
    pub open: Span,
    pub inner: T,
    pub close: Span,
}

pub struct Bracketed<T> {
    pub open: Span,
    pub inner: T,
    pub close: Span,
}

/// Inner = list of (item, trailing-separator-span) pairs. The separator is
/// `None` for the last item (no trailing comma).
pub struct CommaSeparatedList<T> {
    pub items: Vec<(T, Option<Span>)>,
}

/// Like CommaSeparatedList but uses `;` (not used much in XS, included
/// for parity with the user's ast/mod.rs).
pub struct SemiColonSeparatedList<T> {
    pub items: Vec<(T, Option<Span>)>,
}

impl<T> CommaSeparatedList<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(t, _)| t)
    }
    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter().map(|(t, _)| t)
    }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
    pub fn len(&self) -> usize { self.items.len() }
}
```

Format-preserving output: the formatter iterates items + separators, emitting them in the original order with original spans (no reformatting of whitespace, comments, etc., unless specifically requested).

### 2.4 `TokenSpan` — already in the user's existing file; extend

```rust
pub struct TokenSpan {
    pub token: Token,
    pub span:  Span,
}
```

Add constructors: `TokenSpan::from_cst(cst, parent) -> Option<TokenSpan>` that pulls the first matching token. Useful for diagnostic positions on rules that have a specific leading token.

---

## 3. AST node types (full catalogue)

For each rule in xs.llw that has user-meaningful structure, a struct. Sum types for alternatives.

### 3.1 Top-level

```rust
pub type Identifier = Spanned<String>;  // borrowed via DSO later; for now owned

pub enum TopLevelItem {
    PreprocIf(PreprocIf),
    PreprocElif(PreprocElif),
    PreprocElse(PreprocElse),
    PreprocEndif(PreprocEndif),
    PreprocDef(PreprocDef),
    IncludeDirective(IncludeDirective),
    FunctionDefinition(FunctionDefinitionRule),
    ForwardDeclaration(ForwardDeclarationRule),
    ClassDefinition(ClassDefinition),
    RuleDefinition(RuleDefinition),
    Declaration(Declaration),
}
```

### 3.2 Type system

```rust
pub enum Type {
    Void,
    Int,
    Bool,
    Float,
    String,
    Vector,
    Class(Identifier),   // user-defined type name
}

pub struct TypeSpecifier {
    pub ty: Type,
    pub span: Span,
}

pub struct DeclarationSpecifiers {
    pub storage: Vec<(StorageClassSpecifier, Span)>,
    pub ty: TypeSpecifier,
    pub type_quals: Vec<(TypeQualifier, Span)>,
    /// Scope (start..end) of the whole DeclarationSpecifiers node
    pub span: Span,
}

pub enum StorageClassSpecifier { Extern, Static, Mutable }
pub enum TypeQualifier { Const, Ref }
```

### 3.3 Declarations

```rust
pub struct Declaration {
    pub decl_specs: DeclarationSpecifiers,
    pub init_declarator_list: InitDeclaratorList,
    pub semi: Span,
    pub span: Span,
}

pub struct InitDeclaratorList {
    pub items: Vec<InitDeclarator>,
}

pub struct InitDeclarator {
    pub declarator: Declarator,
    pub initializer: Option<(Span, Expr)>,   // (`=`, rhs)
    pub span: Span,
}

pub struct Declarator {
    pub direct: DirectDeclarator,
    pub span: Span,
}

pub enum DirectDeclarator {
    FunctionDeclarator(FunctionDeclarator),
    IdentDeclarator(IdentDeclarator),
    ParenDeclarator(ParenDeclarator),
    ArrayDeclarator(ArrayDeclarator),
}

pub struct FunctionDeclarator {
    pub base: Box<DirectDeclarator>,
    pub params: ParameterList,
    pub close: Span,  // `)`
    pub span: Span,
}

pub struct IdentDeclarator {
    pub name: Identifier,
    pub span: Span,
}

pub struct ParenDeclarator {
    pub inner: Box<Declarator>,
    pub span: Span,
}

pub struct ArrayDeclarator {
    pub base: Box<DirectDeclarator>,
    /// Always empty `[]` in XS; the production is `array_or_primitive_type`.
    /// Could store the optional subscript expression here if XS later grows it.
    pub span: Span,
}
```

### 3.4 Parameters (mirrors the user's existing Param struct)

```rust
pub struct ParameterDeclaration {
    pub decl_specs: DeclarationSpecifiers,
    pub inner: ParameterInner,
    pub span: Span,
}

pub enum ParameterInner {
    FunctionPointerParam(FunctionPointerParam),
    RegularParam(RegularParam),
}

pub struct FunctionPointerParam {
    pub fn_type: (TypeSpecifier, Parenthesized<ParameterList>),
    pub name: Identifier,
    pub default: Option<(Span, Expr)>,
    pub span: Span,
}

pub struct RegularParam {
    pub name: Identifier,
    pub default: Option<(Span, Expr)>,
    pub span: Span,
}

pub struct ParameterList {
    pub items: CommaSeparatedList<ParameterDeclaration>,
    pub span: Span,
}
```

### 3.5 Top-level items

```rust
pub struct FunctionDefinitionRule {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub body: Braced<BlockItemList>,
    pub span: Span,
}

pub struct ForwardDeclarationRule {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub semi: Span,
    pub span: Span,
}

pub struct ClassDefinition {
    pub name: Identifier,
    pub members: Braced<Vec<ClassMember>>,
    pub semi: Span,
    pub span: Span,
}

pub enum ClassMember {
    FunctionDefinition(FunctionDefinitionRule),
    ForwardDeclaration(ForwardDeclarationRule),
    FieldDeclaration(FieldDeclaration),
}

pub struct FieldDeclaration {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub initializer: Option<(Span, Expr)>,
    pub semi: Span,
    pub span: Span,
}

pub struct RuleDefinition {
    pub name: Identifier,
    pub modifiers: Vec<RuleModifier>,
    pub body: Braced<BlockItemList>,
    pub span: Span,
}

pub enum RuleModifier {
    MinInterval(Spanned<i32>),  // `minInterval N`
    MaxInterval(Spanned<i32>),
    MinIntervalMS(Spanned<i32>),
    MaxIntervalMS(Spanned<i32>),
    Priority(Spanned<i32>),
    HighFrequency,
    Active,
    Inactive,
    RunImmediately,
    Group(Identifier),
}

pub struct IncludeDirective {
    pub path: Spanned<String>,   // the string literal text
    pub semi: Span,
    pub span: Span,
}
```

### 3.6 C preprocessor

```rust
pub struct PreprocDef {
    pub name: Identifier,
    pub span: Span,
}

pub struct PreprocIf {
    pub cond: PreprocExpr,
    pub span: Span,
}

pub struct PreprocElif {
    pub cond: PreprocExpr,
    pub span: Span,
}
pub struct PreprocElse { pub span: Span }
pub struct PreprocEndif { pub span: Span }

pub enum PreprocExpr {
    Or(Box<PreprocExpr>, Box<PreprocExpr>),       // a || b
    And(Box<PreprocExpr>, Box<PreprocExpr>),      // a && b
    Eq(Box<PreprocExpr>, EqOp, Box<PreprocExpr>), // a == b, a != b
    Relational(Box<PreprocExpr>, RelOp, Box<PreprocExpr>),
    Additive(Box<PreprocExpr>, AddOp, Box<PreprocExpr>),
    Multiplicative(Box<PreprocExpr>, MulOp, Box<PreprocExpr>),
    Not(Box<PreprocExpr>),
    Defined(Identifier),
    Paren(Box<PreprocExpr>),
    IntConst(Spanned<String>),    // raw lexeme, narrow later
    FloatConst(Spanned<String>),
    Identifier(Identifier),
}

pub enum EqOp { Eq, Neq }
pub enum RelOp { Lt, Gt, Leq, Geq }
pub enum AddOp { Plus, Minus }
pub enum MulOp { Times, Div, Mod }
```

Note: `PreprocExpr` is intentionally a separate hierarchy from the regular `Expr` — the preprocessor doesn't have function calls, identifiers-as-values, etc.

### 3.7 Statements

```rust
pub enum BlockItem {
    Declaration(Declaration),
    ForwardDeclaration(ForwardDeclarationRule),
    FunctionDefinition(FunctionDefinitionRule),
    Statement(Statement),
}

pub type BlockItemList = Vec<BlockItem>;

pub enum Statement {
    If(IfStatement),
    While(WhileStatement),
    For(ForStatement),
    Switch(SwitchStatement),
    Return(ReturnStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Compound(CompoundStatement),
    Expression(ExpressionStatement),
}

pub struct CompoundStatement {
    pub items: Braced<BlockItemList>,
    pub span: Span,
}

pub struct IfStatement {
    pub cond: Parenthesized<Expr>,
    pub then_branch: Box<Statement>,
    pub else_branch: Option<Box<Statement>>,
    pub span: Span,
}

pub struct WhileStatement {
    pub cond: Parenthesized<Expr>,
    pub body: Box<Statement>,
    pub span: Span,
}

pub struct ForStatement {
    pub init: ForInit,
    pub cond: Option<Expr>,
    pub update: Option<Expr>,
    pub body: Box<Statement>,
    pub span: Span,
}

pub enum ForInit {
    Declaration(Declaration),    // x86-future: requires Limitation 2A
    Expression(Option<Expr>),    // current PoC behavior
    Empty,
}

pub struct SwitchStatement {
    pub cond: Parenthesized<Expr>,
    pub body: Braced<Vec<SwitchCase>>,
    pub span: Span,
}

pub struct SwitchCase {
    pub label: SwitchLabel,
    pub body: CompoundStatement,
    pub span: Span,
}

pub enum SwitchLabel {
    Case(Expr),
    Default,
}

pub struct ReturnStatement {
    pub value: Option<Expr>,
    pub semi: Span,
    pub span: Span,
}

pub struct BreakStatement { pub semi: Span, pub span: Span }
pub struct ContinueStatement { pub semi: Span, pub span: Span }

pub struct ExpressionStatement {
    pub expr: Option<Expr>,
    pub semi: Span,
    pub span: Span,
}
```

### 3.8 Expressions

This is the largest category. Mirrors the user's existing patterns but extended for our grammar.

```rust
pub enum Expr {
    Int(IntLiteral),
    Float(FloatLiteral),
    String(StringLiteral),
    True(TrueLiteral),
    False(FalseLiteral),
    Null(NullLiteral),
    Identifier(IdentifierExpr),
    Paren(ParenExpr),
    Lambda(LambdaExpr),
    New(NewExpr),
    Default(DefaultExpr),
    Vector(VectorLiteral),
    Unary(UnaryExpr),
    Postfix(PostfixExpr),
    Binary(BinaryExpr),
    Conditional(ConditionalExpr),
    Assignment(AssignmentExpr),
    Comma(CommaExpr),
}

pub struct IntLiteral { pub value: i64, pub span: Span }
pub struct FloatLiteral { pub value: f64, pub span: Span }
pub struct StringLiteral { pub value: String, pub span: Span }
pub struct TrueLiteral { pub span: Span }
pub struct FalseLiteral { pub span: Span }
pub struct NullLiteral { pub span: Span }

pub struct IdentifierExpr { pub name: Identifier, pub span: Span }
pub struct ParenExpr { pub inner: Box<Expr>, pub span: Span }

pub struct LambdaExpr {
    pub params: Parenthesized<ParameterList>,
    pub return_ty: Option<Spanned<TypeSpecifier>>,
    pub body: CompoundStatement,
    pub span: Span,
}

pub struct NewExpr {
    pub ty: TypeSpecifier,
    pub args: Parenthesized<ArgumentList>,
    pub span: Span,
}

pub struct DefaultExpr { pub span: Span }

pub struct VectorLiteral {
    pub args: Parenthesized<ArgumentList>,
    pub span: Span,
}

pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

pub enum UnaryOp {
    PreInc,    // ++x
    PreDec,    // --x
    Minus,     // -x
    Not,       // !x
    Tilde,     // ~x
}

pub struct PostfixExpr {
    pub inner: Box<PostfixInner>,
    pub span: Span,
}

pub enum PostfixInner {
    Primary(PrimaryExpr),
    Call(CallExpr),
    Field(FieldExpr),
    Subscript(SubscriptExpr),
    PostInc(PostIncExpr),
    PostDec(PostDecExpr),
}

pub enum PrimaryExpr {
    Identifier(IdentifierExpr),
    Int(IntLiteral),
    // ... etc — same as Expr minus the postfix-able variants
}

pub struct CallExpr {
    pub callee: Box<PostfixExpr>,
    pub args: Parenthesized<ArgumentList>,
    pub span: Span,
}

pub struct FieldExpr {
    pub base: Box<PostfixExpr>,
    pub name: Identifier,
    pub span: Span,
}

pub struct SubscriptExpr {
    pub base: Box<PostfixExpr>,
    pub index: Bracketed<Expr>,
    pub span: Span,
}

pub struct PostIncExpr { pub base: Box<PostfixExpr>, pub span: Span }
pub struct PostDecExpr { pub base: Box<PostfixExpr>, pub span: Span }

pub struct BinaryExpr {
    pub lhs: Box<Expr>,
    pub op: BinaryOp,
    pub rhs: Box<Expr>,
    pub span: Span,
}

pub enum BinaryOp {
    LogicalOr,
    LogicalAnd,
    Eq, Neq,
    Lt, Gt, Leq, Geq,
    Plus, Minus,
    Times, Div, Mod,
}

pub struct ConditionalExpr {
    pub cond: Box<Expr>,
    pub then_branch: Option<Box<Expr>>,
    pub else_branch: Box<Expr>,
    pub span: Span,
}

pub struct AssignmentExpr {
    pub lhs: Box<Expr>,
    pub op: AssignmentOp,
    pub rhs: Box<Expr>,
    pub span: Span,
}

pub enum AssignmentOp { Assign, PlusAssign, MinusAssign, TimesAssign, DivAssign, ModAssign }

pub struct CommaExpr {
    pub first: Box<Expr>,
    pub rest: Vec<Expr>,   // trailing commas not allowed (XS doesn't use comma op)
    pub span: Span,
}

pub struct ArgumentList {
    pub items: CommaSeparatedList<Argument>,
    pub span: Span,
}

pub struct Argument {
    pub expr: Expr,
    pub span: Span,
}
```

---

## 4. The `cast()` extraction pattern

The C.llw example uses `cast(cst, NodeRef) -> Option<Self>` plus field accessor methods. For the eager rich-types pattern, we need a slightly different approach: `cast()` builds the entire struct in one pass, recursively.

### 4.1 Helper functions (in `cst_helpers.rs`)

```rust
/// Walks children once and returns the unique child matching `rule`.
/// Errors if multiple match.
pub fn child_by_rule<'a>(
    cst: &'a Cst,
    parent: NodeRef,
    rule: Rule,
) -> Option<NodeRef>;

/// Returns all children that match the rule (zero or more).
pub fn children_by_rule<'a>(
    cst: &'a Cst,
    parent: NodeRef,
    rule: Rule,
) -> Vec<NodeRef>;

/// Returns the unique child token matching `token`, if any.
pub fn child_by_token(cst: &Cst, parent: NodeRef, token: Token) -> Option<Span>;

/// Returns the unique child node of any kind, matching by its `Rule` discriminant.
pub fn only_child(cst: &Cst, parent: NodeRef) -> Option<NodeRef>;
```

### 4.2 `from_cst` constructors per type

Each AST type gets a single `from_cst(cst, NodeRef) -> Option<Self>` constructor that:
1. Checks the input NodeRef matches the expected rule
2. Walks children once to extract all fields
3. Returns `None` on any shape mismatch

```rust
impl Declaration {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::Declaration) { return None; }
        let decl_specs = cst.children(node).find_map(|c| {
            DeclarationSpecifiers::from_cst(cst, c)
        })?;
        let init_declarator_list = cst.children(node).find_map(|c| {
            InitDeclaratorList::from_cst(cst, c)
        })?;
        let semi = cst.children(node)
            .find_map(|c| cst.match_token(c, Token::Semi))?;
        let span = cst.span(node);
        Some(Self { decl_specs, init_declarator_list, semi, span })
    }
}
```

This pattern is repeated for every AST type. With ~50 types, it's ~50 of these methods + the field-accessor boilerplate.

### 4.3 The `ast!` macro

To reduce boilerplate, a macro similar to C.llw's `ast_node!`:

```rust
macro_rules! ast_simple {
    ($name:ident, $rule:ident, |$cst:ident, $node:ident| $body:expr) => {
        impl $name {
            pub fn from_cst($cst: &Cst, $node: NodeRef) -> Option<Self> {
                if !$cst.match_rule($node, Rule::$rule) {
                    return None;
                }
                Some($body)
            }
            pub fn span(&self, cst: &Cst) -> Span { cst.span(self.syntax) }
            pub fn syntax(&self) -> NodeRef { self.syntax }
        }
    };
}
```

Even with the macro, each type still needs its extraction body, but the boilerplate around it is gone.

---

## 5. Storage strategy

### 5.1 Owned vs borrowed `String`s

The current `ast/mod.rs` uses `pub struct Identifier;` (a newtype around `String`). For an LSP that may parse many files, borrowed strings would be ideal.

**Decision (deferred):** Owned strings for the first pass (matches user's existing pattern). Later, introduce a `'src` lifetime once we know what the LSP actually needs to hold across requests.

```rust
pub type Identifier = Spanned<String>;  // owned for now
```

### 5.2 Span attachment

Three options, ranked:

1. **Spans on wrapper tokens only** (user's pattern): the formatter reconstructs source by emitting tokens at their original positions.
2. **Span on every node**: each AST struct carries `span: Span`. Useful for diagnostics (`go-to-def`, hover) to know where they are in source.
3. **Both**: wrapper tokens preserve formatting; each node carries its own span for diagnostics.

**Decision:** Option 3. It's the most useful without being much more expensive (a `Span` is just two `usize`s).

The formatter reproduces source by emitting tokens at their stored positions. The LSP uses node-level spans for diagnostic range mapping.

### 5.3 Avoiding allocations in hot paths

For format-preserving output (Option 1's main benefit), the formatter iterates `CommaSeparatedList<T>`, alternating `(item, separator?)`. For `T` to be a `Copy` reference type (like `&Declaration`), the items must be small. If we own `Declaration`, the items are large; the list would re-allocate or wrap in `Box`.

**Decision:** Use `Vec<(T, Option<Span>)>` for owned items. The span is `Copy` (`Span` is two `usize`s). For the formatter's iteration, just allocate the list once at parse time.

---

## 6. Implementation pass plan (matches what I proposed)

### Pass 1 — Foundation + declarations (~700 lines, one session)

1. `cst_helpers.rs`: `child_by_rule`, `child_by_token`, `only_child` (and tests)
2. `tokens.rs`: `Paren`, `Brace`, `Bracket`, `Comma`, `Semi`, `Eq`, `Ref`, `Question`, `Arrow`, `Dot`, etc., all `Copy + Debug + PartialEq + Hash` zero-sized types with `from_cst` constructors
3. `spanned.rs`: `Spanned<T>`, plus `ComaSeparatedList<T>`, `SemiColonSeparatedList<T>`, `Parenthesized<T>`, `Braced<T>`, `Bracketed<T>`
4. `type_system.rs`: `Type`, `TypeSpecifier`, `PrimitiveType`, `ArrayOrPrimitiveType`, `DeclarationSpecifiers`, `StorageClassSpecifier`, `TypeQualifier` wrappers
5. `declaration.rs`: `Declaration`, `InitDeclaratorList`, `InitDeclarator`, `Declarator`, `DirectDeclarator` (sum), `FunctionDeclarator`, `IdentDeclarator`, `ParenDeclarator`, `ArrayDeclarator`, `ParameterDeclaration`, `ParameterInner`, `FunctionPointerParam`, `RegularParam`, `ParameterList`
6. Tests: parse sample declarations, verify AST shape, verify format-preserving output for one example

### Pass 2 — Top-level items + preprocessor + statements (~800 lines, one session)

1. `top_level.rs`: `TranslationUnit`, `TopLevelItem` (sum of 11 variants), `ClassDefinition`, `ClassMember`, `RuleDefinition`, `RuleModifier`, `FieldDeclaration`, `FunctionDefinitionRule`, `ForwardDeclarationRule`, `IncludeDirective`
2. `preproc.rs`: `PreprocDef`, `PreprocIf`, `PreprocElif`, `PreprocElse`, `PreprocEndif`, `PreprocExpr`, `EqOp`, `RelOp`, `AddOp`, `MulOp`
3. `statement.rs`: `Statement` (sum of 9 variants), `BlockItem`, `CompoundStatement`, `IfStatement`, `WhileStatement`, `ForStatement`, `ForInit`, `SwitchStatement`, `SwitchCase`, `SwitchLabel`, `ReturnStatement`, `BreakStatement`, `ContinueStatement`, `ExpressionStatement`
4. Tests: full retail `bo_system.xs` forEach, verify AST shape matches; verify format-preserving on one function

### Pass 3 — Expressions + parameters + integration tests (~800 lines, one session)

1. `expr.rs`: ~30 expression-related wrappers (literals, primary, unary, postfix, binary, conditional, assignment, comma, lambda, new, default, vector)
2. `parameter.rs`: already done in Pass 1; integration here
3. `argument.rs`: `Argument`, `ArgumentList`
4. `format.rs`: proof-of-concept formatter that emits source unchanged for one full input file
5. Tests: round-trip 5 retail files; verify formatter reproduces source byte-for-byte (modulo known whitespace differences)

Each pass is a session's worth of work — breaks naturally at the LSP use-case level (Pass 1 enables symbol extraction; Pass 2 enables hover/completion; Pass 3 enables type-checking + formatting).

---

## 7. Test strategy

For each AST type, 3-5 tests:

```rust
#[test]
fn parses_simple_function_definition() {
    let source = "void bar() {}";
    let cst = parse(source);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT).unwrap();
    let func = match &tu.items[0] {
        TopLevelItem::FunctionDefinition(f) => f,
        _ => panic!("expected function def"),
    };
    assert_eq!(func.declarator.name.node, "bar");
    assert_eq!(func.body.inner.len(), 0);
}

#[test]
fn format_preserves_function_source() {
    let source = "void bar(int x = -1, ref int y) {}";
    let cst = parse(source);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT).unwrap();
    let func = ...;
    assert_eq!(format_function(func), source);
}
```

Format-preservation tests are the key validation: if we can reconstruct the source byte-for-byte from the AST, the wrapper-token scheme works.

---

## 8. Open decisions

1. **Lifetime introduction.** When do we move from `String` to `&'src str` for `Identifier`?
   - Pragmatic: do Pass 1-3 with owned, swap in lifetimes later if profiling shows allocation pressure.

2. **Trait vs no trait.** Use an `AstNode` trait or just per-type `from_cst` methods?
   - C.llw uses a trait. The user's pattern doesn't need one (each type has its own `from_cst`).
   - Trait adds 1 indirection per method call; no trait means duplication but simpler.
   - **Decision:** start without a trait; add one if it becomes valuable for generic walk (e.g., LSP code that processes any node type).

3. **Where do `Span`s live?** Two current options:
   - On every node struct (verbose, but trivial to query)
   - Computed on demand via `cst.span(node)` (no storage, but requires keeping `NodeRef` everywhere)
   - **Decision:** every node struct carries its `syntax: NodeRef`. `span()` is computed via `cst.span(self.syntax)`. No storage duplication; consumer pattern is "always have a `&Cst` handy".

4. **What does `String` mean in `StringLiteral`?** The raw lexeme (with quotes)? The unescaped content?
   - Lexeme includes quotes; consumer escapes when needed. Storage is the lexeme (`"foo"`).
   - **Decision:** store the raw lexeme; provide a helper `unquote() -> &str`.

5. **`IntLiteral` value type.** `i64` won't fit arbitrarily large hex ints. Use `String` (raw lexeme) and provide a `parse() -> Option<i64>` helper?
   - **Decision:** store as `String`, parse on demand. Same for `FloatConst`.

---

## 9. Files we'll create

Inside `tools/xs-language-server/lelwel-xs/src/ast/`:

```
mod.rs          // re-exports + the AstNode trait (if we use one)
cst_helpers.rs  // walk children by rule/token
spanned.rs      // Spanned<T>, CommaSeparatedList<T>, etc.
tokens.rs       // Paren, Brace, Comma, etc.
type_system.rs  // Type, TypeSpecifier, DeclarationSpecifiers, etc.
declaration.rs  // Declaration, Declarator, DirectDeclarator variants
top_level.rs    // TranslationUnit, TopLevelItem variants
preproc.rs      // PreprocDef, PreprocIf, PreprocElif, etc.
statement.rs    // Statement variants
expr.rs         // Expression variants
argument.rs     // Argument, ArgumentList
parameter.rs    // ParameterDeclaration (or keep in declaration.rs)
format.rs       // proof-of-concept formatter (Pass 3)
```

The existing `mod.rs` becomes the entry point that re-exports everything.

---

## 10. What this plan is NOT

- Not a replacement for the migration plan at `docs/plans/2026-07-01-migrate-tree-sitter-to-lelwel.md`. That's the implementation plan for the parser + LSP consumer code; this is the typed AST layer plan.
- Not the formatter design — only includes "proof-of-concept format preservation". A real formatter needs decisions about comments, indentation strategy, etc.
- Not a guarantee we'll do all three passes. If user wants to ship incremental value, we can stop after Pass 1.

---

## Next step (when resuming after /compact)

1. Read this file: `docs/plans/2026-07-02-typed-ast-design.md`
2. Start Pass 1 by creating `lelwel-xs/src/ast/cst_helpers.rs` and `lelwel-xs/src/ast/spanned.rs` (foundation)
3. Add `#[cfg(test)]` tests for each helper as we go
4. End Pass 1 with a compile-pass + working symbol-table extraction upgrade for top-level declarations
