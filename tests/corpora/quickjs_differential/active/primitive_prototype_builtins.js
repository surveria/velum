let numberBox = Object(255);
print(
    numberBox.valueOf(),
    numberBox.toString(16),
    Number.prototype.toString.call(15, 2),
    Number.isInteger(42),
    Number.isInteger(42.5),
    Number.isSafeInteger(9007199254740991),
    Number.isSafeInteger(9007199254740992)
);

let falseBox = Object(false);
let trueBox = new Boolean(true);
print(
    falseBox.valueOf(),
    falseBox.toString(),
    trueBox.valueOf(),
    trueBox.toString(),
    Boolean.prototype.valueOf.call(true)
);

let symbol = Symbol("slot");
let symbolBox = Object(symbol);
let descriptor = Object.getOwnPropertyDescriptor(Symbol.prototype, "description");
print(
    symbol.description,
    Symbol().description,
    symbol.toString(),
    symbolBox.toString(),
    symbol.valueOf() === symbol,
    symbolBox.valueOf() === symbol,
    typeof descriptor.get,
    descriptor.enumerable,
    descriptor.configurable
);
