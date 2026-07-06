//@ check-fail
void f() {
   int x = (1 + 2; //~ ERROR unclosed parenthesis '('
}
