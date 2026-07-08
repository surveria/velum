let text = "id=123;id=456;name=abc";
let single = text.match(/\d+/);
let global = text.match(/\d+/g);
let adjacent = "a1b2".match(/\w/g);
let splitGlobal = text.split(/;/g);
let splitFirst = text.split(/;/);

print(single[0], single.index, single.input, single.length);
print(global.length, global[0], global[1]);
print(adjacent.length, adjacent[0], adjacent[3]);
print(text.search(/name/), text.search(/missing/));
print(text.replace(/\d+/, "N"));
print(text.replace(/\d+/g, "N"));
print(splitGlobal.length, splitGlobal[0], splitGlobal[2]);
print(splitFirst.length, splitFirst[0], splitFirst[1]);
print(String.prototype.match.name, String.prototype.replace.length, String.prototype.split.length);
