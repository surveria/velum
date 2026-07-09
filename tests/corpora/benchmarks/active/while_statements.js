let rounds = 0;
let grandTotal = 0;

while (rounds < 1000) {
  let values = [1, 2, 3, 4];
  let index = 0;
  let total = 0;

  while (index < 99500) {
    var slot = index & 3;
    total = total + values[slot];
    index = index + 1;
  }

  grandTotal = grandTotal + total;
  rounds = rounds + 1;
}

grandTotal;
