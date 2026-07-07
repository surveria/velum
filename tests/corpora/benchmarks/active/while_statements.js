let values = [1, 2, 3, 4];
let index = 0;
let total = 0;

while (index < 98304) {
  var slot = index & 3;
  total = total + values[slot];
  index = index + 1;
}

total;
