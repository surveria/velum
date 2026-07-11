class Point {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }
  sum() {
    return this.x + this.y;
  }
}
var p = new Point(40, 2);
print(p.sum(), p instanceof Point, Point.prototype.constructor === Point, typeof Point);

class Empty {}
print(new Empty() instanceof Empty, Empty.name, Empty.length);

class Registry {
  static create(tag) {
    return new Registry(tag);
  }
  constructor(tag) {
    this.tag = tag;
  }
}
print(Registry.create("r").tag, Registry.prototype.create === undefined);

class Boxed {
  get value() {
    return this.stored * 2;
  }
  set value(next) {
    this.stored = next / 2;
  }
}
var boxed = new Boxed();
boxed.value = 42;
print(boxed.stored, boxed.value);

var staticKey = "value";
class StaticBase {
  static get [staticKey]() {
    return this.stored;
  }
  static set [staticKey](next) {
    this.stored = next;
  }
}
class StaticChild extends StaticBase {}
StaticChild.value = 42;
var staticDescriptor = Object.getOwnPropertyDescriptor(StaticBase, staticKey);
print(
  StaticChild.value,
  StaticBase.value,
  Object.getPrototypeOf(StaticChild) === StaticBase,
  typeof staticDescriptor.get,
  typeof staticDescriptor.set,
  staticDescriptor.enumerable,
  staticDescriptor.configurable
);

var suffix = "puted";
class Keys {
  ["com" + suffix]() {
    return "c";
  }
  "quoted"() {
    return "q";
  }
  42() {
    return "n";
  }
}
var keys = new Keys();
print(keys.computed(), keys.quoted(), keys[42]());

class Quiet {
  visible() {}
}
var quiet = new Quiet();
quiet.own = 1;
var seen = "";
for (var key in quiet) {
  seen = seen + key;
}
print(seen);

try {
  Empty();
} catch (error) {
  print(error instanceof TypeError);
}

var Named = class Inner {
  m() {
    return "named";
  }
};
print(new Named().m(), Named.name);

class Wide {
  constructor({a, b = 2}, ...rest) {
    this.sum = a + b + rest.length;
  }
}
print(new Wide({a: 1}, 9, 9, 9).sum);

class Override {
  constructor() {
    return {custom: "yes"};
  }
}
print(new Override().custom);

class MethodName {
  myMethod() {}
}
print(MethodName.prototype.myMethod.name, MethodName.name);
