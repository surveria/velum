function probe(a, b) {
  return arguments.length + ":" + arguments[0] + ":" + arguments[2];
}
print(probe(10, 20, 30));

function count() {
  return arguments.length;
}
print(count(), count(1), count(1, 2, 3));

function unmapped(a) {
  "use strict";
  arguments[0] = 99;
  return a + ":" + arguments[0];
}
print(unmapped(1));

function byParam(arguments) {
  return arguments;
}
print(byParam("param"));

function total() {
  var sum = 0;
  for (var i = 0; i < arguments.length; i = i + 1) {
    sum = sum + arguments[i];
  }
  return sum;
}
print(total(1, 2, 3, 4));

function spreadThrough() {
  return Math.max(...arguments);
}
print(spreadThrough(3, 9, 4));

var holder = {
  m: function () {
    return arguments.length;
  }
};
print(holder.m(1, 2));

function forwarded() {
  return [...arguments].join("|");
}
print(forwarded("x", "y"));
