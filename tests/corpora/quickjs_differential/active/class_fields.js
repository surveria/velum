class Point {
  x = 1;
  y = this.x + 1;
  bare;
}
var p = new Point();
print(p.x, p.y, p.bare === undefined);

class Registry {
  static count = 40 + 2;
  static kind = typeof this;
}
print(Registry.count, Registry.kind);

var suffix = "puted";
class Keys {
  ["com" + suffix] = "c";
  "quoted" = "q";
  42 = "n";
}
var keys = new Keys();
print(keys.computed, keys.quoted, keys[42]);

class Base {
  v = "base";
}
class Derived extends Base {
  w = this.v + "+derived";
}
class Third extends Derived {
  z = this.w + "+third";
}
print(new Third().z);

class Mixed {
  f = 1;
  constructor() {
    this.g = this.f + 1;
  }
  sum() {
    return this.f + this.g;
  }
}
print(new Mixed().sum());

var counter = 0;
function next() {
  counter = counter + 1;
  return counter;
}
class Counted {
  id = next();
}
print(new Counted().id, new Counted().id, counter);

var seen = "";
for (var key in p) {
  seen = seen + "|" + key;
}
print(seen, Object.keys(p).length);

class Named {
  static = 1;
  get = 2;
  set = 3;
}
var named = new Named();
print(named.static, named.get, named.set);

class WithMethods {
  n = 5;
  double() {
    return this.n * 2;
  }
}
print(new WithMethods().double());
