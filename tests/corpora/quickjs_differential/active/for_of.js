var collected = [];
for (var item of [1, 2, 3]) {
  collected.push(item);
}
print(collected.join(","));

var sum = 0;
for (var value of [10, 20, 30]) {
  sum = sum + value;
}
print(sum);

var chars = [];
for (var ch of "ab") {
  chars.push(ch);
}
print(chars.join("|"));

var assigned = "";
for (assigned of ["p", "q"]) {}
print(assigned);

var braked = [];
for (var current of [1, 2, 3]) {
  if (current === 2) {
    break;
  }
  braked.push(current);
}
print(braked.join(","));

var skipped = [];
for (var entry of [1, 2, 3]) {
  if (entry === 2) {
    continue;
  }
  skipped.push(entry);
}
print(skipped.join(","));

try {
  for (var broken of 5) {}
} catch (error) {
  print(error instanceof TypeError);
}

try {
  for (var missing of null) {}
} catch (error) {
  print(error instanceof TypeError);
}

var custom = [];
var iterable = {};
iterable[Symbol.iterator] = function () {
  var index = 0;
  return {
    next: function () {
      index = index + 1;
      return { done: index > 3, value: index * 10 };
    }
  };
};
for (var produced of iterable) {
  custom.push(produced);
}
print(custom.join(","));

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
for (var ignored of closable) {
  break;
}
print(closed);

var live = [1, 2, 3];
for (var grown of live) {
  if (grown === 1) {
    live.push(99);
  }
  print(grown);
}
