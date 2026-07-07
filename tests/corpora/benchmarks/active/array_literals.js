let total = 0;

for (let index = 0; index < 16384; index = index + 1) {
  let values = [index, index + 1, index + 2, index + 3];
  values[0] = values[0] + values[1];
  values[1] = values[0] + values[2];
  values[2] = values[1] + values[3];
  values[3] = values[2] + values[0];
  values[4] = values[3] + values[1];
  values[5] = values[4] + values[2];
  values[6] = values[5] + values[3];
  values[7] = values[6] + values[4];
  values[8] = values[7] + values.length;
  values[9] = values[8] + values[0];
  total = total + values[9];
}

total;
