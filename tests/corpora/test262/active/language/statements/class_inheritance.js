class Base {
  constructor(x) {
    this.x = x;
  }
  getX() {
    return this.x;
  }
}
class Derived extends Base {
  constructor(x, y) {
    super(x);
    this.y = y;
  }
  sum() {
    return this.getX() + this.y;
  }
}
var d = new Derived(40, 2);
if (d.sum() !== 42 || !(d instanceof Derived) || !(d instanceof Base)) {
  throw new Test262Error("derived construction mismatch");
}

class Forward extends Base {}
if (new Forward(7).x !== 7) {
  throw new Test262Error("default derived constructor mismatch");
}

class A {
  describe() {
    return "A";
  }
}
class B extends A {
  describe() {
    return super.describe() + "B";
  }
}
class C extends B {
  describe() {
    return super.describe() + "C";
  }
}
if (new C().describe() !== "ABC") {
  throw new Test262Error("super method chain mismatch");
}

class StaticBase {
  static tag() {
    return "base";
  }
}
class StaticDerived extends StaticBase {}
if (StaticDerived.tag() !== "base") {
  throw new Test262Error("static inheritance mismatch");
}

function Legacy(v) {
  this.v = v;
}
Legacy.prototype.read = function () {
  return this.v;
};
class Modern extends Legacy {
  constructor() {
    super(42);
  }
}
if (new Modern().read() !== 42) {
  throw new Test262Error("constructor-function heritage mismatch");
}

class SpreadBase {
  constructor(a, b, c) {
    this.total = a + b + c;
  }
}
class SpreadDerived extends SpreadBase {
  constructor(values) {
    super(...values);
  }
}
if (new SpreadDerived([20, 21, 1]).total !== 42) {
  throw new Test262Error("super spread call mismatch");
}

var caught = "";
try {
  class Bad extends 5 {}
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("non-constructor heritage must throw TypeError");
}

var order = "";
class L1 {
  constructor() {
    order = order + "1";
  }
}
class L2 extends L1 {
  constructor() {
    super();
    order = order + "2";
  }
}
class L3 extends L2 {
  constructor() {
    super();
    order = order + "3";
  }
}
new L3();
if (order !== "123") {
  throw new Test262Error("construction order mismatch");
}

42;
