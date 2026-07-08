class Point {
  x = 1;
  y = this.x + 1;
  bare;
}
var p = new Point();
if (p.x !== 1 || p.y !== 2 || p.bare !== undefined) {
  throw new Test262Error("instance field mismatch");
}

class Registry {
  static count = 40 + 2;
  static kind = typeof this;
}
if (Registry.count !== 42 || Registry.kind !== "function") {
  throw new Test262Error("static field mismatch");
}

var suffix = "puted";
class Keys {
  ["com" + suffix] = "c";
  "quoted" = "q";
  42 = "n";
}
var keys = new Keys();
if (keys.computed !== "c" || keys.quoted !== "q" || keys[42] !== "n") {
  throw new Test262Error("field key form mismatch");
}

class Base {
  v = "base";
}
class Derived extends Base {
  w = this.v + "+derived";
}
if (new Derived().w !== "base+derived") {
  throw new Test262Error("derived field ordering mismatch");
}

class Mixed {
  f = 1;
  constructor() {
    this.g = this.f + 1;
  }
}
var mixed = new Mixed();
if (mixed.f !== 1 || mixed.g !== 2) {
  throw new Test262Error("field constructor interplay mismatch");
}

var counter = 0;
function next() {
  counter = counter + 1;
  return counter;
}
class Counted {
  id = next();
}
if (new Counted().id !== 1 || new Counted().id !== 2 || counter !== 2) {
  throw new Test262Error("per-instance initializer mismatch");
}

var enumerated = "";
var shape = new Point();
for (var key in shape) {
  enumerated = enumerated + key;
}
if (enumerated !== "xybare") {
  throw new Test262Error("field enumerability mismatch");
}

42;
