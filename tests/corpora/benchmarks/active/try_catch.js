var value = 0;

for (let round = 0; round < 4096; round = round + 1) {
  for (let index = 0; index < 32; index = index + 1) {
    try {
      throw "caught";
      value = 100;
    } catch (error) {
      if (error === "caught") {
        value = value + 1;
      }
    }
  }
}

value;
