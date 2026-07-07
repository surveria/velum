let total = 0;

for (let index = 0; index < 65536; index = index + 1) {
    total = total + 1 + 2 + 3 + 4 + 5 + 6;
    total = total + 7 + 8 + 9 + 10 + 11 + 12;
    total = total + (index & 15);
}

total;
