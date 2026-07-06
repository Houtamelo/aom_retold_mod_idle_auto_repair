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
    fn test_missing_semicolon_message() {
        let source = "void f()\n{\n   return 5\n}";
        let diags = parse_with_recovery(source);

        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        let matching = errors.iter().find(|d| {
            d.message == "missing ';'" && primary_start_line(source, d) == 3
        });
        assert!(
            matching.is_some(),
            "expected an error 'missing \";\"' on line 3, got {:?}",
            errors
                .iter()
                .map(|d| (&d.message, primary_start_line(source, d)))
                .collect::<Vec<_>>()
        );
    }
}
