// Unit tests for xs-parser recovery messages

#[cfg(test)]
mod tests {
    use crate::ast::type_table::TypeTable;
    use crate::parser::{compact_recovery_messages, Diagnostic, Parser};
    use codespan_reporting::diagnostic::{LabelStyle, Severity};

    fn parse_with_recovery(source: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
        compact_recovery_messages(source, &mut diags);
        diags
    }

    fn line_at_offset(source: &str, offset: usize) -> usize {
        source[..offset.min(source.len())].lines().count()
    }

    fn primary_start_line(source: &str, diag: &Diagnostic) -> usize {
        let label = diag
            .labels
            .iter()
            .find(|l| l.style == LabelStyle::Primary)
            .expect("diagnostic must have a primary label");
        line_at_offset(source, label.range.start)
    }

    #[test]
    fn test_missing_semicolon_message_expression_statement() {
        let source = "void g() { xsSetPointerPrice(1) }";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let matching = errors.iter().find(|d| {
            d.message == "missing ';'" && primary_start_line(source, d) == 1
        });
        assert!(
            matching.is_some(),
            "expected an error 'missing \";\"' on line 1 for expression statement, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_missing_closing_brace_message() {
        let source = "void f()\n{\n   return;\n";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let matching = errors.iter().find(|d| {
            d.message == "missing closing brace '}'" && primary_start_line(source, d) == 1
        });
        assert!(
            matching.is_some(),
            "expected a 'missing closing brace' error on line 1, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_missing_opening_brace_message() {
        let source = "void f()\n   return 5;\n";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let matching = errors.iter().find(|d| {
            d.message == "missing opening brace '{'" && primary_start_line(source, d) == 1
        });
        assert!(
            matching.is_some(),
            "expected a 'missing opening brace' error on line 1, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_unclosed_string_message() {
        let source = "string s = \"hello\n";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let matching = errors.iter().find(|d| {
            d.message == "unclosed string literal" && primary_start_line(source, d) == 1
        });
        assert!(
            matching.is_some(),
            "expected an 'unclosed string literal' error on line 1, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_unclosed_paren_message() {
        let source = "void f() {\n   int x = (1 + 2;\n}\n";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let matching = errors.iter().find(|d| {
            d.message == "unclosed parenthesis '('" && primary_start_line(source, d) == 2
        });
        assert!(
            matching.is_some(),
            "expected an 'unclosed parenthesis' error on line 2, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }
}
