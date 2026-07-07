let value = 1;
let add = function(left, right) {
  var local = left + right;
  return local;
};
let bump = function(delta) {
  value = value + delta;
  return value;
};
let total = 0;

for (let index = 0; index < 32768; index = index + 1) {
  total = total + add(index & 15, 20);
  total = total + add(20, index & 7);
  total = total + bump(1);
}

total + value;
