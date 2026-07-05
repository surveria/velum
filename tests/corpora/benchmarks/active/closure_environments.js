let makeCounter = function(start) {
  var value = start;
  return function(delta) {
    value = value + delta;
    return value;
  };
};

let counter = makeCounter(0);
counter(1);
counter(2);
counter(3);
counter(4);
counter(5);
