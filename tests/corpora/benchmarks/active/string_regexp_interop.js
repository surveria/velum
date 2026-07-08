let total = 0;
let index = 0;
let text = "id=123;id=456;name=abc";
let digits = /\d+/g;
let words = /\w/g;
let separator = /;/g;

while (index < 192) {
  digits.lastIndex = 0;
  words.lastIndex = 0;
  separator.lastIndex = 0;

  let matches = text.match(digits);
  let adjacent = "a1b2".match(words);
  let replaced = text.replace(digits, "N");
  let split = text.split(separator);

  total = total + matches.length + adjacent.length + split.length + replaced.length;
  total = total + text.search(/name/);
  index = index + 1;
}

total
