var {a, b: renamed, missing = "fallback"} = {a: 1, b: 2};
print(a, renamed, missing);

var [first, , third = 30, absent = 40] = [1, 2, undefined];
print(first, third, absent);

var {outer: {inner}, list: [head, {deep}]} =
    {outer: {inner: "i"}, list: ["h", {deep: "d"}]};
print(inner, head, deep);

var {a: consumed, ...others} = {a: 1, b: 2, c: 3};
print(consumed, others.b, others.c, others.a === undefined);

var [restHead, ...restTail] = [10, 20, 30];
print(restHead, restTail.length, restTail[0], restTail[1]);

var key = "dyn";
var {[key + "amic"]: computedValue} = {dynamic: "found"};
print(computedValue);

var [sa, sb] = "xy";
print(sa, sb);

function join({left, right = "R"}, [one, two]) {
  return left + right + one + two;
}
print(join({left: "L"}, ["1", "2"]));

var pick = function ({v}) { return v; };
print(pick({v: 42}));

var collected = "";
for (var {id} of [{id: 1}, {id: 2}]) {
  collected = collected + ":" + id;
}
for (var [x, y] of [[1, 2], [3, 4]]) {
  collected = collected + ";" + (x + y);
}
print(collected);

try {
  var {broken} = null;
} catch (error) {
  print(error instanceof TypeError);
}

try {
  var [alsoBroken] = 5;
} catch (error) {
  print(error instanceof TypeError);
}

var closed = false;
var closable = {};
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
var [onlyFirst] = closable;
print(onlyFirst, closed);

var defaults = "";
function order() {
  defaults = defaults + "!";
  return "computed";
}
var {present = order()} = {present: "kept"};
print(present, defaults === "");
