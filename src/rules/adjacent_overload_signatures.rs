use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn adjacent_overload_signatures_rule() -> Arc<dyn Rule> {
    rule! {
        name => "adjacent-overload-signatures",
        languages => [Typescript],
        messages => [
            adjacent_signature => "All {{name}} signatures should be adjacent.",
        ],
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_adjacent_overload_signatures_rule() {
        RuleTester::run(
            adjacent_overload_signatures_rule(),
            rule_tests! {
                valid => [
                    {
                      code => r#"
			function error(a: string);
			function error(b: number);
			function error(ab: string | number) {}
			export { error };
                      "#,
                      environment => { source_type => "module" },
                    },
                    {
                      code => r#"
			import { connect } from 'react-redux';
			export interface ErrorMessageModel {
			  message: string;
			}
			function mapStateToProps() {}
			function mapDispatchToProps() {}
			export default connect(mapStateToProps, mapDispatchToProps)(ErrorMessage);
                      "#,
                      environment => { source_type => "module" },
                    },
                    r#"
			export const foo = 'a',
			  bar = 'b';
			export interface Foo {}
			export class Foo {}
                    "#,
                    r#"
			export interface Foo {}
			export const foo = 'a',
			  bar = 'b';
			export class Foo {}
                    "#,
                    r#"
			const foo = 'a',
			  bar = 'b';
			interface Foo {}
			class Foo {}
                    "#,
                    r#"
			interface Foo {}
			const foo = 'a',
			  bar = 'b';
			class Foo {}
                    "#,
                    r#"
			export class Foo {}
			export class Bar {}
			export type FooBar = Foo | Bar;
                    "#,
                    r#"
			export interface Foo {}
			export class Foo {}
			export class Bar {}
			export type FooBar = Foo | Bar;
                    "#,
                    r#"
			export function foo(s: string);
			export function foo(n: number);
			export function foo(sn: string | number) {}
			export function bar(): void {}
			export function baz(): void {}
                    "#,
                    r#"
			function foo(s: string);
			function foo(n: number);
			function foo(sn: string | number) {}
			function bar(): void {}
			function baz(): void {}
                    "#,
                    r#"
			declare function foo(s: string);
			declare function foo(n: number);
			declare function foo(sn: string | number);
			declare function bar(): void;
			declare function baz(): void;
                    "#,
                    r#"
			declare module 'Foo' {
			  export function foo(s: string): void;
			  export function foo(n: number): void;
			  export function foo(sn: string | number): void;
			  export function bar(): void;
			  export function baz(): void;
			}
                    "#,
                    r#"
			declare namespace Foo {
			  export function foo(s: string): void;
			  export function foo(n: number): void;
			  export function foo(sn: string | number): void;
			  export function bar(): void;
			  export function baz(): void;
			}
                    "#,
                    r#"
			type Foo = {
			  foo(s: string): void;
			  foo(n: number): void;
			  foo(sn: string | number): void;
			  bar(): void;
			  baz(): void;
			};
                    "#,
                    r#"
			type Foo = {
			  foo(s: string): void;
			  ['foo'](n: number): void;
			  foo(sn: string | number): void;
			  bar(): void;
			  baz(): void;
			};
                    "#,
                    r#"
			interface Foo {
			  (s: string): void;
			  (n: number): void;
			  (sn: string | number): void;
			  foo(n: number): void;
			  bar(): void;
			  baz(): void;
			}
                    "#,
                    r#"
			interface Foo {
			  (s: string): void;
			  (n: number): void;
			  (sn: string | number): void;
			  foo(n: number): void;
			  bar(): void;
			  baz(): void;
			  call(): void;
			}
                    "#,
                    r#"
			interface Foo {
			  foo(s: string): void;
			  foo(n: number): void;
			  foo(sn: string | number): void;
			  bar(): void;
			  baz(): void;
			}
                    "#,
                    r#"
			interface Foo {
			  foo(s: string): void;
			  ['foo'](n: number): void;
			  foo(sn: string | number): void;
			  bar(): void;
			  baz(): void;
			}
                    "#,
                    r#"
			interface Foo {
			  foo(): void;
			  bar: {
			    baz(s: string): void;
			    baz(n: number): void;
			    baz(sn: string | number): void;
			  };
			}
                    "#,
                    r#"
			interface Foo {
			  new (s: string);
			  new (n: number);
			  new (sn: string | number);
			  foo(): void;
			}
                    "#,
                    r#"
			class Foo {
			  constructor(s: string);
			  constructor(n: number);
			  constructor(sn: string | number) {}
			  bar(): void {}
			  baz(): void {}
			}
                    "#,
                    r#"
			class Foo {
			  foo(s: string): void;
			  foo(n: number): void;
			  foo(sn: string | number): void {}
			  bar(): void {}
			  baz(): void {}
			}
                    "#,
                    r#"
			class Foo {
			  foo(s: string): void;
			  ['foo'](n: number): void;
			  foo(sn: string | number): void {}
			  bar(): void {}
			  baz(): void {}
			}
                    "#,
                    r#"
			class Foo {
			  name => string;
			  foo(s: string): void;
			  foo(n: number): void;
			  foo(sn: string | number): void {}
			  bar(): void {}
			  baz(): void {}
			}
                    "#,
                    r#"
			class Foo {
			  name => string;
			  static foo(s: string): void;
			  static foo(n: number): void;
			  static foo(sn: string | number): void {}
			  bar(): void {}
			  baz(): void {}
			}
                    "#,
                    r#"
			class Test {
			  static test() {}
			  untest() {}
			  test() {}
			}
                    "#,
                    // examples from https://github.com/nzakas/eslint-plugin-typescript/issues/138
                    "export default function <T>(foo: T) {}",
                    "export default function named<T>(foo: T) {}",
                    r#"
			interface Foo {
			  [Symbol.toStringTag](): void;
			  [Symbol.iterator](): void;
			}
                    "#,
                    // private members
                    r#"
			class Test {
			  #private(): void;
			  #private(arg: number): void {}

			  bar() {}

			  '#private'(): void;
			  '#private'(arg: number): void {}
			}
                    "#,
                    // block statement
                    r#"
			function wrap() {
			  function foo(s: string);
			  function foo(n: number);
			  function foo(sn: string | number) {}
			}
                    "#,
                    r#"
			if (true) {
			  function foo(s: string);
			  function foo(n: number);
			  function foo(sn: string | number) {}
			}
                    "#,
                  ],
                  invalid => [
                    {
                      code => r#"
function wrap() {
  function foo(s: string);
  function foo(n: number);
  type bar = number;
  function foo(sn: string | number) {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
if (true) {
  function foo(s: string);
  function foo(n: number);
  let a = 1;
  function foo(sn: string | number) {}
  foo(a);
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
export function foo(s: string);
export function foo(n: number);
export function bar(): void {}
export function baz(): void {}
export function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
export function foo(s: string);
export function foo(n: number);
export type bar = number;
export type baz = number | string;
export function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
function foo(s: string);
function foo(n: number);
function bar(): void {}
function baz(): void {}
function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
function foo(s: string);
function foo(n: number);
type bar = number;
type baz = number | string;
function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
function foo(s: string) {}
function foo(n: number) {}
const a = '';
const b = '';
function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
function foo(s: string) {}
function foo(n: number) {}
class Bar {}
function foo(sn: string | number) {}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 5,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
function foo(s: string) {}
function foo(n: number) {}
function foo(sn: string | number) {}
class Bar {
  foo(s: string);
  foo(n: number);
  name => string;
  foo(sn: string | number) {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 9,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
declare function foo(s: string);
declare function foo(n: number);
declare function bar(): void;
declare function baz(): void;
declare function foo(sn: string | number);
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
declare function foo(s: string);
declare function foo(n: number);
const a = '';
const b = '';
declare function foo(sn: string | number);
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 6,
                          column => 1,
                        },
                      ],
                    },
                    {
                      code => r#"
declare module 'Foo' {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function bar(): void;
  export function baz(): void;
  export function foo(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
declare module 'Foo' {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  function baz(s: string): void;
  export function bar(): void;
  function baz(n: number): void;
  function baz(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "baz" },
                          line => 8,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
declare namespace Foo {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function bar(): void;
  export function baz(): void;
  export function foo(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
declare namespace Foo {
  export function foo(s: string): void;
  export function foo(n: number): void;
  export function foo(sn: string | number): void;
  function baz(s: string): void;
  export function bar(): void;
  function baz(n: number): void;
  function baz(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "baz" },
                          line => 8,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
type Foo = {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
};
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
type Foo = {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
};
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
type Foo = {
  foo(s: string): void;
  name => string;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
};
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  (s: string): void;
  foo(n: number): void;
  (n: number): void;
  (sn: string | number): void;
  bar(): void;
  baz(): void;
  call(): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "call" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  foo(s: string): void;
  'foo'(n: number): void;
  bar(): void;
  baz(): void;
  foo(sn: string | number): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  foo(s: string): void;
  name => string;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
  baz(): void;
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  foo(): void;
  bar: {
    baz(s: string): void;
    baz(n: number): void;
    foo(): void;
    baz(sn: string | number): void;
  };
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "baz" },
                          line => 8,
                          column => 5,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  new (s: string);
  new (n: number);
  foo(): void;
  bar(): void;
  new (sn: string | number);
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "new" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
interface Foo {
  new (s: string);
  foo(): void;
  new (n: number);
  bar(): void;
  new (sn: string | number);
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "new" },
                          line => 5,
                          column => 3,
                        },
                        {
                          message_id => "adjacent_signature",
                          data => { name => "new" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  constructor(s: string);
  constructor(n: number);
  bar(): void {}
  baz(): void {}
  constructor(sn: string | number) {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "constructor" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  foo(s: string): void;
  ['foo'](n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 7,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  // prettier-ignore
  "foo"(s: string): void;
  foo(n: number): void;
  bar(): void {}
  baz(): void {}
  foo(sn: string | number): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 8,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  constructor(s: string);
  name => string;
  constructor(n: number);
  constructor(sn: string | number) {}
  bar(): void {}
  baz(): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "constructor" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  foo(s: string): void;
  name => string;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "foo" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    {
                      code => r#"
class Foo {
  static foo(s: string): void;
  name => string;
  static foo(n: number): void;
  static foo(sn: string | number): void {}
  bar(): void {}
  baz(): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "static foo" },
                          line => 5,
                          column => 3,
                        },
                      ],
                    },
                    // private members
                    {
                      code => r#"
class Test {
  #private(): void;
  '#private'(): void;
  #private(arg: number): void {}
  '#private'(arg: number): void {}
}
                      "#,
                      errors => [
                        {
                          message_id => "adjacent_signature",
                          data => { name => "#private" },
                          line => 5,
                          column => 3,
                        },
                        {
                          message_id => "adjacent_signature",
                          data => { name => "\"#private\"" },
                          line => 6,
                          column => 3,
                        },
                      ],
                    },
                  ],
            },
        )
    }
}
