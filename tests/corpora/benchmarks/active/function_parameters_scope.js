let value = 1;
let add = function(left, right) {
  var local = left + right;
  return local;
};
let bump = function(delta) {
  value = value + delta;
  return value;
};

add(10, 20);
add(20, 22);
bump(41);
value;
