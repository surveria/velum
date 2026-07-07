var value = 0;

for (let index = 0; index < 131072; index = index + 1) {
  try {
    throw "caught";
    value = 100;
  } catch (error) {
    if (error === "caught") {
      value = value + 1;
    }
  }
}

value;
