# XS language syntax

## Variable types

- int
- float
- bool
- string
- vector

```xs
vector v0 = cOriginVector; // Creates a copy of the origin vector.
vector v1 = vector(0.0, 0.0, 0.0);
vector v2 = vector(0.0); // y and z default to x.

// Components:
float a = v.x;
v.x = a;
v.x += a;

// Copy-assigned directly:
vector v3 = myOtherVector;

// Methods are called through . and return a new vector — no in-place mutation.
v = v.normalize();
float l = v.length();

// Multiplying a vector by a float multiplies each component:
v = v * l;
```

## Built-in functions

- length
- normalize
- dot(vector other)
- cross(vector other)
- distance(vector other)
- distanceXZ(vector other)
- distanceToLine(vector linePoint, vector direction)
- distanceToLineSegment(vector p1, vector p2)
- angleBetweenVector(vector other)
- angleAroundY()
- rotateXZ(float angleRadians)
- translateXZ(float radius, float angleRadians)

  
## Class

```xs
class Foo {
  int a = 1;
  int b = 5;

  void show() { aiEcho("a=" + a + ", b=" + b); }
}

void main() {
  Foo foo;
  foo.a = 4;
  foo.show();
}
```

## Array

```xs
int[] arr = default;
arr = new int(10, -1);

// With a class, no default value is needed:
Foo[] fooArr = new Foo(5);

arr[i] = 5;
aiEcho("arr[i]="+arr[i]);
int fooArrSize = fooArr.size();
```

### Built-in methods + return values.

- size() — number of elements.
- resize(int size, <type> defaultValue); for classes just resize(int size).
- add(<type> value) — index at which it was added.
- uniqueAdd(<type> value) — not supported on classes. Returns the index added at.
- insert(int index, <type> value) — index at which it was added.
- removeIndex(int index) — bool, whether it succeeded.
- removeValue(<type> value) — not supported on classes. Returns bool.
- find(<type> value) — not supported on classes. Returns index, or -1 if not found.
- clear()

## References

All variables are passed by copy unless the parameter has a `ref` modifier.

```xs
// References are only allowed on parameters (like C#).
// Use them with classes or vectors to avoid copying.
void test(ref int a) {
  a += 7;
}

void main() {
  int a = 5;
  test(a);
  // a is now 12.
  aiEcho("a=" + a);
}
```

## Rules

Rules are an XS unique feature that require extensive explanation, as such they have their own document: *"BANG documentation\XS documentation\XS rules functionality"*.

## Forward declarations

XS does NOT support implicit forward declarations like C/C++. If function `A` calls function `B`, then `B` must be either:

1. **Defined** before `A` in the same file, OR
2. **Declared** (signature only) with a trailing semicolon, placed earlier in the file.

A function declared `mutable` is the only exception — it can be called before its definition and redefined later.

```xs
// Forward-declaration block — typically placed right after the `extern` block.
void helper_foo(int x = -1);
int  helper_bar(int y = -1, int z = -1);

// Original code that calls them may follow.
void main() {
  helper_foo(1);
  int v = helper_bar(2, 3);
}

// Definitions come later in the file (or in another file in the same include root).
void helper_foo(int x = -1) { /* ... */ }
int  helper_bar(int y = -1, int z = -1) { /* ... */ }
```

**Failure mode:** if a call site precedes both a declaration and a definition, the engine rejects the mod on load with `Error 0310: invalid symbol lookup`. There is no standalone XS compiler to catch this earlier — only the game engine validates.

**Cross-file note:** the same rule applies across `include` boundaries; the included file's function must be defined (not just declared) before the call site in the including file. See the "Includes" section.

## Modifiers

- const — cannot be modified after assignment. Must be initialized from another constant (XS limitation).
  ```xs
  const int number = 3; // Fine.
  const int number = getNumberFromWherever(); // Compile error.
  const int number = someOtherVariable; // Compile error.
  ```
- static — usable inside functions; value persists across function exits.
- extern — global variable accessible from other source files.
- ref — parameter modifier; pass by reference instead of value.
- mutable — function modifier; allows the function to be redefined later with the same signature.

```xs
// Forward reference is allowed because the function is declared mutable.
mutable void helper(int planID = -1) {}

void main() {
  callHelper(15);
}

void callHelper(int planID = -1) {
  helper(planID);
}

void helper(int planID = -1) {
}
```

## Operators

Not all operators work on every type combination. Left operand → valid right operand types:

```xs
int num = 10;
num + 10.0; // Fine: float added to int.
num + vector(0.0, 0.0, 0.0); // Compile error: int + vector is not valid.
```

- add(+)
  - int + (int/float)
  - float + (int/float)
  - string + (int/float/bool/string/vector)
  - vector + vector
- subtract(-)
  - int - (int/float)
  - float - (int/float)
  - vector - vector
- multiply(\*)
  - int \* (int/float)
  - float \* (int/float)
  - vector \*(int/float)
- divide(/)
  - int / (int/float)
  - float / (int/float)
  - vector / (int/float)
- mod(%)
  - int % int
- shift left/right(>>/<<)
  - int >> int
- bitwise and(&)
  - int & int
  - bool & bool
- bitwise or(|)
  - int | int
  - bool | bool
- bitwise xor(^)
  - int ^ int
  - bool ^ bool
- neg(-)
  - -int

All arithmetic/bitwise operators above have compound assignment forms (e.g., `+=`, `&=`, `>>=`).

- equal(==)
  - int == (int/float)
  - float == (int/float)
  - bool == bool
  - string == string
  - vector == vector
- not equal(!=)
  - int != (int/float)
  - float != (int/float)
  - bool != bool
  - string != string
  - vector != vector
- less(<)
  - int < (int/float)
  - float < (int/float)
  - string < string
- less equal than(<=)
  - int <= (int/float)
  - float <= (int/float)
  - string <= string
- greater(>)
  - int > (int/float)
  - float > (int/float)
  - string > string
- greater equal than(>=)
  - int >= (int/float)
  - float >= (int/float)
  - string >= string
- and(&&) — short-circuits.
  - bool && bool
- or(||) — short-circuits.
  - bool || bool
- not(!)
  - !(bool)
- ternary operator — `condition ? expression1 : expression2`

Prefer same-type operands — e.g. use `5.0` not `5` when adding to a float.

## Conditions

- if

```xs
if (expression) {
}
else if (expression) {
}
else {
}
```

Constant expressions can be evaluated at compile time:

```xs
if (cNumberPlayers == 5) {
  // This block is only compiled when number of players equals 5 — equivalent to #if.
}

// Partial constant check: when cMyCiv == cCivZeus the RHS is dropped at compile time
// because the LHS short-circuits.
if (cMyCiv == cCivZeus || someNonConstantCheck() == false)
```

Constant-function results are also folded:

```xs
bool civNotZeusAndLessThan5Players()
{
return(cMyCiv != cCivZeus && cNumberPlayers < 5);
}

// Evaluated at compile time.
if (civNotZeusAndLessThan5Players() == false)
```

- switch

```xs
switch (expression) {
  case constant: {
    break;
  }
  // Unlike C++, execution does not fall through into the next case.
  case constant2: {
  }
  // Multiple values per case:
  case constant3:
  case constant4: {
    break;
  }
}
```

## Loops

### for

```xs
for (int i = 0; i < 10; i++) {
}

// Cache function-call bounds — the function would otherwise be called every iteration.
int numBases = kbBaseGetNumber(cMyID);
for (int i = 0; i < numBases; i++)
```

`arr.size()` is inlined for non-class arrays; for class arrays it involves a division (size stored as array size * class size), so cache it the same way.

### while

```xs
while (i < 10) {
}
```

### do while

```xs
do {
} while (i < 10)
```

## Includes

XS supports including files into other files. Each runtime has its own root folder from which all includes are relative:

- AI: `"game\ai"`
- TR: `"game\data\trigger"`
- RM: `"game\random_maps"`

```xs
// Includes do not use the # prefix, even though they are preprocessor directives.
include "core/core.xs";
```

Functions can be used across files as long as their definition precedes their use. To use a function before its definition, mark it `mutable`.

Variables cannot be used across files by default. Mark them with `extern`, and the definition must still precede the use; forward declaration of variables is not allowed.

## Preprocessor directives

```xs
// Defines a new token.
#define TOKEN_NAME

// True if the token has been defined. Can also be used at runtime.
defined(token)

// Evaluates a constant expression (or another defined() directive).
// The block is ignored if the expression is false.
#if (constant expression)
// Some code...
#elif (constant expression)
// Some code...
#else
// Some code...
#endif

// Example:
#define TEST
#if (defined(TEST) == true)
// Compiled in only when TEST is defined.
#endif
```
