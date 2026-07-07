let values = [1, 2, 3, 4];
let index = 0;
let total = 0;

while (index < 98304) {
  index = index + 1;
  if ((index & 3) === 0) {
    continue;
  }
  if (index > 98296) {
    break;
  }
  total = total + values[index & 3];
}

total;
