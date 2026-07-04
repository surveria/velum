var value = 0;
value = true ? value + 1 : value + 100;
value = false ? value + 100 : value + 1;
value = value + ((value === 2) & true);
undefined;
