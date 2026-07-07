var value = 0;

for (let index = 0; index < 65536; index = index + 1) {
    value = true ? value + 1 : value + 100;
    value = false ? value + 100 : value + 1;
    value = value + ((value & 1) & true);
}

value;
