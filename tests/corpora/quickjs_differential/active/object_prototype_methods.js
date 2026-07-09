var op = Object.prototype;

print("array", op.toString.call([]));
print("null", op.toString.call(null));
print("undefined", op.toString.call(undefined));
print("number", op.toString.call(42));
print("string", op.toString.call("s"));
print("boolean", op.toString.call(true));
print("function", op.toString.call(function () {}));
print("date", op.toString.call(new Date()));
print("regexp", op.toString.call(/x/));
print("error", op.toString.call(new Error("e")));
print("boxed-number", op.toString.call(new Number(1)));
print("boxed-string", op.toString.call(new String("x")));
print("boxed-boolean", op.toString.call(new Boolean(true)));
print("plain", op.toString.call(Object.create(op)));

var tagged = Object.create(op);
tagged[Symbol.toStringTag] = "Custom";
print("tagged", op.toString.call(tagged));

var nonStringTag = Object.create(op);
nonStringTag[Symbol.toStringTag] = 123;
print("non-string-tag", op.toString.call(nonStringTag));

print("valueOf-object", op.valueOf.call(op) === op);
print("valueOf-number", op.valueOf.call(5) instanceof Number);
print("valueOf-string", op.valueOf.call("z") instanceof String);

var proto = Object.create(op);
var child = Object.create(proto);
var grandchild = Object.create(child);
print("isProto", proto.isPrototypeOf(child), proto.isPrototypeOf(grandchild));
print("isProto-neg", child.isPrototypeOf(proto), proto.isPrototypeOf(proto));
print("isProto-nonobj", op.isPrototypeOf.call(op, 42), op.isPrototypeOf.call(op, null));

var fe = Object.fromEntries([["a", 1], ["b", 2], ["dup", 3], ["dup", 4]]);
print("fromEntries", fe.a, fe.b, fe.dup, Object.keys(fe).join(","));
var feMap = Object.fromEntries(new Map([["x", 10], ["y", 20]]));
print("fromEntries-map", feMap.x, feMap.y);

print("toLocale", op.toLocaleString.call(Object.create(op)));

var errors = "";
try { op.valueOf.call(undefined); } catch (e) { errors += e.constructor.name + ";"; }
try { op.valueOf.call(null); } catch (e) { errors += e.constructor.name + ";"; }
try { Object.fromEntries(undefined); } catch (e) { errors += e.constructor.name + ";"; }
print("errors", errors);

print("meta", op.toString.length, op.valueOf.length, op.isPrototypeOf.length, Object.fromEntries.length);
print("names", op.toString.name, op.valueOf.name, op.isPrototypeOf.name, op.toLocaleString.name);
