let total = 0;

for (let index = 0; index < 32768; index = index + 1) {
  if (Boolean(index & 1)) {
    total = total + 1;
  }
  if (!Boolean(index & 0)) {
    total = total + 1;
  }
}

total;
