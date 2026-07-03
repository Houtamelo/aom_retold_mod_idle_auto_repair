use xs_parser::parser::{Parser, Rule};
use xs_parser::lexer::Token;

fn main() -> std::io::Result<()> {
    // Regression cases for the class_specifier fix
    // (openspec/changes/2026-07-03-fix-class-specifier-no-trailing-semi/).
    // Real XS does not use trailing `;` after class definitions.
    for source in &[
        // Synthetic class shapes — all should produce 0 diagnostics.
        "class A { int x; }",
        "class A { int x; int y; }",
        "class A { }",
        "class A { int x = 5; void foo() {} }",
        // Backward-compat: trailing `;` (rare) should still parse.
        "class A { int x; };",
        "class A { int x; int y; };",
    ] {
        let mut diags = vec![];
        let _cst = Parser::new(source, &mut diags).parse(&mut diags);
        eprintln!("source: {:?} -> {} diags", source, diags.len());
        for d in &diags {
            eprintln!("    {}", d.message);
        }
    }

    // Phase 3 expression diagnostics — print the actual CST shape
    // for each expression kind so we know what `from_cst` must walk.
    let expr_sources: &[(&str, &str)] = &[
        // Primary expressions
        ("int x = 5;", "IntConst"),
        ("float f = 3.14;", "FloatConst"),
        ("string s = \"hello\";", "StringLiteral"),
        ("bool b = true;", "TrueLiteral"),
        ("bool b = false;", "FalseLiteral"),
        ("bool b = null;", "NullLiteral"),
        ("int x = y;", "IdentifierExpr"),
        ("int x = (5);", "ParenExpr"),
        ("int x = new Foo();", "NewExpr"),
        ("int x = default;", "DefaultExpr"),
        ("int x = vector(1, 2, 3);", "VectorLiteral"),
        ("void gFoo = []() { x; };", "LambdaExpr"),
        // Unary
        ("int y = -x;", "UnaryOp Minus"),
        ("bool b = !cond;", "UnaryOp Excl"),
        ("int y = ~x;", "UnaryOp Tilde"),
        ("int y = ++x;", "PreIncOrDec"),
        ("int y = --x;", "PreIncOrDec"),
        // Postfix
        ("int y = a.b;", "FieldExpr"),
        ("int y = a[0];", "SubscriptExpr"),
        ("int y = foo(a, b);", "CallExpr"),
        ("int y = x++;", "PostIncExpr"),
        ("int y = x--;", "PostDecExpr"),
        // Binary
        ("bool b = a == b;", "Equality"),
        ("bool b = a < b;", "Relational"),
        ("int y = a + b;", "Additive"),
        ("int y = a * b;", "Multiplicative"),
        ("bool b = a && b;", "LogicalAnd"),
        ("bool b = a || b;", "LogicalOr"),
        // Conditional + assignment
        ("int y = a ? b : c;", "Conditional"),
        ("int y = a = 5;", "Assignment (=)"),
        ("int y = a += 5;", "Assignment (+=)"),
        // Comma
        ("for (i = 0, j = 0; i < 10; i = i + 1) { }", "CommaExpr in for-init"),
    ];

    for (source, _label) in expr_sources {
        eprintln!("\n========= expr source: {:?} =========", source);
        let mut diags = vec![];
        let cst = Parser::new(source, &mut diags).parse(&mut diags);
        if !diags.is_empty() {
            eprintln!("  diagnostics:");
            for d in &diags {
                eprintln!("    {}", d.message);
            }
        }
        print_tree(&cst, 0, xs_parser::parser::NodeRef::ROOT);
    }

    Ok(())
}

fn print_tree(cst: &xs_parser::parser::Cst<'_>, depth: usize, node: xs_parser::parser::NodeRef) {
    use xs_parser::parser::Node;
    let indent = "  ".repeat(depth);
    match cst.get(node) {
        Node::Token(t, _) => {
            let span = cst.span(node);
            eprintln!("{}{:?} @ {}..{}", indent, t, span.start, span.end);
        }
        Node::Rule(r, _) => {
            let span = cst.span(node);
            eprintln!("{}{:?} @ {}..{} {{", indent, r, span.start, span.end);
            let mut children: Vec<_> = cst.children(node).collect();
            // Sort children by start to get readable output.
            children.sort_by_key(|c| cst.span(*c).start);
            for c in children {
                print_tree(cst, depth + 1, c);
            }
            eprintln!("{}}}", indent);
        }
    }
    let _ = Token::Void; // Silence unused-import.
    let _ = Rule::Expression; // Silence unused-import.
}
