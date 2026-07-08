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
let reviverOrder = "";
let revived = JSON.parse(
    '{"a":1,"nested":{"b":2},"arr":[3,4],"drop":5}',
    function(key, value) {
        reviverOrder = reviverOrder + key + ";";
        if (key === "drop" || key === "1") {
            return undefined;
        }
        if (key === "b") {
            return value + 40;
        }
        return value;
    }
);
let replacerThisOk = false;
let replacerText = JSON.stringify(
    { a: 1, b: 2, c: 3 },
    function(key, value) {
        if (key === "a" && this.a === 1) {
            replacerThisOk = true;
        }
        if (key === "b") {
            return undefined;
        }
        return value;
    }
);
let listText = JSON.stringify({ a: 1, b: 2, c: 3 }, ["c", "a", "c"]);
let prettyText = JSON.stringify({ a: 1, nested: { b: 2 } }, null, 2);
let arrayReplacerText = JSON.stringify([1, 2, 3], function(key, value) {
    if (key === "1") {
        return undefined;
    }
    return value;
});
let toJsonKey = "";
let toJsonText = JSON.stringify({
    keep: {
        toJSON: function(key) {
            toJsonKey = key;
            return { answer: 42, skip: undefined };
        }
    }
});
let callbackOk =
    revived.a === 1 &&
    revived.nested.b === 42 &&
    revived.arr.length === 2 &&
    revived.arr[0] === 3 &&
    revived.arr[1] === undefined &&
    revived.drop === undefined &&
    reviverOrder === "a;b;nested;0;1;arr;drop;;" &&
    replacerThisOk &&
    replacerText === '{"a":1,"c":3}' &&
    listText === '{"c":3,"a":1}' &&
    prettyText === '{\n  "a": 1,\n  "nested": {\n    "b": 2\n  }\n}' &&
    arrayReplacerText === '[1,null,3]' &&
    toJsonKey === "keep" &&
    toJsonText === '{"keep":{"answer":42}}';

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
print("callbacks:" + callbackOk);
