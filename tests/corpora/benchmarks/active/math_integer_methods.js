let total = 0;
let index = 0;

while (index < 12000) {
  total = total + Math.clz32(index);
  total = total + Math.imul(index, 31);
  total = total + Math.fround(index + 0.1);
  index = index + 1;
}

total;
