let add = Function("left", "right", "return left + right;");
let fromList = new Function("left, right", "return left + right;");
let withoutArgs = Function();
let make = function() {
  let hidden = 42;
  return Function("return typeof hidden;");
};

if (
  typeof Function !== "function" ||
  Function.name !== "Function" ||
  Function.length !== 1 ||
  Function.prototype.constructor !== Function ||
  add.name !== "anonymous" ||
  add.length !== 2 ||
  add(20, 22) !== 42 ||
  fromList(21, 21) !== 42 ||
  withoutArgs() !== undefined ||
  make()() !== "undefined"
) {
  throw new Test262Error("Function constructor behavior was unexpected");
}

42
