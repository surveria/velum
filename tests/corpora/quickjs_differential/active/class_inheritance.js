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
print(d.sum(), d instanceof Derived, d instanceof Base);

class Forward extends Base {}
print(new Forward(7).x);

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
print(new C().describe());

class GetBase {
  get magic() {
    return 21;
  }
}
class GetDerived extends GetBase {
  get magic() {
    return super.magic * 2;
  }
}
print(new GetDerived().magic);

class StaticBase {
  static tag() {
    return "base";
  }
}
class StaticDerived extends StaticBase {}
print(StaticDerived.tag());

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
print(new Modern().read());

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
print(new SpreadDerived([20, 21, 1]).total);

try {
  class Bad extends 5 {}
} catch (error) {
  print(error instanceof TypeError);
}

var inherited = new Forward(1);
print(Object.getPrototypeOf(Object.getPrototypeOf(inherited)) === Base.prototype);

class ArrowHome extends Base {
  constructor() {
    super(9);
  }
  read() {
    const grab = () => super.getX;
    return typeof grab();
  }
}
print(new ArrowHome().read());

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
print(order);
