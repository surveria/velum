let total = 0;
let index = 0;

while (index < 4096) {
  total = total + Math.abs(-index);
  total = total + Math.ceil(index + 0.25);
  total = total + Math.floor(index + 0.75);
  total = total + Math.trunc(index + 0.5);
  total = total + Math.round(index + 0.5);
  total = total + Math.sqrt(81);
  total = total + Math.pow(2, 5);
  total = total + Math.max(index, 7, 3);
  total = total + Math.min(index, -2, 3);
  index = index + 1;
}

total
