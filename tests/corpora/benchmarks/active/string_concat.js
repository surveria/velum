let total = 0;

for (let index = 0; index < 32768; index = index + 1) {
  let name = "camera";
  let result = name + "-stream-" + index;
  total = total + result.length;
}

total;
