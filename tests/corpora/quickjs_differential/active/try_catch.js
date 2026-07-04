print("before");
try {
  throw "caught";
  print("unreachable");
} catch (error) {
  print(error);
}
print("after");
