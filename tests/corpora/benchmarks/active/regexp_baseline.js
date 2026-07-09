let total = 0;
let index = 0;
let words = /\w+/g;
let digits = /\d+/;
let alpha = /a+/y;

while (index < 8192) {
  words.lastIndex = 0;
  if (words.test("abc 123")) {
    total = total + words.lastIndex;
  }
  if (digits.test("id=12345")) {
    total = total + index;
  }
  alpha.lastIndex = 1;
  let match = alpha.exec("baaaa");
  if (match !== null) {
    total = total + match[0].length + match.index;
  }
  let cloned = new RegExp(words, "mi");
  if (cloned.source === "\\w+" && cloned.flags === "im") {
    total = total + 1;
  }
  if (words.global && !words.ignoreCase && !words.sticky) {
    total = total + 1;
  }
  if (words.toString() === "/\\w+/g") {
    total = total + 1;
  }
  index = index + 1;
}

total
