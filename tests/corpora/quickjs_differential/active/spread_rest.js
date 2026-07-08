function tail(first, ...rest) {
  return first + "|" + rest.join(",");
}
print(tail(1, 2, 3));
print(tail("only"));

function pairRest(...[a, b]) {
  return "" + a + b;
}
print(pairRest(1, 2));

function lengthProbe(a, b, ...r) {}
print(lengthProbe.length);

function join4(a, b, c, d) {
  return "" + a + b + c + d;
}
print(join4(...[1, 2], 3, ...[4]));
print(Math.max(...[3, 9, 4]));

var receiver = {
  base: 10,
  add: function (a, b) { return this.base + a + b; }
};
print(receiver.add(...[1, 2]));
var key = "add";
print(receiver[key](...[20, 2]));

function PairCtor(a, b) {
  this.sum = a + b;
}
print(new PairCtor(...[40, 2]).sum);

print([0, ...[1, 2], ..."ab"].join("|"));

var custom = {};
custom[Symbol.iterator] = function () {
  var index = 0;
  return {
    next: function () {
      index = index + 1;
      return { done: index > 2, value: index * 10 };
    }
  };
};
print([...custom].join("+"));

var baseObject = { x: 1, y: 2 };
var merged = { w: 0, ...baseObject, y: 9, ...null, ...undefined };
print(merged.w, merged.x, merged.y, baseObject.y);

try {
  var broken = [...5];
} catch (error) {
  print(error instanceof TypeError);
}

var log = "";
function note(tag, value) {
  log = log + tag;
  return value;
}
function three(a, b, c) {
  return "" + a + b + c;
}
print(three(note("A", 1), ...note("B", [2]), note("C", 3)), log);
