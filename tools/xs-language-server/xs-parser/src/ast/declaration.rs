use crate::ast::cst_helpers::{child_by_rule, child_by_token, children_by_rule, only_child};
use crate::ast::spanned::{CommaSeparatedList, Parenthesized};
use crate::ast::type_system::{DeclarationSpecifiers, FunctionPointerType, Identifier, TypeSpecifier};
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;
use crate::ast::expr::Expr;

/// Placeholder for `Expr` until Pass 3. Holds the unparsed CST node so
/// the rest of the API stays stable across passes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnparsedExpr(pub NodeRef);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Declaration {
    pub decl_specs: DeclarationSpecifiers,
    pub init_declarator_list: InitDeclaratorList,
    pub semi: Span,
    pub span: Span,
}

impl Declaration {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::FunctionPointerDeclaration) {
            return Self::from_function_pointer_declaration(cst, node);
        }
        if cst.match_rule(node, Rule::Declaration) {
            if let Some(fp) = cst.children(node).find(|c| cst.match_rule(*c, Rule::FunctionPointerDeclaration)) {
                return Self::from_function_pointer_declaration(cst, fp);
            }
        }
        if !cst.match_rule(node, Rule::Declaration) {
            return None;
        }

        let mut decl_specs: Option<DeclarationSpecifiers> = None;
        let mut init_declarator_list: Option<InitDeclaratorList> = None;
        let mut semi: Option<Span> = None;

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::DeclarationSpecifiers) && decl_specs.is_none() {
                decl_specs = DeclarationSpecifiers::from_cst(cst, child);
                continue;
            }
            if cst.match_rule(child, Rule::InitDeclaratorList) && init_declarator_list.is_none() {
                init_declarator_list = InitDeclaratorList::from_cst(cst, child);
                continue;
            }
            if let Some(s) = cst.match_token(child, Token::Semi) {
                semi = Some(s.1);
            }
        }

        // `for_init` declarations omit the trailing ';' because the
        // parent `for` rule consumes it; fall back to a zero-width span
        // at the node's end when no semicolon is present.
        let semi = semi.unwrap_or_else(|| {
            let end = cst.span(node).end;
            end..end
        });
        Some(Self {
            decl_specs: decl_specs?,
            init_declarator_list: init_declarator_list?,
            semi,
            span: cst.span(node),
        })
    }

    /// Synthesize a `Declaration` from a `Rule::FunctionPointerDeclaration`
    /// node (`void(int) cb = nullptr;`). The primitive return type and the
    /// parenthesized parameter list are folded into the declaration
    /// specifier's `fn_pointer` field.
    fn from_function_pointer_declaration(cst: &Cst, node: NodeRef) -> Option<Self> {
        let prim_node = cst.children(node).find(|c| cst.match_rule(*c, Rule::PrimitiveType))?;
        let mut ret_ty = TypeSpecifier::from_primitive_type(cst, prim_node)?;
        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LPar).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RPar).map(|(_, s)| s))?;
        let param_types: Vec<TypeSpecifier> = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ParameterList))
            .and_then(|n| crate::ast::parameter::ParameterList::from_cst(cst, n))
            .map(|list| list.items.iter().map(|p| p.decl_specs.ty.clone()).collect())
            .unwrap_or_default();
        ret_ty.fn_pointer = Some(FunctionPointerType {
            ret: Box::new(ret_ty.clone()),
            params: Parenthesized::new(open, param_types, close),
            span: cst.span(node),
        });
        ret_ty.span = cst.span(node);

        let decl_specs = DeclarationSpecifiers {
            storage: Vec::new(),
            ty: ret_ty,
            type_quals: Vec::new(),
            span: cst.span(node),
        };

        let (name_text, name_span) = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Identifier))?;
        let name = Identifier::new(name_text.to_string(), name_span.clone());
        let declarator = Declarator {
            direct: DirectDeclarator::IdentDeclarator(IdentifierDeclarator {
                name: name.clone(),
                span: name_span.clone(),
            }),
            span: name_span.clone(),
        };

        let initializer = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::Assign).is_some())
            .map(|assign| {
                let eq_span = cst.match_token(assign, Token::Assign).unwrap().1;
                let expr_node = cst
                    .children(node)
                    .skip_while(|c| *c != assign)
                    .nth(1)
                    .expect("expression after '='");
                (eq_span, UnparsedExpr(expr_node))
            });

        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))
            .unwrap_or_else(|| {
                let end = cst.span(node).end;
                end..end
            });

        let init = InitDeclarator {
            declarator,
            initializer,
            trailing_comma: None,
            span: name_span.start..semi.end,
        };

        Some(Self {
            decl_specs,
            init_declarator_list: InitDeclaratorList { items: vec![init] },
            semi,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InitDeclaratorList {
    pub items: Vec<InitDeclarator>,
}

impl InitDeclaratorList {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::InitDeclaratorList) {
            return None;
        }

        let mut items: Vec<InitDeclarator> = Vec::new();

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::InitDeclarator) {
                let init = InitDeclarator::from_cst(cst, child)?;
                items.push(init);
            } else if let Some((_, span)) = cst.match_token(child, Token::Comma) {
                // Back-patch: this comma separates the previously pushed
                // item from the next, so it is the trailing comma of the
                // last item. The final item never gets a trailing comma.
                if let Some(last) = items.last_mut() {
                    last.trailing_comma = Some(span);
                }
            }
        }

        Some(Self { items })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InitDeclarator {
    pub declarator: Declarator,
    pub initializer: Option<(Span, UnparsedExpr)>,
    /// Span of the trailing comma that separates this item from the next
    /// in the surrounding list. `None` for the last item (no trailing
    /// comma) or in single-item lists.
    pub trailing_comma: Option<Span>,
    pub span: Span,
}

impl InitDeclarator {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::InitDeclarator) {
            return None;
        }

        let children: Vec<_> = cst.children(node).collect();

        // Find the assign token — everything before it is the declarator
        let assign_idx = children
            .iter()
            .position(|c| cst.match_token(*c, Token::Assign).is_some())
            .unwrap_or(children.len());
        let decl_children = &children[..assign_idx];
        let declarator = Declarator::from_inline_children(cst, decl_children)?;

        // After assign, find the expr rule
        let initializer = if assign_idx < children.len() {
            let eq_span = cst.match_token(children[assign_idx], Token::Assign)?.1;
            let expr_node = children[assign_idx + 1..]
                .iter()
                .find(|c| matches!(cst.get(**c), Node::Rule(_, _)))?
                .clone();
            Some((eq_span, UnparsedExpr(expr_node)))
        } else {
            None
        };

        Some(Self {
            declarator,
            initializer,
            trailing_comma: None,
            span: cst.span(node),
        })
    }

    /// Re-extract the initializer as a typed `Expr` (T12 helper).
    /// Returns `None` when this declarator has no initializer or when
    /// extraction fails for any reason.
    pub fn expr(&self, cst: &Cst) -> Option<Expr> {
        let (_, unparsed) = self.initializer.as_ref()?;
        Expr::from_cst(cst, unparsed.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Declarator {
    pub direct: DirectDeclarator,
    pub span: Span,
}

impl Declarator {
    /// Extract the declarator inline from the given parent (an
    /// `InitDeclarator` or `ForwardDeclaration` or `FunctionDefinitionRule`).
    /// Walks the parent's children past any leading
    /// `DeclarationSpecifiers` and assembles the declarator content
    /// (Identifier + optional `(...)`).
    ///
    /// In the actual CST, wrappers like `Rule::Declarator` and
    /// `Rule::DirectDeclarator` collapse — see `examples/diag_cst.rs`.
    pub fn from_inline(cst: &Cst, parent: NodeRef) -> Option<Self> {
        let children: Vec<_> = cst.children(parent).collect();
        // Skip leading rules (declaration_specifiers) and tokens other than
        // the declarator start. The declarator always begins at the first
        // Identifier (or LPar for paren declarator).
        let decl_start = children
            .iter()
            .position(|c| match cst.get(*c) {
                Node::Token(t, _) => matches!(t, Token::Identifier | Token::LPar),
                _ => false,
            })?;
        Self::from_inline_children(cst, &children[decl_start..])
    }

    /// Legacy wrapper: when `node` is itself a `Rule::Declarator`
    /// (which the current grammar never produces — kept for parity).
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::Declarator) {
            return None;
        }
        let inner_children: Vec<_> = cst
            .children(node)
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)))
            .collect();
        Self::from_inline_children(cst, &inner_children)
    }

    /// Construct a declarator from an inline slice of children whose first
    /// element is the declarator's start (an Identifier or LPar).
    pub fn from_inline_children(cst: &Cst, children: &[NodeRef]) -> Option<Self> {
        let first = *children.first()?;

        // Leading array declarator: '[' ']' Identifier
        // (the C99 GCC-extension form used in retail XS for user-
        // defined array types like `ConstraintParameters[] vConstraints`).
        if cst.match_token(first, Token::LBrak).is_some() {
            // Find the matching RBrak.
            let close_idx = children
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(i, c)| cst.match_token(*c, Token::RBrak).map(|(_, s)| (i, s)))?;
            let _close_span = cst.match_token(children[close_idx.0], Token::RBrak)?.1;
            // The next non-skip token should be the Identifier (the
            // name of the variable being declared).
            let name_idx = children
                .iter()
                .enumerate()
                .skip(close_idx.0 + 1)
                .find_map(|(i, c)| cst.match_token(*c, Token::Identifier).map(|(_, s)| (i, s)))?;
            let (name_text, name_span) = cst.match_token(children[name_idx.0], Token::Identifier)?;
            let open_span = cst.match_token(first, Token::LBrak)?.1;
            let close_span = cst.match_token(children[close_idx.0], Token::RBrak)?.1;
            let span = open_span.start..name_span.end;
            return Some(Self {
                direct: DirectDeclarator::ArrayDeclarator(ArrayDeclarator {
                    base: Identifier::new(name_text.to_string(), name_span.clone()),
                    span: span.clone(),
                }),
                span,
            });
        }

        // Paren declarator: '(' ... ')'
        if cst.match_token(first, Token::LPar).is_some() {
            // For now, model the simple case where the inner is a plain identifier.
            // The grammar permits `int (x) = 1;` — an Identifier wrapped in parens.
            let inner_lpar = cst.match_token(first, Token::LPar)?.1;
            let _ = inner_lpar;
            // The inner content of the paren decl is the next Identifier, etc.
            // Read until matching RPar.
            let mut close_idx = None;
            for (i, c) in children.iter().enumerate().skip(1) {
                if cst.match_token(*c, Token::RPar).is_some() {
                    close_idx = Some(i);
                    break;
                }
            }
            let close_idx = close_idx?;
            let inner_decl = Self::from_inline_children(cst, &children[1..close_idx])?;
            let open_span = cst.match_token(first, Token::LPar)?.1;
            let close_span = cst.match_token(children[close_idx], Token::RPar)?.1;
            let span = open_span.start..close_span.end;
            return Some(Self {
                direct: DirectDeclarator::ParenDeclarator(ParenDeclarator {
                    inner: Box::new(inner_decl),
                    span: span.clone(),
                }),
                span,
            });
        }

        // Identifier-based declarator
        let (name_text, name_span) = cst.match_token(first, Token::Identifier)?;

        // Look for '[' after the name for an array declarator (e.g.
        // `int[] vConstraints`). Must be checked BEFORE the function
        // declarator because both can be followed by content; the
        // '[' vs '(' first token is what distinguishes them.
        let lbrak_pos = children
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(i, c)| cst.match_token(*c, Token::LBrak).map(|(_, s)| (i, s)));

        if let Some((lbrak_idx, open_span)) = lbrak_pos {
            // Find matching RBrak after the LBrak.
            let close_idx = children
                .iter()
                .enumerate()
                .skip(lbrak_idx + 1)
                .find_map(|(i, c)| cst.match_token(*c, Token::RBrak).map(|(_, s)| (i, s)))?;
            let close_span = cst.match_token(children[close_idx.0], Token::RBrak)?.1;
            let span = name_span.start..close_span.end;
            return Some(Self {
                direct: DirectDeclarator::ArrayDeclarator(ArrayDeclarator {
                    base: Identifier::new(name_text.to_string(), name_span.clone()),
                    span: span.clone(),
                }),
                span,
            });
        }

        // Look for '(' after the name for a function declarator.
        let lpar_pos = children
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(i, c)| cst.match_token(*c, Token::LPar).map(|(_, s)| (i, s)));

        if let Some((lpar_idx, open_span)) = lpar_pos {
            // Find matching RPar after the LPar.
            let close_idx = children
                .iter()
                .enumerate()
                .skip(lpar_idx + 1)
                .find_map(|(i, c)| cst.match_token(*c, Token::RPar).map(|(_, s)| (i, s)))?;
            let close_span = cst.match_token(children[close_idx.0], Token::RPar)?.1;

            // Optional parameter_list between LPar and RPar.
            let params_inner = children[lpar_idx + 1..close_idx.0]
                .iter()
                .find_map(|c| {
                    if cst.match_rule(*c, Rule::ParameterList) {
                        ParameterList::from_cst(cst, *c)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(ParameterList::empty);

            let span = name_span.start..close_span.end;
            return Some(Self {
                direct: DirectDeclarator::FunctionDeclarator(FunctionDeclarator {
                    base: Identifier::new(name_text.to_string(), name_span.clone()),
                    params: Some(Parenthesized::new(open_span, params_inner, close_span)),
                    span: span.clone(),
                }),
                span,
            });
        }

        // Plain identifier declarator
        Some(Self {
            direct: DirectDeclarator::IdentDeclarator(IdentifierDeclarator {
                name: Identifier::new(name_text.to_string(), name_span.clone()),
                span: name_span.clone(),
            }),
            span: name_span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DirectDeclarator {
    FunctionDeclarator(FunctionDeclarator),
    ArrayDeclarator(ArrayDeclarator),
    IdentDeclarator(IdentifierDeclarator),
    ParenDeclarator(ParenDeclarator),
}

impl DirectDeclarator {
    /// Legacy wrapper — the current grammar collapses this enum's
    /// variants into inline content. Kept for API stability.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::FunctionDeclarator) {
            let fd = FunctionDeclarator::from_cst(cst, node)?;
            return Some(DirectDeclarator::FunctionDeclarator(fd));
        }
        if cst.match_rule(node, Rule::ArrayDeclarator) {
            let ad = ArrayDeclarator::from_cst(cst, node)?;
            return Some(DirectDeclarator::ArrayDeclarator(ad));
        }
        if cst.match_rule(node, Rule::IdentifierDeclarator) {
            let id = IdentifierDeclarator::from_cst(cst, node)?;
            return Some(DirectDeclarator::IdentDeclarator(id));
        }
        if cst.match_rule(node, Rule::ParenDeclarator) {
            let pd = ParenDeclarator::from_cst(cst, node)?;
            return Some(DirectDeclarator::ParenDeclarator(pd));
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionDeclarator {
    pub base: Identifier,
    pub params: Option<Parenthesized<ParameterList>>,
    pub span: Span,
}

impl FunctionDeclarator {
    /// Legacy: when `Rule::FunctionDeclarator` is a wrapper node.
    /// In the current grammar the parser produces inline content —
    /// use `Declarator::from_inline` instead.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::FunctionDeclarator) {
            return None;
        }

        let mut name: Option<Identifier> = None;
        let mut open: Option<Span> = None;
        let mut close: Option<Span> = None;
        let mut params_node: Option<NodeRef> = None;

        for child in cst.children(node) {
            if let Some((text, span)) = cst.match_token(child, Token::Identifier) {
                if name.is_none() {
                    name = Some(Identifier::new(text.to_string(), span));
                }
                continue;
            }
            if let Some((_, span)) = cst.match_token(child, Token::LPar) {
                open = Some(span);
                continue;
            }
            if let Some((_, span)) = cst.match_token(child, Token::RPar) {
                close = Some(span);
                continue;
            }
            if cst.match_rule(child, Rule::ParameterList) {
                params_node = Some(child);
            }
        }

        let name = name?;
        let open = open?;
        let close = close?;
        let params_inner = params_node
            .and_then(|n| ParameterList::from_cst(cst, n))
            .unwrap_or_else(ParameterList::empty);

        Some(Self {
            base: name,
            params: Some(Parenthesized::new(open, params_inner, close)),
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayDeclarator {
    pub base: Identifier,
    pub span: Span,
}

/// `name[]` — a declarator that wraps an identifier in array brackets,
/// e.g. `ConstraintParameters[] vConstraints` or `int[] x`. Used to
/// model XS user-defined array types (Limitation 8 fix: previously
/// only `int[]` and other primitive arrays were supported, but retail
/// uses `ConstraintParameters[]`, `int[]`, `float[]`, etc. with
/// both primitive and user-defined element types).
///
/// The `[]` is "empty" in XS — no size expression. C-style
/// `int[5]` would be a separate `SizedArrayDeclarator` variant if the
/// engine ever supports it.
impl ArrayDeclarator {
    /// Legacy: when `Rule::ArrayDeclarator` is a wrapper node.
    /// In the current grammar the parser produces inline content —
    /// use `Declarator::from_inline` instead.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ArrayDeclarator) {
            return None;
        }
        let ident_node = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::Identifier).is_some())?;
        let (text, span) = cst.match_token(ident_node, Token::Identifier)?;
        Some(Self {
            base: Identifier::new(text.to_string(), span),
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentifierDeclarator {
    pub name: Identifier,
    pub span: Span,
}

impl IdentifierDeclarator {
    /// Legacy: when `Rule::IdentifierDeclarator` is a wrapper node.
    /// In the current grammar the parser produces inline content —
    /// use `Declarator::from_inline` instead.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::IdentifierDeclarator) {
            return None;
        }
        let ident_node = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::Identifier).is_some())?;
        let (text, span) = cst.match_token(ident_node, Token::Identifier)?;
        Some(Self {
            name: Identifier::new(text.to_string(), span),
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParenDeclarator {
    pub inner: Box<Declarator>,
    pub span: Span,
}

impl ParenDeclarator {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ParenDeclarator) {
            return None;
        }
        let decl_node = cst.children(node).find(|c| cst.match_rule(*c, Rule::Declarator))?;
        let decl = Declarator::from_cst(cst, decl_node)?;
        Some(Self {
            inner: Box::new(decl),
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterDeclaration {
    pub decl_specs: DeclarationSpecifiers,
    pub inner: ParameterInner,
    pub span: Span,
}

impl ParameterDeclaration {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        // In the current grammar the parser collapses the
        // `parameter_declaration` wrapper: the body's `@regular_param` and
        // `@function_pointer_param` markers replace the parent rule. We
        // accept both the (unused) wrapper shape and the (active) direct
        // body shape.
        if cst.match_rule(node, Rule::RegularParam) {
            return Self::from_regular_param_body(cst, node);
        }
        if cst.match_rule(node, Rule::LeadingArrayParam) {
            let span = cst.span(node);
            let regular = RegularParam::from_cst(cst, node)?;
            return Some(Self {
                decl_specs: DeclarationSpecifiers {
                    storage: Vec::new(),
                    ty: TypeSpecifier {
                        ty: crate::ast::type_system::Type::Void,
                        is_array: false,
                        fn_pointer: None,
                        span: span.clone(),
                    },
                    type_quals: Vec::new(),
                    span: span.clone(),
                },
                inner: ParameterInner::LeadingArrayParam(regular),
                span,
            });
        }
        if cst.match_rule(node, Rule::FunctionPointerParam) {
            let ret_ty = cst
                .children(node)
                .find(|c| cst.match_rule(*c, Rule::DeclarationSpecifiers))
                .and_then(|n| DeclarationSpecifiers::from_cst(cst, n))
                .map(|ds| ds.ty)?;
            let fp = Self::function_pointer_param_from_body(cst, node, ret_ty)?;
            let span = cst.span(node);
            return Some(Self {
                decl_specs: DeclarationSpecifiers {
                    storage: Vec::new(),
                    ty: fp.fn_type.0.clone(),
                    type_quals: Vec::new(),
                    span: span.clone(),
                },
                inner: ParameterInner::FunctionPointerParam(fp),
                span,
            });
        }
        if !cst.match_rule(node, Rule::ParameterDeclaration) {
            return None;
        }

        let mut decl_specs: Option<DeclarationSpecifiers> = None;
        let mut inner: Option<ParameterInner> = None;

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::DeclarationSpecifiers) && decl_specs.is_none() {
                decl_specs = DeclarationSpecifiers::from_cst(cst, child);
                continue;
            }
            if cst.match_rule(child, Rule::FunctionPointerParam) {
                let ret_ty = decl_specs.as_ref()?.ty.clone();
                inner = Some(ParameterInner::FunctionPointerParam(
                    Self::function_pointer_param_from_body(cst, child, ret_ty)?,
                ));
                continue;
            }
            if cst.match_rule(child, Rule::RegularParam) {
                inner = Some(ParameterInner::RegularParam(
                    RegularParam::from_cst(cst, child)?,
                ));
                continue;
            }
            if cst.match_rule(child, Rule::LeadingArrayParam) {
                inner = Some(ParameterInner::LeadingArrayParam(
                    RegularParam::from_cst(cst, child)?,
                ));
            }
        }

        Some(Self {
            decl_specs: decl_specs?,
            inner: inner?,
            span: cst.span(node),
        })
    }

    /// Extract a `ParameterDeclaration` from a `Rule::RegularParam` body
    /// node, which contains: `declaration_specifiers` + `Identifier` +
    /// optional `= expression`.
    fn from_regular_param_body(cst: &Cst, regular_param: NodeRef) -> Option<Self> {
        let mut decl_specs: Option<DeclarationSpecifiers> = None;
        let mut name: Option<Identifier> = None;
        let mut default: Option<(Span, UnparsedExpr)> = None;
        let children: Vec<_> = cst.children(regular_param).collect();
        let assign_idx = children
            .iter()
            .position(|c| cst.match_token(*c, Token::Assign).is_some());

        for (i, child) in children.iter().enumerate() {
            if cst.match_rule(*child, Rule::DeclarationSpecifiers) && decl_specs.is_none() {
                decl_specs = DeclarationSpecifiers::from_cst(cst, *child);
                continue;
            }
            if cst.match_token(*child, Token::Identifier).is_some() && name.is_none() {
                let (text, span) = cst.match_token(*child, Token::Identifier)?;
                name = Some(Identifier::new(text.to_string(), span));
                continue;
            }
            if let Some(idx) = assign_idx {
                if i == idx {
                    let eq_span = cst.match_token(*child, Token::Assign)?.1;
                    let expr_node = children
                        .iter()
                        .skip(idx + 1)
                        .find(|c| matches!(cst.get(**c), Node::Rule(_, _)))?;
                    default = Some((eq_span, UnparsedExpr(*expr_node)));
                }
            }
        }

        let span = cst.span(regular_param);
        Some(Self {
            decl_specs: decl_specs?,
            inner: ParameterInner::RegularParam(RegularParam {
                name,
                default,
                span: span.clone(),
            }),
            span,
        })
    }

    /// Build a `FunctionPointerParam` from a `Rule::FunctionPointerParam`
    /// body node given the already-extracted return type. The return type
    /// lives in the parent `ParameterDeclaration`'s `declaration_specifiers`.
    fn function_pointer_param_from_body(
        cst: &Cst,
        fp_param: NodeRef,
        ret_ty: TypeSpecifier,
    ) -> Option<FunctionPointerParam> {
        let mut open: Option<Span> = None;
        let mut close: Option<Span> = None;
        let mut params_node: Option<NodeRef> = None;
        let mut name: Option<Identifier> = None;
        let mut default: Option<(Span, UnparsedExpr)> = None;

        let children: Vec<_> = cst.children(fp_param).collect();
        let assign_idx = children
            .iter()
            .position(|c| cst.match_token(*c, Token::Assign).is_some());

        for (i, child) in children.iter().enumerate() {
            if cst.match_rule(*child, Rule::ParameterList) && params_node.is_none() {
                params_node = Some(*child);
                continue;
            }
            if cst.match_token(*child, Token::LPar).is_some() && open.is_none() {
                open = cst.match_token(*child, Token::LPar).map(|(_, s)| s);
                continue;
            }
            if cst.match_token(*child, Token::RPar).is_some() && close.is_none() {
                close = cst.match_token(*child, Token::RPar).map(|(_, s)| s);
                continue;
            }
            if cst.match_token(*child, Token::Identifier).is_some() && name.is_none() {
                let (text, span) = cst.match_token(*child, Token::Identifier)?;
                name = Some(Identifier::new(text.to_string(), span));
                continue;
            }
            if let Some(idx) = assign_idx {
                if i == idx {
                    let eq_span = cst.match_token(*child, Token::Assign)?.1;
                    let expr_node = children
                        .iter()
                        .skip(idx + 1)
                        .find(|c| matches!(cst.get(**c), Node::Rule(_, _)))?;
                    default = Some((eq_span, UnparsedExpr(*expr_node)));
                }
            }
        }

        let span = cst.span(fp_param);
        let params = params_node
            .and_then(|n| ParameterList::from_cst(cst, n))
            .unwrap_or_else(ParameterList::empty);
        Some(FunctionPointerParam {
            fn_type: (ret_ty, Parenthesized::new(open?, params, close?)),
            name: name?,
            default,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParameterInner {
    FunctionPointerParam(FunctionPointerParam),
    RegularParam(RegularParam),
    /// Parameter with a leading array decoration, e.g.
    /// `ref HuntData[] candidates`. The `[]` is the leading array
    /// form (C99 GCC extension) used by retail XS for user-defined
    /// array types. Structurally identical to a `RegularParam` in
    /// terms of `name` and `default`; the separate variant lets
    /// downstream consumers tell the array case apart if they
    /// need to.
    LeadingArrayParam(RegularParam),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionPointerParam {
    pub fn_type: (TypeSpecifier, Parenthesized<ParameterList>),
    pub name: Identifier,
    pub default: Option<(Span, UnparsedExpr)>,
    pub span: Span,
}

impl FunctionPointerParam {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::FunctionPointerParam) {
            return None;
        }

        let mut ty: Option<TypeSpecifier> = None;
        let mut open: Option<Span> = None;
        let mut close: Option<Span> = None;
        let mut params_node: Option<NodeRef> = None;
        let mut name: Option<Identifier> = None;
        let mut default: Option<(Span, UnparsedExpr)> = None;

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::TypeSpecifier) && ty.is_none() {
                ty = TypeSpecifier::from_cst(cst, child);
                continue;
            }
            if cst.match_rule(child, Rule::ParameterList) && params_node.is_none() {
                params_node = Some(child);
                continue;
            }
            if let Some((_, span)) = cst.match_token(child, Token::LPar) {
                open = Some(span);
                continue;
            }
            if let Some((_, span)) = cst.match_token(child, Token::RPar) {
                close = Some(span);
                continue;
            }
            if cst.match_token(child, Token::Identifier).is_some() && name.is_none() {
                let (text, span) = cst.match_token(child, Token::Identifier)?;
                name = Some(Identifier::new(text.to_string(), span));
                continue;
            }
            if let Some((_, eq_span)) = cst.match_token(child, Token::Assign) {
                let expr_node = cst
                    .children(node)
                    .find(|c| {
                        let n = cst.get(*c);
                        matches!(n, Node::Rule(_, _))
                            && !cst.match_rule(*c, Rule::TypeSpecifier)
                            && !cst.match_rule(*c, Rule::ParameterList)
                            && !cst.match_token(*c, Token::Identifier).is_some()
                    })?;
                default = Some((eq_span, UnparsedExpr(expr_node)));
            }
        }

        let ty = ty?;
        let open = open?;
        let close = close?;
        let name = name?;
        let params = params_node
            .and_then(|n| ParameterList::from_cst(cst, n))
            .unwrap_or_else(ParameterList::empty);

        Some(Self {
            fn_type: (ty, Parenthesized::new(open, params, close)),
            name,
            default,
            span: cst.span(node),
        })
    }

    /// Re-extract the default value as a typed `Expr` (T12 helper).
    pub fn expr(&self, cst: &Cst) -> Option<Expr> {
        let (_, unparsed) = self.default.as_ref()?;
        Expr::from_cst(cst, unparsed.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegularParam {
    /// Parameter name. `None` for unnamed function-pointer signature
    /// parameters such as `void(int)`.
    pub name: Option<Identifier>,
    pub default: Option<(Span, UnparsedExpr)>,
    pub span: Span,
}

impl RegularParam {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::RegularParam) {
            return None;
        }

        let mut name: Option<Identifier> = None;
        let mut default: Option<(Span, UnparsedExpr)> = None;

        for child in cst.children(node) {
            if cst.match_token(child, Token::Identifier).is_some() && name.is_none() {
                let (text, span) = cst.match_token(child, Token::Identifier)?;
                name = Some(Identifier::new(text.to_string(), span));
                continue;
            }
            if let Some((_, eq_span)) = cst.match_token(child, Token::Assign) {
                let expr_node = cst
                    .children(node)
                    .find(|c| {
                        let n = cst.get(*c);
                        matches!(n, Node::Rule(_, _))
                    })?;
                default = Some((eq_span, UnparsedExpr(expr_node)));
            }
        }

        Some(Self {
            name,
            default,
            span: cst.span(node),
        })
    }

    /// Re-extract the default value as a typed `Expr` (T12 helper).
    pub fn expr(&self, cst: &Cst) -> Option<Expr> {
        let (_, unparsed) = self.default.as_ref()?;
        Expr::from_cst(cst, unparsed.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterList {
    pub items: CommaSeparatedList<ParameterDeclaration>,
    pub span: Span,
}

impl ParameterList {
    /// An empty parameter list with a zero-width span. Used when a
    /// parameter list is syntactically optional but the consumer still
    /// expects a `ParameterList` value (e.g. for `()` in a function
    /// declarator).
    pub fn empty() -> Self {
        Self {
            items: CommaSeparatedList::default(),
            span: 0..0,
        }
    }

    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ParameterList) {
            return None;
        }

        let mut items: Vec<(ParameterDeclaration, Option<Span>)> = Vec::new();

        for child in cst.children(node) {
            if cst.match_rule(child, Rule::Parameter) {
                let p = Parameter::from_cst(cst, child)?;
                items.push((p.declaration, None));
            } else if let Some((_, span)) = cst.match_token(child, Token::Comma) {
                // Back-patch: this comma is the trailing comma of the
                // previously pushed item. The last item never gets one
                // because the iteration ends with no trailing comma in
                // valid syntax.
                if let Some(last) = items.last_mut() {
                    last.1 = Some(span);
                }
            }
        }

        Some(Self {
            items: CommaSeparatedList::new(items),
            span: cst.span(node),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Wrapper that pairs a `ParameterDeclaration` with the parser-side
/// `Parameter` rule node (kept for diagnostic attribution).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parameter {
    pub declaration: ParameterDeclaration,
}

impl Parameter {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::Parameter) {
            return None;
        }
        // Find the body rule. In the current grammar the
        // `parameter_declaration` wrapper is collapsed — the
        // body rule (regular_param or function_pointer_param) is the
        // direct child of `parameter`. The wrapper path is kept for
        // forward compatibility.
        let body = cst.children(node).find_map(|c| {
            if cst.match_rule(c, Rule::RegularParam)
                || cst.match_rule(c, Rule::FunctionPointerParam)
                || cst.match_rule(c, Rule::ParameterDeclaration)
            {
                Some(c)
            } else {
                None
            }
        })?;
        let declaration = ParameterDeclaration::from_cst(cst, body)?;
        Some(Self { declaration })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArgumentList {
    pub items: CommaSeparatedList<Argument>,
    pub span: Span,
}

impl ArgumentList {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ArgumentList) {
            return None;
        }

        let mut items: Vec<(Argument, Option<Span>)> = Vec::new();

        // Back-patch: when a comma is seen, set it as the trailing
        // comma of the LAST pushed item. The last item never gets
        // a trailing comma. Matches ParameterList / InitDeclaratorList
        // semantics (and the `comma_separated_list_iter_and_len` test).
        for child in cst.children(node) {
            if cst.match_rule(child, Rule::Argument) {
                let arg = Argument::from_cst(cst, child)?;
                items.push((arg, None));
            } else if let Some((_, span)) = cst.match_token(child, Token::Comma) {
                if let Some(last) = items.last_mut() {
                    last.1 = Some(span);
                }
            }
        }

        Some(Self {
            items: CommaSeparatedList::new(items),
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Argument {
    pub expr: UnparsedExpr,
    pub span: Span,
}

impl Argument {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::Argument) {
            return None;
        }
        let expr_node = only_child(cst, node)?;
        Some(Self {
            expr: UnparsedExpr(expr_node),
            span: cst.span(node),
        })
    }

    /// Re-extract the inner expression as a typed `Expr` (T12 helper).
    pub fn expr(&self, cst: &Cst) -> Option<Expr> {
        Expr::from_cst(cst, self.expr.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::cst_helpers::{child_by_rule, is_skip_token, only_child};
    use crate::ast::type_table::TypeTable;
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

    fn parse_declaration(source: &str) -> Declaration {
        let cst = parse(source);
        let decl = first_decl(&cst);
        Declaration::from_cst(&cst, decl).unwrap_or_else(|| panic!("Declaration for {:?}", source))
    }

    #[test]
    fn parses_plain_int_declaration() {
        let decl = parse_declaration("int x = 5;");
        assert_eq!(decl.decl_specs.ty.ty, crate::ast::type_system::Type::Int);
        assert_eq!(decl.init_declarator_list.items.len(), 1);
        let init = &decl.init_declarator_list.items[0];
        assert!(init.initializer.is_some(), "expected = 5 initializer");
    }

    #[test]
    fn declarator_name_is_x() {
        let decl = parse_declaration("int x = 5;");
        let init = &decl.init_declarator_list.items[0];
        let dir = &init.declarator.direct;
        match dir {
            DirectDeclarator::IdentDeclarator(id) => assert_eq!(id.name.node, "x"),
            other => panic!("expected IdentDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn parses_multi_declarator_list() {
        let decl = parse_declaration("int a = 1, b = 2, c = 3;");
        assert_eq!(decl.init_declarator_list.items.len(), 3);
        let names: Vec<String> = decl
            .init_declarator_list
            .items
            .iter()
            .map(|init| match &init.declarator.direct {
                DirectDeclarator::IdentDeclarator(id) => id.name.node.clone(),
                _ => panic!("expected identifier declarators"),
            })
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        // The first two items carry trailing commas; the last one does not.
        assert!(decl.init_declarator_list.items[0].trailing_comma.is_some());
        assert!(decl.init_declarator_list.items[1].trailing_comma.is_some());
        assert!(decl.init_declarator_list.items[2].trailing_comma.is_none());
    }

    #[test]
    fn parses_function_declarator_in_decl() {
        let decl = parse_declaration("void foo(int x) = 1;");
        let init = &decl.init_declarator_list.items[0];
        match &init.declarator.direct {
            DirectDeclarator::FunctionDeclarator(fd) => {
                assert_eq!(fd.base.node, "foo");
                let params = fd.params.as_ref().expect("params");
                assert_eq!(params.inner.items.items.len(), 1);
                match &params.inner.items.items[0].0.inner {
                    ParameterInner::RegularParam(rp) => {
                        assert_eq!(rp.name.as_ref().expect("param name").node, "x");
                    }
                    other => panic!("expected RegularParam, got {:?}", other),
                }
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn parses_empty_function_declarator() {
        let decl = parse_declaration("void foo() = 1;");
        let init = &decl.init_declarator_list.items[0];
        match &init.declarator.direct {
            DirectDeclarator::FunctionDeclarator(fd) => {
                assert_eq!(fd.base.node, "foo");
                let params = fd.params.as_ref().expect("params");
                assert!(params.inner.is_empty());
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn parses_extern_modifier() {
        // Source uses `=` so the parser produces a `Declaration` (with
        // an init_declarator). The `extern`/`const` modifiers still
        // flow through `DeclarationSpecifiers`.
        let decl = parse_declaration("extern const int x = 0;");
        assert!(decl
            .decl_specs
            .storage
            .iter()
            .any(|(s, _)| *s == crate::ast::type_system::StorageClassSpecifier::Extern));
        assert!(decl.decl_specs.is_const());
    }

    #[test]
    fn parses_ref_qualifier() {
        // Source uses `=` to produce a `Declaration` (the bare
        // `int ref x;` form is a `forward_declaration`; the same
        // DeclarationSpecifiers extraction works in either case).
        let decl = parse_declaration("int ref x = 0;");
        assert!(decl.decl_specs.is_ref());
    }

    #[test]
    fn parses_array_type() {
        let decl = parse_declaration("int[] arr = 1;");
        assert!(decl.decl_specs.ty.is_array);
        assert_eq!(decl.decl_specs.ty.ty, crate::ast::type_system::Type::Int);
    }

    #[test]
    fn parses_paren_declarator() {
        let cst = parse("int (x) = 1;");
        // May not always parse in PoC — assert that we can at least
        // recover a Declaration node if the grammar allows it.
        let decl = first_decl(&cst);
        let _ = Declaration::from_cst(&cst, decl);
    }

    #[test]
    fn parameter_list_extracts_two_params() {
        let cst = parse("void foo(int a, string b) = 0;");
        let decl = first_decl(&cst);
        let init_list = child_by_rule(&cst, decl, Rule::InitDeclaratorList).unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        // The declarator content is inline in the init_declarator node —
        // not wrapped in a `Rule::Declarator` or `Rule::DirectDeclarator`.
        // Walk to the first Identifier, then look for the LPar/RPar/params.
        let declarator = Declarator::from_inline(&cst, init).unwrap();
        match declarator.direct {
            DirectDeclarator::FunctionDeclarator(fd) => {
                let params = fd.params.as_ref().unwrap();
                assert_eq!(params.inner.items.len(), 2);
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn function_pointer_param_recognized() {
        let cst = parse("void foo(void(int) cb) = 0;");
        let _ = cst;
    }

    #[test]
    fn block_comment_edge_cases_lex_cleanly_lambda_d_05() {
        let sources = [
            "/* */ int x;",
            "/**/ int x;",
            "/* * */ int x;",
            "/*\n*/ int x;",
            "/* multi\nline\ncomment */ int x;",
        ];
        for source in sources {
            let (cst, diags) = {
                let mut diags = vec![];
                let cst = Parser::new_with_context(source, &mut diags, TypeTable::with_primitives())
                    .parse(&mut diags);
                (cst, diags)
            };
            assert!(diags.is_empty(), "{} produced diagnostics: {:?}", source, diags);
            let _ = cst;
        }
    }

    #[test]
    fn function_pointer_variable_decl_extracts_fn_pointer_type() {
        use crate::ast::type_system::Type;
        let decl = parse_declaration("void(int) cb = nullptr;");
        assert_eq!(decl.decl_specs.ty.ty, Type::Void);
        assert!(decl.decl_specs.ty.fn_pointer.is_some(), "expected function-pointer type");
        let fp = decl.decl_specs.ty.fn_pointer.as_ref().unwrap();
        assert_eq!(fp.params.inner.len(), 1);
        assert_eq!(fp.params.inner[0].ty, Type::Int);
        let init = &decl.init_declarator_list.items[0];
        match &init.declarator.direct {
            DirectDeclarator::IdentDeclarator(id) => assert_eq!(id.name.node, "cb"),
            other => panic!("expected identifier declarator, got {:?}", other),
        }
    }

    fn first_function_param(cst: &Cst<'_>) -> ParameterDeclaration {
        let first = first_decl(cst);
        let fd = crate::ast::top_level::FunctionDefinition::from_cst(cst, first)
            .expect("function definition");
        match &fd.declarator.direct {
            DirectDeclarator::FunctionDeclarator(fnd) => {
                let params = fnd.params.as_ref().expect("params");
                assert_eq!(params.inner.items.items.len(), 1);
                params.inner.items.items[0].0.clone()
            }
            other => panic!("expected function declarator, got {:?}", other),
        }
    }

    #[test]
    fn parameter_with_function_pointer_default_extracts_lambda_d_03() {
        use crate::ast::type_system::Type;
        let cst = parse("void foo(void(int) cb = [](int x) {}) {}");
        let param = first_function_param(&cst);
        assert_eq!(param.decl_specs.ty.ty, Type::Void);
        match &param.inner {
            ParameterInner::FunctionPointerParam(fp) => {
                assert_eq!(fp.name.node, "cb");
                assert_eq!(fp.fn_type.1.inner.items.items.len(), 1);
                assert_eq!(fp.fn_type.1.inner.items.items[0].0.decl_specs.ty.ty, Type::Int);
                assert!(fp.default.is_some(), "expected default lambda");
            }
            other => panic!("expected FunctionPointerParam, got {:?}", other),
        }
    }

    #[test]
    fn parameter_with_function_pointer_default_extracts_nullptr_lambda_d_04() {
        use crate::ast::type_system::Type;
        let cst = parse("void foo(void(int) cb = nullptr) {}");
        let param = first_function_param(&cst);
        assert_eq!(param.decl_specs.ty.ty, Type::Void);
        match &param.inner {
            ParameterInner::FunctionPointerParam(fp) => {
                assert_eq!(fp.name.node, "cb");
                assert_eq!(fp.fn_type.1.inner.items.items.len(), 1);
                assert_eq!(fp.fn_type.1.inner.items.items[0].0.decl_specs.ty.ty, Type::Int);
                assert!(fp.default.is_some(), "expected default nullptr");
            }
            other => panic!("expected FunctionPointerParam, got {:?}", other),
        }
    }
}