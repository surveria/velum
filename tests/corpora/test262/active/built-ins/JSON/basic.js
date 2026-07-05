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

if (
  !primitiveOk ||
  typeof JSON !== "object" ||
  JSON.__proto__ !== Object.prototype ||
  typeof JSON.parse !== "function" ||
  JSON.parse.name !== "parse" ||
  JSON.parse.length !== 2 ||
  typeof JSON.stringify !== "function" ||
  JSON.stringify.name !== "stringify" ||
  JSON.stringify.length !== 3 ||
  parsed.camera !== "front" ||
  parsed.active !== true ||
  parsed.count !== 2 ||
  parsed.items.length !== 3 ||
  parsed.items[0] !== 1 ||
  parsed.items[1] !== null ||
  parsed.items[2] !== "x" ||
  parsed.nested.ok !== false ||
  arrayText !== '[1,null,"x"]' ||
  generated !== '{"z":1,"a":"front","nested":{"ok":true},"arr":[true,null,null,null,null]}' ||
  JSON.stringify(undefined) !== undefined ||
  negativeZero !== "0" ||
  jsonKeys !== ""
) {
  throw new Test262Error("JSON basic behavior was unexpected");
}

42
