use crate::ast::spanned::{Parenthesized, Spanned};
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

/// An owned identifier (`Spanned<String>`). Borrowed identifiers can be
/// added later once the LSP lifetime strategy is finalized.
pub type Identifier = Spanned<String>;

impl Identifier {
    pub fn text(&self) -> &str {
        &self.node
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Void,
    Int,
    Bool,
    Float,
    String,
    Vector,
    Class(Identifier),
}

impl Type {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::Void => Type::Void,
            Token::Int => Type::Int,
            Token::Bool => Type::Bool,
            Token::Float => Type::Float,
            Token::StringKw => Type::String,
            Token::Vector => Type::Vector,
            _ => return None,
        })
    }

    pub fn from_token_by_text(text: &str) -> Option<Self> {
        Some(match text {
            "void" => Type::Void,
            "int" => Type::Int,
            "bool" => Type::Bool,
            "float" => Type::Float,
            "string" => Type::String,
            "vector" => Type::Vector,
            _ => return None,
        })
    }

    /// Maps a primitive type to its source token, if applicable.
    pub fn primitive_token(&self) -> Option<Token> {
        Some(match self {
            Type::Void => Token::Void,
            Type::Int => Token::Int,
            Type::Bool => Token::Bool,
            Type::Float => Token::Float,
            Type::String => Token::StringKw,
            Type::Vector => Token::Vector,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionPointerType {
    /// Return type of the function-pointer signature.
    pub ret: Box<TypeSpecifier>,
    /// Parameter types of the function-pointer signature (names are
    /// not required in XS function-pointer signatures).
    pub params: Parenthesized<Vec<TypeSpecifier>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeSpecifier {
    pub ty: Type,
    /// `true` when this type was written with a trailing `[]` (e.g.
    /// `int[]`). The array decoration is stored on the specifier rather
    /// than folded into `Type` to keep the enum simple.
    pub is_array: bool,
    /// If present, this specifier denotes a function-pointer type;
    /// `ty` is the return type and `params` holds the parameter types.
    pub fn_pointer: Option<FunctionPointerType>,
    pub span: Span,
}

impl TypeSpecifier {
    /// Extract from any node that represents a type, accommodating the
    /// multiple shapes the lelwel parser produces depending on whether
    /// the type is a primitive, a user-defined identifier, or wrapped
    /// in a `Rule::TypeSpecifier`/`Rule::ArrayOrPrimitiveType` rule node.
    ///
    /// In practice the parser collapses most wrappers — see the diagnostic
    /// notes in `examples/diag_cst.rs`.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::PrimitiveType) {
            return Self::from_primitive_type(cst, node);
        }
        if cst.match_rule(node, Rule::ArrayOrPrimitiveType) {
            return Self::from_array_or_primitive_type(cst, node);
        }
        if let Some((text, span)) = cst.match_token(node, Token::Identifier) {
            return Some(Self {
                ty: Type::Class(Identifier::new(text.to_string(), span.clone())),
                is_array: false,
                fn_pointer: None,
                span,
            });
        }
        // Fall back: when the rule node wraps a primitive or identifier.
        for child in cst.children(node) {
            if cst.match_rule(child, Rule::PrimitiveType) {
                return Self::from_primitive_type(cst, child);
            }
            if let Some((text, span)) = cst.match_token(child, Token::Identifier) {
                return Some(Self {
                    ty: Type::Class(Identifier::new(text.to_string(), span.clone())),
                    is_array: false,
                    fn_pointer: None,
                    span,
                });
            }
        }
        None
    }

    /// Extract from a `Rule::PrimitiveType` node, which wraps exactly one
    /// primitive-type token child.
    pub fn from_primitive_type(cst: &Cst, prim_type_node: NodeRef) -> Option<Self> {
        if !cst.match_rule(prim_type_node, Rule::PrimitiveType) {
            return None;
        }
        let token = cst.children(prim_type_node).find_map(|c| {
            cst.match_token(c, Token::Void)
                .or_else(|| cst.match_token(c, Token::Int))
                .or_else(|| cst.match_token(c, Token::Bool))
                .or_else(|| cst.match_token(c, Token::Float))
                .or_else(|| cst.match_token(c, Token::StringKw))
                .or_else(|| cst.match_token(c, Token::Vector))
                .map(|(_, span)| span)
        })?;
        let text_node = cst.children(prim_type_node).find(|c| {
            cst.match_token(*c, Token::Void).is_some()
                || cst.match_token(*c, Token::Int).is_some()
                || cst.match_token(*c, Token::Bool).is_some()
                || cst.match_token(*c, Token::Float).is_some()
                || cst.match_token(*c, Token::StringKw).is_some()
                || cst.match_token(*c, Token::Vector).is_some()
        })?;
        let (text, _) = cst.match_token(text_node, Token::Void)
            .or_else(|| cst.match_token(text_node, Token::Int))
            .or_else(|| cst.match_token(text_node, Token::Bool))
            .or_else(|| cst.match_token(text_node, Token::Float))
            .or_else(|| cst.match_token(text_node, Token::StringKw))
            .or_else(|| cst.match_token(text_node, Token::Vector))?;
        Some(Self {
            ty: Type::from_token_by_text(text)?,
            is_array: false,
            fn_pointer: None,
            span: token,
        })
    }

    /// Extract from a `Rule::ArrayOrPrimitiveType` node. In the current
    /// grammar this rarely appears — the parser usually inlines the
    /// primitive plus `[]` tokens directly into the parent. Kept for
    /// forward compatibility if a future grammar change produces it.
    pub fn from_array_or_primitive_type(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ArrayOrPrimitiveType) {
            return None;
        }
        let mut ty = None;
        let mut is_array = false;
        for child in cst.children(node) {
            if cst.match_rule(child, Rule::PrimitiveType) {
                ty = Self::from_primitive_type(cst, child).map(|t| t.ty);
            } else if cst.match_token(child, Token::LBrak).is_some() {
                is_array = true;
            }
        }
        Some(Self {
            ty: ty?,
            is_array,
            fn_pointer: None,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayOrPrimitiveType {
    pub ty: Type,
    pub is_array: bool,
    pub span: Span,
}

impl ArrayOrPrimitiveType {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ArrayOrPrimitiveType) {
            return None;
        }

        let mut ty: Option<Type> = None;
        let mut is_array = false;

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::PrimitiveType) {
                ty = PrimitiveType::from_cst(cst, child).map(|p| p.ty);
            } else if cst.match_token(child, Token::LBrak).is_some() {
                is_array = true;
            }
        }

        Some(Self {
            ty: ty?,
            is_array,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrimitiveType {
    pub ty: Type,
    pub span: Span,
}

impl PrimitiveType {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PrimitiveType) {
            return None;
        }
        let token_node = cst.children(node).next()?;
        let (text, span) = cst
            .match_token(token_node, Token::Void)
            .or_else(|| cst.match_token(token_node, Token::Int))
            .or_else(|| cst.match_token(token_node, Token::Bool))
            .or_else(|| cst.match_token(token_node, Token::Float))
            .or_else(|| cst.match_token(token_node, Token::StringKw))
            .or_else(|| cst.match_token(token_node, Token::Vector))?;
        let ty = Type::from_token_by_text(text)?;
        Some(Self { ty, span })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StorageClassSpecifier {
    Extern,
    Static,
    Mutable,
}

impl StorageClassSpecifier {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::Extern => StorageClassSpecifier::Extern,
            Token::Static => StorageClassSpecifier::Static,
            Token::Mutable => StorageClassSpecifier::Mutable,
            _ => return None,
        })
    }

    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<(Self, Span)> {
        if !cst.match_rule(node, Rule::StorageClassSpecifier) {
            return None;
        }
        for child in cst.children(node) {
            if let Some((text, span)) = cst
                .match_token(child, Token::Extern)
                .or_else(|| cst.match_token(child, Token::Static))
                .or_else(|| cst.match_token(child, Token::Mutable))
            {
                let variant = Self::from_text(text)?;
                return Some((variant, span));
            }
        }
        None
    }

    fn from_text(text: &str) -> Option<Self> {
        Some(match text {
            "extern" => StorageClassSpecifier::Extern,
            "static" => StorageClassSpecifier::Static,
            "mutable" => StorageClassSpecifier::Mutable,
            _ => return None,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TypeQualifier {
    Const,
    Ref,
}

impl TypeQualifier {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::Const => TypeQualifier::Const,
            Token::Ref => TypeQualifier::Ref,
            _ => return None,
        })
    }

    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<(Self, Span)> {
        if !cst.match_rule(node, Rule::TypeQualifier) {
            return None;
        }
        for child in cst.children(node) {
            if let Some((text, span)) = cst
                .match_token(child, Token::Const)
                .or_else(|| cst.match_token(child, Token::Ref))
            {
                let variant = Self::from_text(text)?;
                return Some((variant, span));
            }
        }
        None
    }

    fn from_text(text: &str) -> Option<Self> {
        Some(match text {
            "const" => TypeQualifier::Const,
            "ref" => TypeQualifier::Ref,
            _ => return None,
        })
    }
}

/// A sequence of storage-class specifiers, type qualifiers, and exactly
/// one type specifier (in any order, as per the grammar).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeclarationSpecifiers {
    pub storage: Vec<(StorageClassSpecifier, Span)>,
    pub ty: TypeSpecifier,
    pub type_quals: Vec<(TypeQualifier, Span)>,
    pub span: Span,
}

impl DeclarationSpecifiers {
    /// Extract `DeclarationSpecifiers` from a CST node.
    ///
    /// The parser flattens most type wrappers — for plain `int x;`, the
    /// direct children are `Rule::PrimitiveType → Int` (a single rule
    /// node wrapping one token), plus optional `Token::LBrak`/`Token::RBrak`
    /// tokens for array decoration. For `MyType x;` (user type), the
    /// direct child is just `Token::Identifier`.
    ///
    /// See `examples/diag_cst.rs` for the actual CST shapes.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::DeclarationSpecifiers) {
            return None;
        }

        let mut storage = Vec::new();
        let mut ty: Option<TypeSpecifier> = None;
        let mut type_quals = Vec::new();
        let mut saw_lbrak = false;
        let mut saw_rbrak_after_lbrak = false;

        for child in cst.children(node) {
            match cst.get(child) {
                Node::Rule(Rule::StorageClassSpecifier, _) => {
                    if let Some(parsed) = StorageClassSpecifier::from_cst(cst, child) {
                        storage.push(parsed);
                    }
                }
                Node::Rule(Rule::TypeQualifier, _) => {
                    if let Some(parsed) = TypeQualifier::from_cst(cst, child) {
                        type_quals.push(parsed);
                    }
                }
                Node::Rule(Rule::PrimitiveType, _) => {
                    if ty.is_some() {
                        return None;
                    }
                    ty = TypeSpecifier::from_primitive_type(cst, child);
                }
                Node::Token(Token::Identifier, _) => {
                    if ty.is_some() {
                        return None;
                    }
                    ty = TypeSpecifier::from_cst(cst, child);
                }
                Node::Token(Token::LBrak, _) => {
                    saw_lbrak = true;
                }
                Node::Token(Token::RBrak, _) => {
                    if saw_lbrak {
                        saw_rbrak_after_lbrak = true;
                    }
                }
                _ => {}
            }
        }

        let mut ty = ty?;
        if saw_lbrak && saw_rbrak_after_lbrak {
            ty.is_array = true;
        }

        Some(Self {
            storage,
            ty,
            type_quals,
            span: cst.span(node),
        })
    }

    pub fn storage_class_specifiers(&self) -> &[(StorageClassSpecifier, Span)] {
        &self.storage
    }

    pub fn type_qualifiers(&self) -> &[(TypeQualifier, Span)] {
        &self.type_quals
    }

    pub fn ty(&self) -> &TypeSpecifier {
        &self.ty
    }

    pub fn is_const(&self) -> bool {
        self.type_quals.iter().any(|(q, _)| *q == TypeQualifier::Const)
    }

    pub fn is_ref(&self) -> bool {
        self.type_quals.iter().any(|(q, _)| *q == TypeQualifier::Ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::cst_helpers::{child_by_rule, is_skip_token};
    use crate::parser::Parser;

    fn parse<'a>(source: &'a str) -> Cst<'a> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn first_decl<'a>(cst: &'a Cst<'a>) -> NodeRef {
        cst.children(NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("first non-skip child")
    }

    fn specs_of(source: &str) -> DeclarationSpecifiers {
        let cst = parse(source);
        let decl = first_decl(&cst);
        let specs_node = child_by_rule(&cst, decl, Rule::DeclarationSpecifiers).unwrap();
        DeclarationSpecifiers::from_cst(&cst, specs_node).unwrap()
    }

    #[test]
    fn parses_int_type_specifier() {
        let specs = specs_of("int x;");
        assert_eq!(specs.ty.ty, Type::Int);
        assert!(!specs.ty.is_array);
        assert!(specs.storage.is_empty());
        assert!(specs.type_quals.is_empty());
    }

    #[test]
    fn parses_user_class_type() {
        let specs = specs_of("MyType x;");
        match &specs.ty.ty {
            Type::Class(name) => assert_eq!(name.node, "MyType"),
            other => panic!("expected Class, got {:?}", other),
        }
    }

    #[test]
    fn parses_extern_const_int() {
        let specs = specs_of("extern const int x;");
        assert_eq!(specs.storage.len(), 1);
        assert_eq!(specs.storage[0].0, StorageClassSpecifier::Extern);
        assert_eq!(specs.type_quals.len(), 1);
        assert_eq!(specs.type_quals[0].0, TypeQualifier::Const);
        assert_eq!(specs.ty.ty, Type::Int);
        assert!(specs.is_const());
    }

    #[test]
    fn parses_static_ref() {
        let specs = specs_of("static ref int x;");
        assert_eq!(specs.storage[0].0, StorageClassSpecifier::Static);
        assert_eq!(specs.type_quals[0].0, TypeQualifier::Ref);
        assert!(specs.is_ref());
    }

    #[test]
    fn parses_mutable_modifier() {
        let specs = specs_of("mutable int x;");
        assert_eq!(specs.storage[0].0, StorageClassSpecifier::Mutable);
    }

    #[test]
    fn parses_array_type() {
        let specs = specs_of("int[] x;");
        assert_eq!(specs.ty.ty, Type::Int);
        assert!(specs.ty.is_array);
    }

    #[test]
    fn parses_modifier_after_type() {
        let specs = specs_of("int const x;");
        assert_eq!(specs.ty.ty, Type::Int);
        assert_eq!(specs.type_quals.len(), 1);
        assert_eq!(specs.type_quals[0].0, TypeQualifier::Const);
    }
}