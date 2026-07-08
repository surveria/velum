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
if (p.sum() !== 42 || !(p instanceof Point)) {
  throw new Test262Error("class construction mismatch");
}
if (Point.prototype.constructor !== Point || typeof Point !== "function") {
  throw new Test262Error("class prototype wiring mismatch");
}

class Empty {}
if (!(new Empty() instanceof Empty) || Empty.name !== "Empty") {
  throw new Test262Error("default constructor mismatch");
}

class Registry {
  static create(tag) {
    return new Registry(tag);
  }
  constructor(tag) {
    this.tag = tag;
  }
}
if (Registry.create("r").tag !== "r" || Registry.prototype.create !== undefined) {
  throw new Test262Error("static method mismatch");
}

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
if (boxed.stored !== 21 || boxed.value !== 42) {
  throw new Test262Error("class accessor mismatch");
}

var suffix = "puted";
class Keys {
  ["com" + suffix]() {
    return "c";
  }
}
if (new Keys().computed() !== "c") {
  throw new Test262Error("computed member key mismatch");
}

var caught = "";
try {
  Empty();
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("class call without new must throw TypeError");
}

var tdz = "";
try {
  Later;
  class Later {}
} catch (error) {
  if (error instanceof ReferenceError) {
    tdz = "ReferenceError";
  }
}
if (tdz !== "ReferenceError") {
  throw new Test262Error("class declaration must have TDZ semantics");
}

var Named = class Inner {
  m() {
    return "named";
  }
};
if (new Named().m() !== "named" || Named.name !== "Inner") {
  throw new Test262Error("class expression mismatch");
}

42;
