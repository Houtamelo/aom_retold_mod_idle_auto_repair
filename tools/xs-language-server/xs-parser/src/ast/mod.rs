pub mod argument;
pub mod cst_helpers;
pub mod declaration;
pub mod expr;
pub mod format;
pub mod parameter;
pub mod preproc;
pub mod spanned;
pub mod statement;
pub mod tokens;
pub mod top_level;
pub mod type_system;
pub mod type_table;

pub use spanned::{
    Braced, Bracketed, CommaSeparatedList, Parenthesized, SemiColonSeparatedList, Spanned,
    TokenSpan,
};

pub use type_system::{
    ArrayOrPrimitiveType, DeclarationSpecifiers, Identifier, PrimitiveType, StorageClassSpecifier,
    Type, TypeQualifier, TypeSpecifier,
};

pub use declaration::{
    Argument, ArgumentList, ArrayDeclarator, Declaration, Declarator, DirectDeclarator,
    FunctionDeclarator, FunctionPointerParam, IdentifierDeclarator, InitDeclarator,
    InitDeclaratorList, Parameter, ParameterDeclaration, ParameterInner, ParameterList,
    ParenDeclarator, RegularParam, UnparsedExpr,
};

pub use expr::{
    AssignmentExpr, AssignmentOp, BinaryExpr, BinaryOp, CallExpr, CommaExpr, ConditionalExpr,
    DefaultExpr, Expr, FalseLiteral, FieldExpr, FloatLiteral, IdentifierExpr, IntLiteral,
    LambdaExpr, NewExpr, NullLiteral, ParenExpr, PostDecExpr, PostIncExpr, PostfixExpr,
    PostfixInner, PreIncDecOp, StringLiteral, SubscriptExpr, TrueLiteral, UnaryExpr, UnaryKind,
    UnaryOp, VectorLiteral,
};

// Pass 3 (T13) — module re-exports so consumers can use the new
// organizational paths.
pub use crate::ast::argument as argument_mod;
pub use crate::ast::parameter as parameter_mod;

pub use preproc::{
    AddOp, EqOp, MulOp, PreprocDef, PreprocElif, PreprocElse, PreprocEndif, PreprocExpr, PreprocIf,
    RelOp,
};

pub use top_level::{
    ClassDefinition, ClassMember, FieldDeclaration, ForwardDeclaration, FunctionDefinition,
    IncludeDirective, RuleDefinition, RuleModifier, TopLevelItem, TranslationUnit,
};

pub use statement::{
    BlockItem, BlockItemList, BreakStatement, CompoundStatement, ContinueStatement,
    ExpressionStatement, ForInit, ForStatement, IfStatement, ReturnStatement, StmtSpanned,
    SwitchCase, SwitchLabel, SwitchStatement, WhileStatement,
};

pub use cst_helpers::{
    child_by_rule, child_by_token, children_by_rule, is_skip_token, only_child,
};

// Re-export the parser API surface so consumers only need to import
// from `xs_parser::ast`.
pub use crate::lexer::Token;
pub use crate::parser::{Cst, CstChildren, Node, NodeRef, Rule, Span};

// Re-export the parser context type so Phase 4 can build a TypeTable
// from workspace symbols and pass it to Parser::new_with_context.
pub use crate::ast::type_table::TypeTable;
