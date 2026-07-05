let parsed = JSON.parse('{"camera":"front","active":true,"count":2,"items":[1,null,"x"],"nested":{"ok":false}}');
let jsonKeys = "";
for (let key in JSON) {
    jsonKeys = jsonKeys + key + ";";
}

let primitiveOk =
    JSON.parse("true") === true &&
    JSON.parse("false") === false &&
    JSON.parse("null") === null &&
    JSON.parse("42") === 42 &&
    JSON.parse('"lens"') === "lens";

let generated = JSON.stringify({
    z: 1,
    a: "front",
    skip: undefined,
    fn: function() {
        return 1;
    },
    nested: { ok: true },
    arr: [true, undefined, null, NaN, Infinity]
});
let arrayText = JSON.stringify(parsed.items);
let negativeZero = JSON.stringify(-0);

print(
    typeof JSON,
    JSON.__proto__ === Object.prototype,
    typeof JSON.parse,
    JSON.parse.name,
    JSON.parse.length,
    typeof JSON.stringify,
    JSON.stringify.name,
    JSON.stringify.length
);
print(parsed.camera, parsed.active, parsed.count, parsed.items.length, parsed.items[2], parsed.nested.ok);
print(
    JSON.stringify(null),
    JSON.stringify(true),
    JSON.stringify(false),
    JSON.stringify("front"),
    JSON.stringify(42),
    JSON.stringify(NaN),
    JSON.stringify(Infinity),
    JSON.stringify(undefined),
    negativeZero
);
print(arrayText);
print(generated);
print("keys:" + jsonKeys);

primitiveOk &&
    typeof JSON === "object" &&
    JSON.__proto__ === Object.prototype &&
    typeof JSON.parse === "function" &&
    JSON.parse.name === "parse" &&
    JSON.parse.length === 2 &&
    typeof JSON.stringify === "function" &&
    JSON.stringify.name === "stringify" &&
    JSON.stringify.length === 3 &&
    parsed.camera === "front" &&
    parsed.active === true &&
    parsed.count === 2 &&
    parsed.items.length === 3 &&
    parsed.items[0] === 1 &&
    parsed.items[1] === null &&
    parsed.items[2] === "x" &&
    parsed.nested.ok === false &&
    arrayText === '[1,null,"x"]' &&
    generated === '{"z":1,"a":"front","nested":{"ok":true},"arr":[true,null,null,null,null]}' &&
    JSON.stringify(undefined) === undefined &&
    negativeZero === "0" &&
    jsonKeys === "" ? 42 : 0
