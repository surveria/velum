let values = [1, 2, 3, 4];
let index = 0;
let total = 0;

while (index < 12288) {
  index = index + 1;
  if ((index & 3) === 0) {
    continue;
  }
  if (index > 12280) {
    break;
  }
  total = total + values[index & 3];
}

total;
