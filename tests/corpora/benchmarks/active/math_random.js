let total = 0;
let index = 0;

while (index < 2000) {
  total = total + Math.random();
  index = index + 1;
}

print(total >= 0);
