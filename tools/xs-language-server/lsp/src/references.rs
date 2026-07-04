//! Find all identifier uses of a name in a parsed XS file, plus helpers
//! used by the three LSP handlers:
//!   * `textDocument/references`
//!   * `textDocument/rename`
//!   * `textDocument/prepareRename`
//!
//! Week 4 of the post-spike roadmap. Workspace-wide resolution is week 5+.
//! For now we only walk the single file the cursor is in.

use tower_lsp_server::ls_types::{Location, Range, Uri};
use xs_parser::ast::{
    ClassDefinition, ClassMember, Declaration, Expr, ForwardDeclaration, FunctionDefinition,
    Identifier, ParameterDeclaration, ParameterInner, TopLevelItem, TranslationUnit, TypeTable,
};
use xs_parser::parser::{Cst, NodeRef, Parser};

use crate::range::{position_to_byte_offset, span_to_range};

/// Parse `source` into a typed AST plus its CST (needed to lift
/// still-unparsed expression fragments).
fn parse_typed(source: &str) -> Option<(Cst<'_>, TranslationUnit)> {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)?;
    Some((cst, tu))
}

/// Walk the typed AST of `source`. For every identifier whose source slice
/// equals `name`, push its range onto the output.
///
/// Declaration names (functions, variables, rules, classes, parameters) and
/// expression identifiers are included. Type identifiers and member field
/// names are omitted to match the legacy parser behaviour, which only
/// matched `"identifier"` nodes.
pub fn find_identifier_uses(source: &str, name: &str) -> Vec<Range> {
    let Some((cst, tu)) = parse_typed(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut walker = IdentifierWalker { source, cst: &cst, name, out: &mut out };
    walker.visit_translation_unit(&tu);
    out
}

struct IdentifierWalker<'a> {
    source: &'a str,
    cst: &'a Cst<'a>,
    name: &'a str,
    out: &'a mut Vec<Range>,
}

impl<'a> IdentifierWalker<'a> {
    fn push(&mut self, id: &Identifier) {
        if id.node == self.name {
            self.out.push(span_to_range(self.source, id.span.clone()));
        }
    }

    fn visit_translation_unit(&mut self, tu: &TranslationUnit) {
        for item in &tu.items {
            self.visit_top_level_item(item);
        }
    }

    fn visit_top_level_item(&mut self, item: &TopLevelItem) {
        match item {
            TopLevelItem::RuleDefinition(r) => self.push(&r.name),
            TopLevelItem::FunctionDefinition(f) => self.visit_function_definition(f),
            TopLevelItem::ForwardDeclaration(f) => self.visit_forward_declaration(f),
            TopLevelItem::ClassDefinition(c) => self.visit_class_definition(c),
            TopLevelItem::Declaration(d) => self.visit_declaration(d),
            _ => {}
        }
    }

    fn visit_function_definition(&mut self, f: &FunctionDefinition) {
        self.push(&f.declarator.direct.base());
        self.visit_declarator_params(&f.declarator.direct);
        self.visit_block_items(&f.body.inner);
    }

    fn visit_forward_declaration(&mut self, f: &ForwardDeclaration) {
        self.push(&f.declarator.direct.base());
        self.visit_declarator_params(&f.declarator.direct);
    }

    fn visit_class_definition(&mut self, c: &ClassDefinition) {
        self.push(&c.name);
        for member in &c.members.inner {
            match member {
                ClassMember::FunctionDefinition(f) => self.visit_function_definition(f),
                ClassMember::ForwardDeclaration(f) => self.visit_forward_declaration(f),
                ClassMember::FieldDeclaration(f) => {
                    self.push(&f.declarator.direct.base());
                    if let Some(expr) = f.expr(self.cst) {
                        self.visit_expr(&expr);
                    }
                }
            }
        }
    }

    fn visit_declaration(&mut self, d: &Declaration) {
        for init in &d.init_declarator_list.items {
            self.push(&init.declarator.direct.base());
            if let Some(expr) = init.expr(self.cst) {
                self.visit_expr(&expr);
            }
        }
    }

    fn visit_declarator_params(&mut self, direct: &xs_parser::ast::DirectDeclarator) {
        match direct {
            xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => {
                if let Some(pl) = &fd.params {
                    for (param, _) in &pl.inner.items.items {
                        match &param.inner {
                            ParameterInner::RegularParam(r) => {
                                if let Some(name) = &r.name {
                                    self.push(name);
                                }
                            }
                            ParameterInner::FunctionPointerParam(fp) => self.push(&fp.name),
                        }
                        if let Some(expr) = param.default_expr(self.cst) {
                            self.visit_expr(&expr);
                        }
                    }
                }
            }
            xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => {
                self.visit_declarator_params(&pd.inner.direct);
            }
            _ => {}
        }
    }

    fn visit_block_items(&mut self, items: &[xs_parser::ast::statement::BlockItem]) {
        use xs_parser::ast::statement::BlockItem;
        for item in items {
            match item {
                BlockItem::Declaration(d) => self.visit_declaration(d),
                BlockItem::ForwardDeclaration(f) => self.visit_forward_declaration(f),
                BlockItem::FunctionDefinition(f) => self.visit_function_definition(f),
                BlockItem::Statement(s) => self.visit_statement(s),
            }
        }
    }

    fn visit_statement(&mut self, s: &xs_parser::ast::statement::Statement) {
        use xs_parser::ast::statement::{ForInit, Statement};
        match s {
            Statement::Compound(c) => self.visit_block_items(&c.items.inner),
            Statement::Expression(es) => {
                if let Some(e) = es.expr.as_ref().and_then(|u| Expr::from_cst(self.cst, u.0)) {
                    self.visit_expr(&e);
                }
            }
            Statement::Return(r) => {
                if let Some(v) = &r.value {
                    if let Some(e) = Expr::from_cst(self.cst, v.0) {
                        self.visit_expr(&e);
                    }
                }
            }
            Statement::If(i) => {
                if let Some(e) = Expr::from_cst(self.cst, i.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_statement(&i.then);
                if let Some(else_) = &i.else_ {
                    self.visit_statement(else_);
                }
            }
            Statement::While(w) => {
                if let Some(e) = Expr::from_cst(self.cst, w.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_statement(&w.body);
            }
            Statement::For(f) => {
                match &f.init {
                    ForInit::Declaration(d) => self.visit_declaration(d),
                    ForInit::Expression(u) => {
                        if let Some(e) = Expr::from_cst(self.cst, u.0) {
                            self.visit_expr(&e);
                        }
                    }
                    ForInit::Empty => {}
                }
                if let Some(u) = &f.cond {
                    if let Some(e) = Expr::from_cst(self.cst, u.0) {
                        self.visit_expr(&e);
                    }
                }
                if let Some(u) = &f.post {
                    if let Some(e) = Expr::from_cst(self.cst, u.0) {
                        self.visit_expr(&e);
                    }
                }
                self.visit_statement(&f.body);
            }
            Statement::Switch(s) => {
                if let Some(e) = Expr::from_cst(self.cst, s.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_block_items(&s.body.items.inner);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, e: &Expr) {
        use xs_parser::ast::expr::*;
        match e {
            Expr::Identifier(id) => self.push(&id.name),
            Expr::Unary(u) => self.visit_expr(&u.operand),
            Expr::Binary(b) => {
                self.visit_expr(&b.lhs);
                self.visit_expr(&b.rhs);
            }
            Expr::Conditional(c) => {
                self.visit_expr(&c.cond);
                if let Some(t) = &c.then {
                    self.visit_expr(t);
                }
                self.visit_expr(&c.else_);
            }
            Expr::Assignment(a) => {
                self.visit_expr(&a.lhs);
                self.visit_expr(&a.rhs);
            }
            Expr::Comma(c) => {
                for e in &c.exprs {
                    self.visit_expr(e);
                }
            }
            Expr::Paren(p) => self.visit_expr(&p.inner),
            Expr::Postfix(p) => {
                self.visit_expr(&p.target);
                if let PostfixInner::Call(call) = &p.inner {
                    if let Some(args) = &call.args {
                        for (arg, _) in &args.items.items {
                            if let Some(e) = arg.expr(self.cst) {
                                self.visit_expr(&e);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Drop the declaration range when the caller didn't ask for it.
///
/// If `include_declaration` is true, the input is returned unchanged.
/// Otherwise we look up `name` in `table` and filter out any range that
/// equals the symbol's `selection_range`.
pub fn filter_declaration(
    ranges: Vec<Range>,
    table: &crate::symbols::SymbolTable,
    name: &str,
    include_declaration: bool,
) -> Vec<Range> {
    if include_declaration {
        return ranges;
    }
    let decl = match table.find(name) {
        Some(s) => s.selection_range,
        None => return ranges,
    };
    ranges.into_iter().filter(|r| *r != decl).collect()
}

/// Find the identifier at `(line, character)` and return its range.
/// Used by `prepare_rename` — if the cursor is on whitespace or punctuation
/// (no enclosing identifier), return `None`.
pub fn identifier_range_at(source: &str, line: u32, character: u32) -> Option<Range> {
    let offset = position_to_byte_offset(source, line, character)?;
    let (cst, tu) = parse_typed(source)?;
    let mut finder = IdentifierAtFinder {
        source,
        cst: &cst,
        offset,
        best: None,
    };
    finder.visit_translation_unit(&tu);
    finder.best
}

struct IdentifierAtFinder<'a> {
    source: &'a str,
    cst: &'a Cst<'a>,
    offset: usize,
    best: Option<Range>,
}

impl<'a> IdentifierAtFinder<'a> {
    fn consider(&mut self, id: &Identifier) {
        if id.span.start <= self.offset && self.offset < id.span.end {
            let range = span_to_range(self.source, id.span.clone());
            let better = match &self.best {
                Some(best) => {
                    let best_len = (best.end.character - best.start.character)
                        + (best.end.line - best.start.line) * u32::MAX;
                    let cur_len = (range.end.character - range.start.character)
                        + (range.end.line - range.start.line) * u32::MAX;
                    cur_len < best_len
                }
                None => true,
            };
            if better {
                self.best = Some(range);
            }
        }
    }

    fn visit_translation_unit(&mut self, tu: &TranslationUnit) {
        for item in &tu.items {
            self.visit_top_level_item(item);
        }
    }

    fn visit_top_level_item(&mut self, item: &TopLevelItem) {
        match item {
            TopLevelItem::RuleDefinition(r) => self.consider(&r.name),
            TopLevelItem::FunctionDefinition(f) => self.visit_function_definition(f),
            TopLevelItem::ForwardDeclaration(f) => self.visit_forward_declaration(f),
            TopLevelItem::ClassDefinition(c) => self.visit_class_definition(c),
            TopLevelItem::Declaration(d) => self.visit_declaration(d),
            _ => {}
        }
    }

    fn visit_function_definition(&mut self, f: &FunctionDefinition) {
        self.consider(&f.declarator.direct.base());
        self.visit_declarator_params(&f.declarator.direct);
        self.visit_block_items(&f.body.inner);
    }

    fn visit_forward_declaration(&mut self, f: &ForwardDeclaration) {
        self.consider(&f.declarator.direct.base());
        self.visit_declarator_params(&f.declarator.direct);
    }

    fn visit_declarator_params(&mut self, direct: &xs_parser::ast::DirectDeclarator) {
        match direct {
            xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => {
                if let Some(pl) = &fd.params {
                    for (param, _) in &pl.inner.items.items {
                        match &param.inner {
                            ParameterInner::RegularParam(r) => {
                                if let Some(name) = &r.name {
                                    self.consider(name);
                                }
                            }
                            ParameterInner::FunctionPointerParam(fp) => self.consider(&fp.name),
                        }
                        if let Some(expr) = param.default_expr(self.cst) {
                            self.visit_expr(&expr);
                        }
                    }
                }
            }
            xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => {
                self.visit_declarator_params(&pd.inner.direct);
            }
            _ => {}
        }
    }

    fn visit_class_definition(&mut self, c: &ClassDefinition) {
        self.consider(&c.name);
        for member in &c.members.inner {
            match member {
                ClassMember::FunctionDefinition(f) => self.visit_function_definition(f),
                ClassMember::ForwardDeclaration(f) => self.visit_forward_declaration(f),
                ClassMember::FieldDeclaration(f) => {
                    self.consider(&f.declarator.direct.base());
                }
            }
        }
    }

    fn visit_declaration(&mut self, d: &Declaration) {
        for init in &d.init_declarator_list.items {
            self.consider(&init.declarator.direct.base());
            if let Some(expr) = init.expr(self.cst) {
                self.visit_expr(&expr);
            }
        }
    }

    fn visit_block_items(&mut self, items: &[xs_parser::ast::statement::BlockItem]) {
        use xs_parser::ast::statement::BlockItem;
        for item in items {
            match item {
                BlockItem::Declaration(d) => self.visit_declaration(d),
                BlockItem::ForwardDeclaration(f) => self.visit_forward_declaration(f),
                BlockItem::FunctionDefinition(f) => self.visit_function_definition(f),
                BlockItem::Statement(s) => self.visit_statement(s),
            }
        }
    }

    fn visit_statement(&mut self, s: &xs_parser::ast::statement::Statement) {
        use xs_parser::ast::statement::{ForInit, Statement};
        match s {
            Statement::Compound(c) => self.visit_block_items(&c.items.inner),
            Statement::Expression(es) => {
                if let Some(e) = es.expr.as_ref().and_then(|u| Expr::from_cst(self.cst, u.0)) {
                    self.visit_expr(&e);
                }
            }
            Statement::Return(r) => {
                if let Some(v) = &r.value {
                    if let Some(e) = Expr::from_cst(self.cst, v.0) {
                        self.visit_expr(&e);
                    }
                }
            }
            Statement::If(i) => {
                if let Some(e) = Expr::from_cst(self.cst, i.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_statement(&i.then);
                if let Some(else_) = &i.else_ {
                    self.visit_statement(else_);
                }
            }
            Statement::While(w) => {
                if let Some(e) = Expr::from_cst(self.cst, w.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_statement(&w.body);
            }
            Statement::For(f) => {
                match &f.init {
                    ForInit::Declaration(d) => self.visit_declaration(d),
                    ForInit::Expression(u) => {
                        if let Some(e) = Expr::from_cst(self.cst, u.0) {
                            self.visit_expr(&e);
                        }
                    }
                    ForInit::Empty => {}
                }
                if let Some(u) = &f.cond {
                    if let Some(e) = Expr::from_cst(self.cst, u.0) {
                        self.visit_expr(&e);
                    }
                }
                if let Some(u) = &f.post {
                    if let Some(e) = Expr::from_cst(self.cst, u.0) {
                        self.visit_expr(&e);
                    }
                }
                self.visit_statement(&f.body);
            }
            Statement::Switch(s) => {
                if let Some(e) = Expr::from_cst(self.cst, s.cond.inner.0) {
                    self.visit_expr(&e);
                }
                self.visit_block_items(&s.body.items.inner);
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, e: &Expr) {
        use xs_parser::ast::expr::*;
        match e {
            Expr::Identifier(id) => self.consider(&id.name),
            Expr::Unary(u) => self.visit_expr(&u.operand),
            Expr::Binary(b) => {
                self.visit_expr(&b.lhs);
                self.visit_expr(&b.rhs);
            }
            Expr::Conditional(c) => {
                self.visit_expr(&c.cond);
                if let Some(t) = &c.then {
                    self.visit_expr(t);
                }
                self.visit_expr(&c.else_);
            }
            Expr::Assignment(a) => {
                self.visit_expr(&a.lhs);
                self.visit_expr(&a.rhs);
            }
            Expr::Comma(c) => {
                for e in &c.exprs {
                    self.visit_expr(e);
                }
            }
            Expr::Paren(p) => self.visit_expr(&p.inner),
            Expr::Postfix(p) => {
                self.visit_expr(&p.target);
                if let PostfixInner::Call(call) = &p.inner {
                    if let Some(args) = &call.args {
                        for (arg, _) in &args.items.items {
                            if let Some(e) = arg.expr(self.cst) {
                                self.visit_expr(&e);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Convert a set of ranges into `Location`s anchored at `uri`.
pub fn to_locations(uri: &Uri, ranges: Vec<Range>) -> Vec<Location> {
    ranges
        .into_iter()
        .map(|range| Location {
            uri: uri.clone(),
            range,
        })
        .collect()
}

// Extension helpers on AST declarators that are not part of the public
// typed-AST API but are convenient for the walker.
trait DirectDeclaratorBasics {
    fn base(&self) -> &Identifier;
}

impl DirectDeclaratorBasics for xs_parser::ast::DirectDeclarator {
    fn base(&self) -> &Identifier {
        match self {
            xs_parser::ast::DirectDeclarator::IdentDeclarator(id) => &id.name,
            xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => &fd.base,
            xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => pd.inner.direct.base(),
        }
    }
}

trait ParameterDefaultExpr {
    fn default_expr(&self, cst: &Cst<'_>) -> Option<Expr>;
}

impl ParameterDefaultExpr for ParameterDeclaration {
    fn default_expr(&self, cst: &Cst<'_>) -> Option<Expr> {
        match &self.inner {
            ParameterInner::RegularParam(r) => r.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0)),
            ParameterInner::FunctionPointerParam(fp) => {
                fp.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::SymbolTable;
    use tower_lsp_server::ls_types::Position;

    #[test]
    fn find_finds_declaration_and_uses() {
        let src = "int helper(int a) { return a; }\n";
        let ranges = find_identifier_uses(src, "helper");
        assert_eq!(ranges.len(), 1, "expected exactly one `helper` occurrence");
    }

    #[test]
    fn find_finds_multiple_callsites() {
        let src = "int helper(int a) { return a; }\nvoid caller() { helper(1); helper(2); }\n";
        let ranges = find_identifier_uses(src, "helper");
        assert_eq!(ranges.len(), 3, "expected 3 `helper` occurrences, got {}", ranges.len());
    }

    #[test]
    fn filter_declaration_with_include_true() {
        let ranges = vec![
            Range::new(Position::new(0, 0), Position::new(0, 5)),
            Range::new(Position::new(1, 0), Position::new(1, 5)),
            Range::new(Position::new(2, 0), Position::new(2, 5)),
        ];
        let table = SymbolTable::default();
        let out = filter_declaration(ranges.clone(), &table, "anything", true);
        assert_eq!(out, ranges);
    }

    #[test]
    fn filter_declaration_with_include_false_drops_symbol_table_match() {
        let decl = Range::new(Position::new(0, 4), Position::new(0, 10));
        let ranges = vec![
            decl,
            Range::new(Position::new(1, 2), Position::new(1, 8)),
            Range::new(Position::new(2, 2), Position::new(2, 8)),
        ];
        let mut table = SymbolTable::default();
        table.symbols.push(crate::symbols::Symbol {
            name: "helper".to_string(),
            kind: crate::symbols::SymbolKind::Function,
            ty: "int".to_string(),
            params: vec![],
            class_owner: None,
            is_extern: false,
            is_mutable: false,
            is_static: false,
            is_forward: false,
            visibility: crate::symbols::Visibility::Public,
            full_range: decl,
            selection_range: decl,
            detail: "int helper".to_string(),
        });
        let out = filter_declaration(ranges, &table, "helper", false);
        assert_eq!(out.len(), 2);
        assert!(!out.contains(&decl), "declaration range should be filtered out");
    }

    #[test]
    fn identifier_range_at_returns_identifier_range() {
        let src = "int helper(int a) { return a; }\nvoid caller()\n{\n   helper(1);\n}\n";
        let r = identifier_range_at(src, 3, 5).expect("expected range");
        assert_eq!(r.start.line, 3);
        assert_eq!(r.start.character, 3);
        assert_eq!(r.end.line, 3);
        assert_eq!(r.end.character, 9);
    }

    #[test]
    fn identifier_range_at_returns_none_when_no_identifier() {
        let src = "int helper(int a) { return a; }\nvoid caller()\n{\n   helper(1);\n}\n";
        assert!(identifier_range_at(src, 3, 1).is_none());
    }
}
