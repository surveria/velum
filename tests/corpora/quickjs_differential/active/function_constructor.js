let add = Function("left", "right", "return left + right;");
let fromList = new Function("left, right", "return left + right;");
let withoutArgs = Function();
let make = function() {
  let hidden = 42;
  return Function("return typeof hidden;");
};

print(typeof Function, Function.name, Function.length);
print(Function.prototype.constructor === Function);
print(add.name, add.length, add(20, 22));
print(fromList(21, 21));
print(withoutArgs());
print(make()());
