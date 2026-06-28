/**
 * @file XS grammar for tree-sitter
 * Forked from tree-sitter-c. XS is the scripting language used by Age of
 * Mythology: Retold. It is a strict subset of C with XS-specific keywords
 * (`rule`, `class`, `mutable`, `default`, `new`), XS primitive types
 * (`string`, `vector`), an `include "..."` directive (no `#`), and a
 * `rule <name>\n<modifiers>\n{ body }` block form (BANG-style).
 * @author houtamelo
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const PREC = {
  PAREN_DECLARATOR: -10,
  ASSIGNMENT: -2,
  CONDITIONAL: -1,
  DEFAULT: 0,
  LOGICAL_OR: 1,
  LOGICAL_AND: 2,
  INCLUSIVE_OR: 3,
  EXCLUSIVE_OR: 4,
  BITWISE_AND: 5,
  EQUAL: 6,
  RELATIONAL: 7,
  SHIFT: 9,
  ADD: 10,
  MULTIPLY: 11,
  CAST: 12,
  UNARY: 14,
  CALL: 15,
  FIELD: 16,
  SUBSCRIPT: 17,
  NEW: 18,
};

module.exports = grammar({
  name: 'xs',

  conflicts: $ => [
    [$.type_specifier, $._declarator],
    [$.type_specifier, $._declarator, $._type_identifier],
    [$.type_specifier, $.expression],
    [$.type_specifier, $.expression, $._type_identifier],
    [$.type_specifier, $._type_identifier],
    [$.function_declarator, $._function_declaration_declarator],
    [$._block_item, $.statement],
    [$._top_level_item, $._top_level_statement],
    [$.type_specifier, $._top_level_expression_statement],
    [$.class_specifier, $.type_specifier],
    [$._top_level_item, $.class_specifier],
    [$._top_level_item, $.rule_definition],
    [$._assignment_left_expression, $.type_specifier],
    [$._declaration_specifiers, $._assignment_left_expression],
    [$.array_type, $.expression],
    [$.array_type, $.expression, $._type_identifier],
    [$.field_declaration],
    [$.function_pointer_type, $.type_specifier],
  ],

  extras: $ => [
    /\s|\\\r?\n/,
    $.comment,
  ],

  inline: $ => [
    $._type_identifier,
    $._field_identifier,
    $._statement_identifier,
    $._non_case_statement,
    $._assignment_left_expression,
    $._expression_not_binary,
  ],

  supertypes: $ => [
    $.expression,
    $.statement,
    $.type_specifier,
    $._declarator,
  ],

  word: $ => $.identifier,

  rules: {
    translation_unit: $ => repeat($._top_level_item),

    _top_level_item: $ => choice(
      $.function_definition,
      $.rule_definition,
      $.class_specifier,
      $.declaration,
      $._top_level_statement,
      $._empty_declaration,
      $.preproc_if,
      $.preproc_ifdef,
      $.include_directive,
      $.preproc_def,
    ),

    _block_item: $ => choice(
      $.function_definition,
      $.declaration,
      $.statement,
      $._empty_declaration,
      $.preproc_if,
      $.preproc_ifdef,
      $.preproc_def,
    ),

    // Preprocessor

    include_directive: $ => seq(
      'include',
      field('path', $.string_literal),
      ';',
    ),

    preproc_def: $ => seq(
      preprocessor('define'),
      field('name', $.identifier),
      token.immediate(/\r?\n/),
    ),

    ...preprocIf($ => $._block_item),

    preproc_arg: _ => token(prec(-1, /\S([^/\n]|\/[^*]|\\\r?\n)*/)),

    _preproc_expression: $ => choice(
      $.identifier,
      $.number_literal,
      $.preproc_defined,
      alias($.preproc_unary_expression, $.unary_expression),
      alias($.preproc_binary_expression, $.binary_expression),
      alias($.preproc_parenthesized_expression, $.parenthesized_expression),
    ),

    preproc_parenthesized_expression: $ => seq(
      '(',
      $._preproc_expression,
      ')',
    ),

    preproc_defined: $ => choice(
      prec(PREC.CALL, seq('defined', '(', $.identifier, ')')),
      seq('defined', $.identifier),
    ),

    preproc_unary_expression: $ => prec.left(PREC.UNARY, seq(
      field('operator', choice('!', '~', '-', '+')),
      field('argument', $._preproc_expression),
    )),

    preproc_binary_expression: $ => {
      const table = [
        ['+', PREC.ADD],
        ['-', PREC.ADD],
        ['*', PREC.MULTIPLY],
        ['/', PREC.MULTIPLY],
        ['%', PREC.MULTIPLY],
        ['||', PREC.LOGICAL_OR],
        ['&&', PREC.LOGICAL_AND],
        ['|', PREC.INCLUSIVE_OR],
        ['^', PREC.EXCLUSIVE_OR],
        ['&', PREC.BITWISE_AND],
        ['==', PREC.EQUAL],
        ['!=', PREC.EQUAL],
        ['>', PREC.RELATIONAL],
        ['>=', PREC.RELATIONAL],
        ['<=', PREC.RELATIONAL],
        ['<', PREC.RELATIONAL],
        ['<<', PREC.SHIFT],
        ['>>', PREC.SHIFT],
      ];

      return choice(...table.map(([operator, precedence]) => {
        return prec.left(precedence, seq(
          field('left', $._preproc_expression),
          field('operator', operator),
          field('right', $._preproc_expression),
        ));
      }));
    },

    // Main grammar

    function_definition: $ => seq(
      $._declaration_specifiers,
      field('declarator', $._function_declarator),
      field('body', $.compound_statement),
    ),

    rule_definition: $ => seq(
      'rule',
      field('name', $.identifier),
      repeat($.rule_modifier),
      field('body', $.compound_statement),
    ),

    rule_modifier: $ => choice(
      seq('minInterval', field('value', $.number_literal)),
      seq('maxInterval', field('value', $.number_literal)),
      seq('minIntervalMS', field('value', $.number_literal)),
      seq('maxIntervalMS', field('value', $.number_literal)),
      seq('priority', field('value', $.number_literal)),
      'highFrequency',
      'active',
      'inactive',
      'runImmediately',
      seq('group', field('name', $.identifier)),
    ),

    class_specifier: $ => prec.right(seq(
      'class',
      field('name', $._type_identifier),
      field('body', $.field_declaration_list),
    )),

    declaration: $ => seq(
      $._declaration_specifiers,
      commaSep1(field('declarator', choice(
        $._declaration_declarator,
        $.init_declarator,
      ))),
      ';',
    ),

    _declaration_modifiers: $ => choice(
      $.storage_class_specifier,
      $.type_qualifier,
    ),

    _declaration_specifiers: $ => prec.right(seq(
      repeat($._declaration_modifiers),
      field('type', $.type_specifier),
      repeat($._declaration_modifiers),
    )),

    _function_declarator: $ => prec.right(1,
      seq(
        field('declarator', $.identifier),
        field('parameters', $.parameter_list),
      ),
    ),

    _declarator: $ => choice(
      $.function_declarator,
      $.parenthesized_declarator,
      $.identifier,
    ),

    _declaration_declarator: $ => choice(
      alias($._function_declaration_declarator, $.function_declarator),
      $.parenthesized_declarator,
      $.identifier,
    ),

    function_declarator: $ => prec.right(1,
      seq(
        field('declarator', $._declarator),
        field('parameters', $.parameter_list),
      ),
    ),

    _function_declaration_declarator: $ => prec.right(1,
      seq(
        field('declarator', $._declarator),
        field('parameters', $.parameter_list),
      )),

    parenthesized_declarator: $ => prec.dynamic(PREC.PAREN_DECLARATOR, seq(
      '(',
      $._declarator,
      ')',
    )),

    init_declarator: $ => seq(
      field('declarator', $._declarator),
      '=',
      field('value', $.expression),
    ),

    compound_statement: $ => seq(
      '{',
      repeat($._block_item),
      '}',
    ),

    storage_class_specifier: _ => choice(
      'extern',
      'static',
      'mutable',
    ),

    type_qualifier: $ => choice(
      'const',
      'ref',
    ),

    type_specifier: $ => choice(
      $.primitive_type,
      $._type_identifier,
      $.array_type,
    ),

    array_type: $ => prec(-1, seq(
      field('element', choice($.primitive_type, $._type_identifier)),
      '[',
      ']',
    )),

    primitive_type: _ => token(choice(
      'bool',
      'int',
      'float',
      'string',
      'vector',
      'void',
    )),

    function_pointer_type: $ => seq(
      field('return', $.type_specifier),
      field('parameters', $.parameter_list),
    ),

    field_declaration_list: $ => seq(
      '{',
      repeat($.field_declaration),
      '}',
    ),

    field_declaration: $ => seq(
      $._declaration_specifiers,
      optional(seq(
        $._field_declarator,
        optional(choice(
          seq($.compound_statement),
          seq('=', $.expression),
        )),
      )),
      optional(';'),
    ),

    _field_declarator: $ => choice(
      $.function_declarator,
      $.identifier,
    ),

    variadic_parameter: _ => '...',

    parameter_list: $ => seq(
      '(',
      commaSep(choice($.parameter_declaration, $.variadic_parameter)),
      ')',
    ),

    parameter_declaration: $ => seq(
      repeat($._declaration_modifiers),
      field('type', choice($.type_specifier, $.function_pointer_type)),
      field('declarator', optional($.identifier)),
      optional(seq('=', field('default', $.expression))),
    ),

    // Statements

    statement: $ => choice(
      $.case_statement,
      $._non_case_statement,
    ),

    _non_case_statement: $ => choice(
      $.labeled_statement,
      $.compound_statement,
      $.expression_statement,
      $.if_statement,
      $.switch_statement,
      $.do_statement,
      $.while_statement,
      $.for_statement,
      $.return_statement,
      $.break_statement,
      $.continue_statement,
    ),

    _top_level_statement: $ => choice(
      $.case_statement,
      $.labeled_statement,
      $.compound_statement,
      alias($._top_level_expression_statement, $.expression_statement),
      $.if_statement,
      $.switch_statement,
      $.do_statement,
      $.while_statement,
      $.for_statement,
      $.return_statement,
      $.break_statement,
      $.continue_statement,
    ),

    labeled_statement: $ => seq(
      field('label', $._statement_identifier),
      ':',
      choice($.declaration, $.statement),
    ),

    _top_level_expression_statement: $ => seq(
      optional($._expression_not_binary),
      ';',
    ),

    expression_statement: $ => seq(
      optional(choice(
        $.expression,
        $.comma_expression,
      )),
      ';',
    ),

    if_statement: $ => prec.right(seq(
      'if',
      field('condition', $.parenthesized_expression),
      field('consequence', $.statement),
      optional(field('alternative', $.else_clause)),
    )),

    else_clause: $ => seq('else', $.statement),

    switch_statement: $ => seq(
      'switch',
      field('condition', $.parenthesized_expression),
      field('body', $.compound_statement),
    ),

    case_statement: $ => prec.right(seq(
      choice(
        seq('case', field('value', $.expression)),
        'default',
      ),
      ':',
      repeat(choice(
        $._non_case_statement,
        $.declaration,
      )),
    )),

    while_statement: $ => seq(
      'while',
      field('condition', $.parenthesized_expression),
      field('body', $.statement),
    ),

    do_statement: $ => seq(
      'do',
      field('body', $.statement),
      'while',
      field('condition', $.parenthesized_expression),
      ';',
    ),

    for_statement: $ => seq(
      'for',
      '(',
      $._for_statement_body,
      ')',
      field('body', $.statement),
    ),
    _for_statement_body: $ => seq(
      choice(
        field('initializer', $.declaration),
        seq(field('initializer', optional(choice($.expression, $.comma_expression))), ';'),
      ),
      field('condition', optional(choice($.expression, $.comma_expression))),
      ';',
      field('update', optional(choice($.expression, $.comma_expression))),
    ),

    return_statement: $ => seq(
      'return',
      optional(choice($.expression, $.comma_expression)),
      ';',
    ),

    break_statement: _ => seq('break', ';'),

    continue_statement: _ => seq('continue', ';'),

    // Expressions

    expression: $ => choice(
      $._expression_not_binary,
      $.binary_expression,
    ),

    _expression_not_binary: $ => choice(
      $.conditional_expression,
      $.assignment_expression,
      $.unary_expression,
      $.update_expression,
      $.cast_expression,
      $.new_expression,
      $.subscript_expression,
      $.call_expression,
      $.field_expression,
      $.identifier,
      $.number_literal,
      $.string_literal,
      $.true,
      $.false,
      $.null,
      $.parenthesized_expression,
      $.lambda_expression,
    ),

    comma_expression: $ => seq(
      field('left', $.expression),
      ',',
      field('right', choice($.expression, $.comma_expression)),
    ),

    conditional_expression: $ => prec.right(PREC.CONDITIONAL, seq(
      field('condition', $.expression),
      '?',
      optional(field('consequence', choice($.expression, $.comma_expression))),
      ':',
      field('alternative', $.expression),
    )),

    _assignment_left_expression: $ => choice(
      $.identifier,
      $.call_expression,
      $.field_expression,
      $.subscript_expression,
      $.parenthesized_expression,
    ),

    assignment_expression: $ => prec.right(PREC.ASSIGNMENT, seq(
      field('left', $._assignment_left_expression),
      field('operator', choice(
        '=',
        '*=',
        '/=',
        '%=',
        '+=',
        '-=',
        '<<=',
        '>>=',
        '&=',
        '^=',
        '|=',
      )),
      field('right', $.expression),
    )),

    unary_expression: $ => prec.left(PREC.UNARY, seq(
      field('operator', choice('!', '~', '-', '+')),
      field('argument', $.expression),
    )),

    binary_expression: $ => {
      const table = [
        ['+', PREC.ADD],
        ['-', PREC.ADD],
        ['*', PREC.MULTIPLY],
        ['/', PREC.MULTIPLY],
        ['%', PREC.MULTIPLY],
        ['||', PREC.LOGICAL_OR],
        ['&&', PREC.LOGICAL_AND],
        ['|', PREC.INCLUSIVE_OR],
        ['^', PREC.EXCLUSIVE_OR],
        ['&', PREC.BITWISE_AND],
        ['==', PREC.EQUAL],
        ['!=', PREC.EQUAL],
        ['>', PREC.RELATIONAL],
        ['>=', PREC.RELATIONAL],
        ['<=', PREC.RELATIONAL],
        ['<', PREC.RELATIONAL],
        ['<<', PREC.SHIFT],
        ['>>', PREC.SHIFT],
      ];

      return choice(...table.map(([operator, precedence]) => {
        return prec.left(precedence, seq(
          field('left', $.expression),
          field('operator', operator),
          field('right', $.expression),
        ));
      }));
    },

    update_expression: $ => {
      const argument = field('argument', $.expression);
      const operator = field('operator', choice('--', '++'));
      return prec.right(PREC.UNARY, choice(
        seq(operator, argument),
        seq(argument, operator),
      ));
    },

    cast_expression: $ => prec(PREC.CAST, seq(
      '(',
      field('type', $.type_descriptor),
      ')',
      field('value', $.expression),
    )),

    type_descriptor: $ => seq(
      repeat($.type_qualifier),
      field('type', $.type_specifier),
      repeat($.type_qualifier),
    ),

    subscript_expression: $ => prec(PREC.SUBSCRIPT, seq(
      field('argument', $.expression),
      '[',
      field('index', $.expression),
      ']',
    )),

    call_expression: $ => prec(PREC.CALL, seq(
      field('function', $.expression),
      field('arguments', $.argument_list),
    )),

    new_expression: $ => prec(PREC.NEW, seq(
      'new',
      field('type', $.type_specifier),
      field('arguments', $.argument_list),
    )),

    argument_list: $ => seq('(', commaSep(choice($.expression, $.compound_statement)), ')'),

    field_expression: $ => seq(
      prec(PREC.FIELD, seq(
        field('argument', $.expression),
        field('operator', choice('.', '->')),
      )),
      field('field', $._field_identifier),
    ),

    parenthesized_expression: $ => seq(
      '(',
      choice($.expression, $.comma_expression, $.compound_statement),
      ')',
    ),

    lambda_expression: $ => seq(
      '[',
      ']',
      optional(field('parameters', $.parameter_list)),
      optional(seq('->', field('return', $.type_specifier))),
      field('body', $.compound_statement),
    ),

    number_literal: _ => {
      const separator = '\'';
      const hex = /[0-9a-fA-F]/;
      const decimal = /[0-9]/;
      const hexDigits = seq(repeat1(hex), repeat(seq(separator, repeat1(hex))));
      const decimalDigits = seq(repeat1(decimal), repeat(seq(separator, repeat1(decimal))));
      return token(seq(
        optional(/[-\+]/),
        optional(choice(/0[xX]/, /0[bB]/)),
        choice(
          seq(
            choice(
              decimalDigits,
              seq(/0[bB]/, decimalDigits),
              seq(/0[xX]/, hexDigits),
            ),
            optional(seq('.', optional(hexDigits))),
          ),
          seq('.', decimalDigits),
        ),
        optional(seq(
          /[eEpP]/,
          optional(seq(
            optional(/[-\+]/),
            hexDigits,
          )),
        )),
        /[uUlLwWfFbBdD]*/,
      ));
    },

    string_literal: $ => seq(
      '"',
      repeat(choice(
        alias(token.immediate(prec(1, /[^\\"\n]+/)), $.string_content),
        $.escape_sequence,
      )),
      '"',
    ),

    escape_sequence: _ => token(prec(1, seq(
      '\\',
      choice(
        /[^xuU]/,
        /\d{2,3}/,
        /x[0-9a-fA-F]{1,4}/,
        /u[0-9a-fA-F]{4}/,
        /U[0-9a-fA-F]{8}/,
      ),
    ))),

    true: _ => token(choice('TRUE', 'true')),
    false: _ => token(choice('FALSE', 'false')),
    null: _ => choice('NULL', 'nullptr'),

    identifier: _ =>
      /(\p{XID_Start}|\$|_|\\u[0-9A-Fa-f]{4}|\\U[0-9a-fA-F]{8})(\p{XID_Continue}|\$|\\u[0-9A-Fa-f]{4}|\\U[0-9a-fA-F]{8})*/,

    _type_identifier: $ => alias(
      $.identifier,
      $.type_identifier,
    ),
    _field_identifier: $ => alias($.identifier, $.field_identifier),
    _statement_identifier: $ => alias($.identifier, $.statement_identifier),

    _empty_declaration: $ => seq(
      $.type_specifier,
      ';',
    ),

    comment: _ => token(choice(
      seq('//', /(\\+(.|\r?\n)|[^\\\n])*/),
      seq(
        '/*',
        /[^*]*\*+([^/*][^*]*\*+)*/,
        '/',
      ),
    )),
  },
});

/**
 * @param {RuleBuilder<string>} content
 *
 * @returns {RuleBuilders<string, string>}
 */
function preprocIf(content) {
  function alternativeBlock($) {
    return choice(
      $.preproc_else,
      $.preproc_elif,
    );
  }

  return {
    preproc_if: $ => prec(0, seq(
      preprocessor('if'),
      field('condition', $._preproc_expression),
      '\n',
      repeat(content($)),
      field('alternative', optional(alternativeBlock($))),
      preprocessor('endif'),
    )),

    preproc_ifdef: $ => prec(0, seq(
      choice(preprocessor('ifdef'), preprocessor('ifndef')),
      field('name', $.identifier),
      repeat(content($)),
      field('alternative', optional(alternativeBlock($))),
      preprocessor('endif'),
    )),

    preproc_else: $ => prec(0, seq(
      preprocessor('else'),
      repeat(content($)),
    )),

    preproc_elif: $ => prec(0, seq(
      preprocessor('elif'),
      field('condition', $._preproc_expression),
      '\n',
      repeat(content($)),
      field('alternative', optional(alternativeBlock($))),
    )),
  };
}

/**
 * Creates a preprocessor regex rule
 *
 * @param {RegExp | Rule | string} command
 *
 * @returns {AliasRule}
 */
function preprocessor(command) {
  return alias(new RegExp('#[ \t]*' + command), '#' + command);
}

/**
 * Creates a rule to optionally match one or more of the rules separated by a comma
 *
 * @param {Rule} rule
 *
 * @returns {ChoiceRule}
 */
function commaSep(rule) {
  return optional(commaSep1(rule));
}

/**
 * Creates a rule to match one or more of the rules separated by a comma
 *
 * @param {Rule} rule
 *
 * @returns {SeqRule}
 */
function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}

module.exports.PREC = PREC;
module.exports.preprocIf = preprocIf;
module.exports.preprocessor = preprocessor;
module.exports.commaSep = commaSep;
module.exports.commaSep1 = commaSep1;
