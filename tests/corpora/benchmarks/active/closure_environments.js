let makeCounter = function(start) {
  var value = start;
  return function(delta) {
    value = value + delta;
    return value;
  };
};

let counter = makeCounter(0);
let total = 0;

for (let index = 0; index < 32768; index = index + 1) {
  total = total + counter(1);
  total = total + counter(2);
  total = total + counter(3);
}

total;
