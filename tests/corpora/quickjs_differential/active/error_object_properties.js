try {
  missing = missing;
  print("unreachable");
} catch (error) {
  print(error.name);
  print(error.message);
}
