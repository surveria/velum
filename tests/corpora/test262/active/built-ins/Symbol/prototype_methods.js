let symbol = Symbol("slot");
let empty = Symbol();
let boxed = Object(symbol);
let descriptor = Object.getOwnPropertyDescriptor(Symbol.prototype, "description");

if (
  symbol.description !== "slot" ||
  empty.description !== undefined ||
  symbol.toString() !== "Symbol(slot)" ||
  boxed.toString() !== "Symbol(slot)" ||
  symbol.valueOf() !== symbol ||
  boxed.valueOf() !== symbol ||
  Object(symbol).valueOf() !== symbol ||
  typeof descriptor.get !== "function" ||
  descriptor.set !== undefined ||
  descriptor.enumerable !== false ||
  descriptor.configurable !== true
) {
  throw new Test262Error("Symbol prototype method behavior was unexpected");
}

let rejected = false;
try {
  Symbol.prototype.valueOf.call({});
} catch (error) {
  rejected = error instanceof TypeError;
}

if (!rejected) {
  throw new Test262Error("Symbol.prototype.valueOf accepted a non-symbol receiver");
}

42
