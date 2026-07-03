use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use codespan_reporting::term::{self, Config};
use xs_parser::ast::format::format_translation_unit;
use xs_parser::ast::TranslationUnit;
use xs_parser::parser::Parser;

fn main() -> std::io::Result<()> {
    let source = r#"
extern const int cFoo = 5;
int gFoo = cFoo;

void bar() {
    int x = 1;
    for (i = 0; i < 10; i = i + 1) {
        x = x + i;
    }
}

int a, b = 2, c = 3;

vector pos = vector(1.0, 2.0, 3.0);

#if (defined(MY_INCLUDE) == false)
#define MY_INCLUDE
int included = 1;
#endif
"#;

    let mut diags = vec![];
    let cst = Parser::new(source, &mut diags).parse(&mut diags);

    println!("=== Parse diagnostics: {} ===", diags.len());
    let file = SimpleFile::new("<test>", source);
    let writer = StandardStream::stderr(ColorChoice::Never);
    let config = Config::default();
    for diag in &diags {
        term::emit_to_write_style(&mut writer.lock(), &config, &file, diag).ok();
    }

    println!("\n=== CST (first 2000 chars) ===");
    let s = cst.to_string();
    let truncated: String = s.chars().take(2000).collect();
    println!("{}", truncated);
    if s.chars().count() > 2000 {
        println!("... (truncated, total {} chars)", s.chars().count());
    }

    // ----- Format preservation round-trip -----
    //
    // Extract the typed AST's `TranslationUnit` and re-emit the source
    // through the proof-of-concept formatter. The success criterion for
    // the typed AST layer is that the formatter's output is byte-for-byte
    // identical to the input source — this validates the wrapper-token
    // scheme is sound.
    let tu = TranslationUnit::from_cst(&cst, xs_parser::parser::NodeRef::ROOT)
        .expect("TranslationUnit from clean source");
    let formatted = format_translation_unit(&cst, &tu);
    println!("\n=== Format round-trip ===");
    println!("source bytes:     {}", source.len());
    println!("formatted bytes:  {}", formatted.len());
    if formatted == source {
        println!("MATCH — byte-for-byte round-trip succeeded.");
    } else {
        println!("DRIFT — formatter output differs from input.");
        // Print a small diff window to help diagnose.
        for (i, (a, b)) in source.bytes().zip(formatted.bytes()).enumerate() {
            if a != b {
                let start = i.saturating_sub(20);
                let end = (i + 40).min(source.len()).min(formatted.len());
                println!(
                    "  first drift at offset {}:\n    source:    {:?}\n    formatted: {:?}",
                    i,
                    &source[start..end],
                    &formatted[start..end.min(formatted.len())]
                );
                break;
            }
        }
        std::process::exit(1);
    }

    Ok(())
}
