const {a, b: renamed, missing = "fallback"} = {a: 1, b: 2};
if (a !== 1 || renamed !== 2 || missing !== "fallback") {
  throw new Test262Error("object pattern declaration mismatch");
}

let [first, , third = 30, absent = 40] = [1, 2, undefined];
if (first !== 1 || third !== 30 || absent !== 40) {
  throw new Test262Error("array pattern elision and default mismatch");
}

const {outer: {inner}, list: [head, {deep}]} =
    {outer: {inner: "i"}, list: ["h", {deep: "d"}]};
if (inner !== "i" || head !== "h" || deep !== "d") {
  throw new Test262Error("nested pattern mismatch");
}

const {a: consumed, ...others} = {a: 1, b: 2, c: 3};
if (consumed !== 1 || others.b !== 2 || others.c !== 3 || others.a !== undefined) {
  throw new Test262Error("object rest mismatch");
}

const [restHead, ...restTail] = [10, 20, 30];
if (restHead !== 10 || restTail.length !== 2 || restTail[0] !== 20 || restTail[1] !== 30) {
  throw new Test262Error("array rest mismatch");
}

const key = "dyn";
const {[key + "amic"]: computedValue} = {dynamic: "found"};
if (computedValue !== "found") {
  throw new Test262Error("computed pattern key mismatch");
}

const [sa, sb] = "xy";
if (sa !== "x" || sb !== "y") {
  throw new Test262Error("string destructuring mismatch");
}

function join({left, right = "R"}, [one, two]) {
  return left + right + one + two;
}
if (join({left: "L"}, ["1", "2"]) !== "LR12") {
  throw new Test262Error("parameter pattern mismatch");
}

const pick = ({v}) => v;
if (pick({v: 42}) !== 42) {
  throw new Test262Error("arrow parameter pattern mismatch");
}

let collected = "";
for (const {id} of [{id: 1}, {id: 2}]) {
  collected = collected + ":" + id;
}
for (const [x, y] of [[1, 2], [3, 4]]) {
  collected = collected + ";" + (x + y);
}
if (collected !== ":1:2;3;7") {
  throw new Test262Error("for-of pattern head mismatch");
}

let caught = "";
try {
  const {broken} = null;
} catch (error) {
  if (error instanceof TypeError) {
    caught = "TypeError";
  }
}
if (caught !== "TypeError") {
  throw new Test262Error("null object pattern source must throw TypeError");
}

let closed = false;
let closable = {};
closable[Symbol.iterator] = function () {
  return {
    next: function () {
      return { done: false, value: 1 };
    },
    return: function () {
      closed = true;
      return {};
    }
  };
};
const [onlyFirst] = closable;
if (onlyFirst !== 1 || !closed) {
  throw new Test262Error("array pattern must close unexhausted iterators");
}

42;
